use crate::runtime_manager::{self, RuntimeRecord};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use reqwest::blocking::Client;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::Duration,
};
use zip::ZipArchive;

#[cfg(windows)]
use std::os::windows::fs::MetadataExt;

const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
const MAX_BUNDLE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_EXTRACTED_BYTES: u64 = 8 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_ENTRIES: usize = 250_000;
const MAX_BACKUP_ENTRIES: u64 = 500_000;
const CLOCK_SKEW_MS: u64 = 5 * 60 * 1000;
const BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"dsh-runtime-bundle-v1\0";

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeUpdateConfig {
    pub schema_version: u32,
    pub enabled: bool,
    pub manifest_url: String,
    pub key_id: String,
    pub public_key: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecureUpdateSnapshot {
    pub configured: bool,
    pub status: String,
    pub available_version: Option<String>,
    pub downloaded_version: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub last_checked_ms: Option<u64>,
    pub backup_path: Option<String>,
    pub launcher_update_configured: bool,
}

impl SecureUpdateSnapshot {
    pub fn new(configured: bool) -> Self {
        Self {
            configured,
            status: if configured { "idle" } else { "disabled" }.to_string(),
            available_version: None,
            downloaded_version: None,
            downloaded_bytes: 0,
            total_bytes: None,
            last_checked_ms: None,
            backup_path: None,
            launcher_update_configured: option_env!("DSH_LAUNCHER_UPDATE_ENABLED") == Some("true"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignedEnvelope {
    schema_version: u32,
    key_id: String,
    payload: String,
    signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeManifest {
    schema_version: u32,
    sequence: u64,
    issued_at_ms: u64,
    expires_at_ms: u64,
    releases: Vec<RuntimeRelease>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRelease {
    channel: String,
    dsh_version: String,
    node_version: String,
    architecture: String,
    bundle_url: String,
    length: u64,
    sha256: String,
    signature: String,
    package_integrity: String,
    recipe_id: String,
    min_launcher_version: String,
    migration: MigrationPlan,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MigrationPlan {
    required: bool,
    id: String,
}

#[derive(Clone, Debug)]
pub struct VerifiedRuntimeRelease {
    release: RuntimeRelease,
    public_key: [u8; 32],
}

impl VerifiedRuntimeRelease {
    pub fn version(&self) -> &str {
        &self.release.dsh_version
    }

    pub fn length(&self) -> u64 {
        self.release.length
    }

    pub fn migration_required(&self) -> bool {
        self.release.migration.required
    }

    pub fn migration_id(&self) -> &str {
        &self.release.migration.id
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeBundleMetadata {
    schema_version: u32,
    dsh_version: String,
    node_version: String,
    architecture: String,
    channel: String,
    package_integrity: String,
    recipe_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SeenManifest {
    schema_version: u32,
    sequence: u64,
    payload_sha256: String,
}

struct DirectoryGuard {
    path: PathBuf,
    armed: bool,
}

impl DirectoryGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for DirectoryGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub fn load_config() -> Result<RuntimeUpdateConfig, String> {
    let config: RuntimeUpdateConfig =
        serde_json::from_str(include_str!("../resources/runtime-update-config.json"))
            .map_err(|error| format!("Runtime 更新配置无效：{error}"))?;
    if config.schema_version != 1 {
        return Err("Runtime 更新配置 schemaVersion 不受支持。".to_string());
    }
    if config.enabled {
        validate_https_url(&config.manifest_url)?;
        decode_public_key(&config.public_key)?;
        if config.key_id.trim().is_empty() {
            return Err("Runtime 更新配置缺少 keyId。".to_string());
        }
    }
    Ok(config)
}

pub fn check_for_update(
    config: &RuntimeUpdateConfig,
    root: &Path,
    channel: &str,
    current_runtime: Option<&str>,
) -> Result<Option<VerifiedRuntimeRelease>, String> {
    if !config.enabled {
        return Err("签名 Runtime 更新尚未配置；本地构建保持安全关闭。".to_string());
    }
    let mut response = http_client()?
        .get(&config.manifest_url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("无法下载签名 Runtime manifest：{error}"))?;
    if response
        .content_length()
        .is_some_and(|length| length > MAX_MANIFEST_BYTES)
    {
        return Err("Runtime manifest 超过大小限制。".to_string());
    }
    let mut bytes = Vec::new();
    response
        .by_ref()
        .take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("无法读取 Runtime manifest：{error}"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("Runtime manifest 超过大小限制。".to_string());
    }
    verify_and_select_manifest(
        config,
        root,
        channel,
        current_runtime,
        env!("CARGO_PKG_VERSION"),
        runtime_manager::now_ms(),
        &bytes,
    )
}

fn verify_and_select_manifest(
    config: &RuntimeUpdateConfig,
    root: &Path,
    channel: &str,
    current_runtime: Option<&str>,
    launcher_version: &str,
    now_ms: u64,
    bytes: &[u8],
) -> Result<Option<VerifiedRuntimeRelease>, String> {
    let envelope: SignedEnvelope = serde_json::from_slice(bytes)
        .map_err(|error| format!("签名 Runtime manifest JSON 无效：{error}"))?;
    if envelope.schema_version != 1 || envelope.key_id != config.key_id {
        return Err("Runtime manifest schemaVersion 或 keyId 不匹配。".to_string());
    }
    let public_key = decode_public_key(&config.public_key)?;
    let payload = decode_bounded_base64(&envelope.payload, MAX_MANIFEST_BYTES as usize)?;
    verify_signature(&public_key, &payload, &envelope.signature, "manifest")?;
    let manifest: RuntimeManifest = serde_json::from_slice(&payload)
        .map_err(|error| format!("Runtime manifest payload 无效：{error}"))?;
    if manifest.schema_version != 1 || manifest.sequence == 0 {
        return Err("Runtime manifest payload schema 或 sequence 无效。".to_string());
    }
    if manifest.issued_at_ms > now_ms.saturating_add(CLOCK_SKEW_MS) {
        return Err("Runtime manifest 的签发时间位于未来。".to_string());
    }
    if manifest.expires_at_ms <= now_ms || manifest.expires_at_ms <= manifest.issued_at_ms {
        return Err("Runtime manifest 已过期或时间范围无效。".to_string());
    }
    let payload_sha256 = format!("{:x}", Sha256::digest(&payload));
    enforce_anti_replay(root, manifest.sequence, &payload_sha256)?;

    let launcher =
        Version::parse(launcher_version).map_err(|error| format!("启动器版本无效：{error}"))?;
    let current = current_runtime
        .map(Version::parse)
        .transpose()
        .ok()
        .flatten();
    let mut matching = manifest
        .releases
        .into_iter()
        .filter(|release| {
            release.channel == channel && release.architecture == target_architecture()
        })
        .collect::<Vec<_>>();
    matching.sort_by(|left, right| {
        Version::parse(&right.dsh_version)
            .ok()
            .cmp(&Version::parse(&left.dsh_version).ok())
    });
    for release in matching {
        validate_release(&release)?;
        let minimum = Version::parse(&release.min_launcher_version)
            .map_err(|error| format!("manifest 的 minLauncherVersion 无效：{error}"))?;
        if launcher < minimum {
            continue;
        }
        let candidate = Version::parse(&release.dsh_version)
            .map_err(|error| format!("manifest 的 DSH 版本无效：{error}"))?;
        if current
            .as_ref()
            .is_some_and(|current| candidate <= *current)
        {
            continue;
        }
        return Ok(Some(VerifiedRuntimeRelease {
            release,
            public_key,
        }));
    }
    Ok(None)
}

fn validate_release(release: &RuntimeRelease) -> Result<(), String> {
    if !matches!(release.channel.as_str(), "recommended" | "alpha") {
        return Err("manifest 包含未知 Runtime 通道。".to_string());
    }
    Version::parse(&release.dsh_version)
        .map_err(|error| format!("manifest 的 dshVersion 无效：{error}"))?;
    Version::parse(&release.node_version)
        .map_err(|error| format!("manifest 的 nodeVersion 无效：{error}"))?;
    validate_https_url(&release.bundle_url)?;
    if release.length == 0 || release.length > MAX_BUNDLE_BYTES {
        return Err("manifest 的 Runtime Bundle 长度无效。".to_string());
    }
    if release.sha256.len() != 64
        || !release
            .sha256
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err("manifest 的 Runtime Bundle SHA-256 无效。".to_string());
    }
    if !release.package_integrity.starts_with("sha512-") || release.recipe_id.trim().is_empty() {
        return Err("manifest 的 Runtime recipe 信息无效。".to_string());
    }
    if release.migration.required && release.migration.id.trim().is_empty() {
        return Err("manifest 声明迁移但没有 migration id。".to_string());
    }
    Ok(())
}

pub fn download_and_install<P, F>(
    root: &Path,
    cache: &Path,
    dsh_home: &Path,
    workspace: &Path,
    release: &VerifiedRuntimeRelease,
    mut progress: P,
    smoke_test: F,
) -> Result<RuntimeRecord, String>
where
    P: FnMut(u8, &str, u64, Option<u64>),
    F: FnOnce(&RuntimeRecord) -> Result<(), String>,
{
    let destination = root
        .join("versions")
        .join(format!("dsh-{}", release.release.dsh_version));
    if destination.join("runtime.json").is_file() {
        let existing = runtime_manager::read_record(&destination.join("runtime.json"))?;
        if runtime_manager::record_valid(&existing)
            && existing.package_integrity == release.release.package_integrity
        {
            return Ok(existing);
        }
        return Err("目标 Runtime 目录已存在但记录无效，拒绝覆盖。".to_string());
    }

    let downloads = cache.join("downloads");
    fs::create_dir_all(&downloads).map_err(|error| format!("无法创建下载目录：{error}"))?;
    let bundle_path = downloads.join(format!(
        "dsh-{}-{}.runtime.zip",
        release.release.dsh_version, release.release.architecture
    ));
    download_bundle(release, &bundle_path, &mut progress)?;
    progress(
        72,
        "签名与 SHA-256 已通过，正在安全解包…",
        release.length(),
        Some(release.length()),
    );

    let staging_root = root.join("staging");
    fs::create_dir_all(&staging_root)
        .map_err(|error| format!("无法创建 Runtime staging 根目录：{error}"))?;
    cleanup_stale_signed_runtime_staging(root, &release.release.dsh_version)?;
    let staging = staging_root.join(format!(
        "signed-dsh-{}-{}",
        release.release.dsh_version,
        runtime_manager::now_ms()
    ));
    fs::create_dir(&staging).map_err(|error| format!("无法创建 Runtime staging：{error}"))?;
    let mut guard = DirectoryGuard::new(staging.clone());
    extract_bundle(&bundle_path, &staging)?;
    let metadata: RuntimeBundleMetadata = serde_json::from_slice(
        &fs::read(staging.join("runtime-bundle.json"))
            .map_err(|error| format!("Runtime Bundle 缺少元数据：{error}"))?,
    )
    .map_err(|error| format!("Runtime Bundle 元数据无效：{error}"))?;
    validate_bundle_metadata(&metadata, &release.release)?;

    let node_path = staging.join("node").join("node.exe");
    let app_root = staging.join("app");
    runtime_manager::validate_installation(
        &node_path,
        &app_root,
        dsh_home,
        &release.release.dsh_version,
        &release.release.package_integrity,
    )?;
    let staged_record = RuntimeRecord {
        schema_version: 1,
        id: format!("signed-{}", release.release.dsh_version),
        dsh_version: release.release.dsh_version.clone(),
        node_version: release.release.node_version.clone(),
        channel: release.release.channel.clone(),
        recipe_id: release.release.recipe_id.clone(),
        node_path: node_path.display().to_string(),
        dsh_entry: app_root
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
            .join("lib")
            .join("bin.js")
            .display()
            .to_string(),
        dsh_home: dsh_home.display().to_string(),
        workspace: workspace.display().to_string(),
        package_integrity: release.release.package_integrity.clone(),
        managed: true,
        smoke_tested: false,
        installed_at_ms: runtime_manager::now_ms(),
    };
    progress(
        84,
        "正在隔离 DSH_HOME 执行完整 smoke test…",
        release.length(),
        Some(release.length()),
    );
    smoke_test(&staged_record)?;
    let final_record = RuntimeRecord {
        node_path: destination
            .join("node")
            .join("node.exe")
            .display()
            .to_string(),
        dsh_entry: destination
            .join("app")
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
            .join("lib")
            .join("bin.js")
            .display()
            .to_string(),
        smoke_tested: true,
        ..staged_record
    };
    commit_runtime_staging_after_record(&staging, &destination, &final_record, || {})?;
    guard.disarm();
    progress(
        100,
        "签名 Runtime 已下载并旁路验证。",
        release.length(),
        Some(release.length()),
    );
    Ok(final_record)
}

fn commit_runtime_staging_after_record<F>(
    staging: &Path,
    destination: &Path,
    record: &RuntimeRecord,
    after_record: F,
) -> Result<(), String>
where
    F: FnOnce(),
{
    runtime_manager::atomic_write_json(&staging.join("runtime.json"), record)?;
    let versions = destination.parent().ok_or("Runtime 版本目录没有父目录。")?;
    fs::create_dir_all(versions).map_err(|error| format!("无法创建版本目录：{error}"))?;
    after_record();
    fs::rename(staging, destination)
        .map_err(|error| format!("无法原子激活旁路 Runtime 目录：{error}"))
}

fn cleanup_stale_signed_runtime_staging(root: &Path, version: &str) -> Result<usize, String> {
    let staging_root = root.join("staging");
    if !staging_root.is_dir() {
        return Ok(0);
    }
    let prefix = format!("signed-dsh-{version}-");
    let mut removed = 0_usize;
    for entry in fs::read_dir(&staging_root)
        .map_err(|error| format!("无法读取 Runtime staging 根目录：{error}"))?
    {
        let entry = entry.map_err(|error| format!("无法读取 Runtime staging 条目：{error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(timestamp) = name.strip_prefix(&prefix) else {
            continue;
        };
        if timestamp.is_empty() || !timestamp.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("无法检查 Runtime staging：{error}"))?;
        if is_reparse_or_symlink(&metadata) || !metadata.is_dir() {
            return Err(format!(
                "Runtime staging 类型异常，拒绝自动清理：{}",
                entry.path().display()
            ));
        }
        fs::remove_dir_all(entry.path())
            .map_err(|error| format!("无法清理中断的 Runtime staging：{error}"))?;
        removed += 1;
    }
    Ok(removed)
}

fn download_bundle<P>(
    release: &VerifiedRuntimeRelease,
    destination: &Path,
    progress: &mut P,
) -> Result<(), String>
where
    P: FnMut(u8, &str, u64, Option<u64>),
{
    let mut response = http_client()?
        .get(&release.release.bundle_url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("Runtime Bundle 下载失败：{error}"))?;
    let content_length = response.content_length();
    persist_verified_bundle(
        release,
        destination,
        content_length,
        &mut response,
        progress,
    )
}

fn persist_verified_bundle<R, P>(
    release: &VerifiedRuntimeRelease,
    destination: &Path,
    content_length: Option<u64>,
    input: &mut R,
    progress: &mut P,
) -> Result<(), String>
where
    R: Read,
    P: FnMut(u8, &str, u64, Option<u64>),
{
    if content_length.is_some_and(|length| length != release.release.length) {
        return Err("Runtime Bundle HTTP 长度与签名 manifest 不一致。".to_string());
    }
    cleanup_stale_bundle_partials(destination)?;
    let temporary = destination.with_extension(format!("part-{}", runtime_manager::now_ms()));
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("无法创建 Runtime Bundle 临时文件：{error}"))?;
    let mut digest = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 128 * 1024];
    let result = (|| {
        loop {
            let read = input
                .read(&mut buffer)
                .map_err(|error| format!("读取 Runtime Bundle 失败：{error}"))?;
            if read == 0 {
                break;
            }
            downloaded = downloaded.saturating_add(read as u64);
            if downloaded > release.release.length || downloaded > MAX_BUNDLE_BYTES {
                return Err("Runtime Bundle 超过签名 manifest 声明的长度。".to_string());
            }
            digest.update(&buffer[..read]);
            output
                .write_all(&buffer[..read])
                .map_err(|error| format!("写入 Runtime Bundle 失败：{error}"))?;
            let percent = 5 + ((downloaded.saturating_mul(60) / release.release.length) as u8);
            progress(
                percent.min(65),
                "正在后台下载签名 Runtime Bundle…",
                downloaded,
                Some(release.release.length),
            );
        }
        if downloaded != release.release.length {
            return Err("Runtime Bundle 实际长度与签名 manifest 不一致。".to_string());
        }
        let digest_output = digest.finalize();
        let actual = format!("{digest_output:x}");
        let digest_bytes: [u8; 32] = digest_output.into();
        if !actual.eq_ignore_ascii_case(&release.release.sha256) {
            return Err("Runtime Bundle SHA-256 校验失败，已拒绝安装。".to_string());
        }
        let mut signed_message = Vec::with_capacity(BUNDLE_SIGNATURE_DOMAIN.len() + 32);
        signed_message.extend_from_slice(BUNDLE_SIGNATURE_DOMAIN);
        signed_message.extend_from_slice(&digest_bytes);
        verify_signature(
            &release.public_key,
            &signed_message,
            &release.release.signature,
            "Runtime Bundle",
        )?;
        output
            .sync_all()
            .map_err(|error| format!("无法同步 Runtime Bundle：{error}"))?;
        Ok(())
    })();
    drop(output);
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary);
        return Err(error);
    }
    runtime_manager::replace_file(&temporary, destination).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

fn cleanup_stale_bundle_partials(destination: &Path) -> Result<usize, String> {
    let parent = destination
        .parent()
        .ok_or("Runtime Bundle 缓存路径没有父目录。")?;
    let stem = destination
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or("Runtime Bundle 缓存文件名无效。")?;
    let prefix = format!("{stem}.part-");
    let mut removed = 0_usize;
    for entry in fs::read_dir(parent)
        .map_err(|error| format!("无法读取 Runtime Bundle 缓存目录：{error}"))?
    {
        let entry = entry.map_err(|error| format!("无法读取 Runtime Bundle 缓存条目：{error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(timestamp) = name.strip_prefix(&prefix) else {
            continue;
        };
        if timestamp.is_empty() || !timestamp.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("无法检查 Runtime Bundle 临时文件：{error}"))?;
        if is_reparse_or_symlink(&metadata) || !metadata.is_file() {
            return Err(format!(
                "Runtime Bundle 临时路径类型异常，拒绝自动清理：{}",
                entry.path().display()
            ));
        }
        fs::remove_file(entry.path())
            .map_err(|error| format!("无法清理中断的 Runtime Bundle 临时文件：{error}"))?;
        removed += 1;
    }
    Ok(removed)
}

fn extract_bundle(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let file =
        File::open(archive_path).map_err(|error| format!("无法打开 Runtime Bundle：{error}"))?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| format!("Runtime Bundle ZIP 无效：{error}"))?;
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err("Runtime Bundle 条目数超过限制。".to_string());
    }
    let mut extracted = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("无法读取 Runtime Bundle 条目：{error}"))?;
        extracted = extracted.saturating_add(entry.size());
        if extracted > MAX_EXTRACTED_BYTES {
            return Err("Runtime Bundle 解压总大小超过限制。".to_string());
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!("Runtime Bundle 包含符号链接：{}", entry.name()));
        }
        let relative = entry
            .enclosed_name()
            .ok_or_else(|| format!("Runtime Bundle 包含不安全路径：{}", entry.name()))?;
        validate_windows_archive_path(&relative, entry.name())?;
        let mut components = relative.components();
        let allowed = components.next().is_some_and(|component| match component {
            Component::Normal(name) => {
                matches!(name.to_str(), Some("node" | "app" | "runtime-bundle.json"))
            }
            _ => false,
        });
        if !allowed {
            return Err(format!("Runtime Bundle 包含未知顶层条目：{}", entry.name()));
        }
        let output = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output).map_err(|error| format!("无法创建解包目录：{error}"))?;
        } else {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(|error| format!("无法创建解包目录：{error}"))?;
            }
            let mut output_file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&output)
                .map_err(|error| format!("无法创建解包文件 {}：{error}", output.display()))?;
            std::io::copy(&mut entry, &mut output_file)
                .map_err(|error| format!("无法解包 {}：{error}", output.display()))?;
        }
    }
    Ok(())
}

fn validate_bundle_metadata(
    metadata: &RuntimeBundleMetadata,
    release: &RuntimeRelease,
) -> Result<(), String> {
    if metadata.schema_version != 1
        || metadata.dsh_version != release.dsh_version
        || metadata.node_version != release.node_version
        || metadata.architecture != release.architecture
        || metadata.channel != release.channel
        || metadata.package_integrity != release.package_integrity
        || metadata.recipe_id != release.recipe_id
    {
        return Err("Runtime Bundle 元数据与签名 manifest 不一致。".to_string());
    }
    Ok(())
}

fn validate_windows_archive_path(path: &Path, display_name: &str) -> Result<(), String> {
    if path.as_os_str().to_string_lossy().len() > 30_000 {
        return Err("Runtime Bundle 路径长度异常。".to_string());
    }
    for component in path.components() {
        let Component::Normal(name) = component else {
            return Err(format!("Runtime Bundle 包含不安全路径：{display_name}"));
        };
        let Some(name) = name.to_str() else {
            return Err("Runtime Bundle 路径不是有效 Unicode。".to_string());
        };
        if name.is_empty()
            || name.len() > 255
            || name.contains(':')
            || name.ends_with(['.', ' '])
            || is_windows_reserved_name(name)
        {
            return Err(format!(
                "Runtime Bundle 包含 Windows 不安全路径：{display_name}"
            ));
        }
    }
    Ok(())
}

fn is_windows_reserved_name(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
}

pub fn backup_dsh_home<P>(cache: &Path, dsh_home: &Path, mut progress: P) -> Result<PathBuf, String>
where
    P: FnMut(u8, &str),
{
    let metadata =
        fs::symlink_metadata(dsh_home).map_err(|error| format!("无法检查 DSH_HOME：{error}"))?;
    if !metadata.is_dir() || is_reparse_or_symlink(&metadata) {
        return Err("DSH_HOME 不是普通目录或本身是重解析点，拒绝备份。".to_string());
    }
    let backups = cache.join("backups");
    fs::create_dir_all(&backups).map_err(|error| format!("无法创建备份目录：{error}"))?;
    let removed = cleanup_stale_backup_staging(&backups)?;
    if removed > 0 {
        progress(2, "已清理上次异常中断留下的备份 staging。");
    }
    let name = format!("dsh-home-{}", runtime_manager::now_ms());
    let staging = backups.join(format!(".{name}.staging"));
    let destination = backups.join(name);
    fs::create_dir(&staging).map_err(|error| format!("无法创建 DSH_HOME 备份 staging：{error}"))?;
    let mut guard = DirectoryGuard::new(staging.clone());
    let mut entries = 0_u64;
    copy_tree_safely(
        dsh_home,
        &staging,
        Path::new(""),
        &mut entries,
        &mut progress,
    )?;
    progress(48, "生产 DSH_HOME 备份已同步，正在提交备份…");
    fs::rename(&staging, &destination)
        .map_err(|error| format!("无法提交 DSH_HOME 备份：{error}"))?;
    guard.disarm();
    Ok(destination)
}

fn cleanup_stale_backup_staging(backups: &Path) -> Result<usize, String> {
    let mut removed = 0_usize;
    for entry in fs::read_dir(backups).map_err(|error| format!("无法读取备份目录：{error}"))?
    {
        let entry = entry.map_err(|error| format!("无法读取备份目录条目：{error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(timestamp) = name
            .strip_prefix(".dsh-home-")
            .and_then(|value| value.strip_suffix(".staging"))
        else {
            continue;
        };
        if timestamp.is_empty() || !timestamp.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("无法检查备份 staging：{error}"))?;
        if is_reparse_or_symlink(&metadata) {
            return Err(format!(
                "备份 staging 是符号链接或重解析点，拒绝自动清理：{}",
                entry.path().display()
            ));
        }
        if metadata.is_dir() {
            fs::remove_dir_all(entry.path())
                .map_err(|error| format!("无法清理中断的备份 staging：{error}"))?;
        } else if metadata.is_file() {
            fs::remove_file(entry.path())
                .map_err(|error| format!("无法清理中断的备份 staging 文件：{error}"))?;
        } else {
            return Err("备份 staging 类型不受支持，拒绝自动清理。".to_string());
        }
        removed += 1;
    }
    Ok(removed)
}

fn copy_tree_safely<P>(
    source: &Path,
    destination: &Path,
    relative_source: &Path,
    entries: &mut u64,
    progress: &mut P,
) -> Result<(), String>
where
    P: FnMut(u8, &str),
{
    for entry in fs::read_dir(source).map_err(|error| format!("无法读取 DSH_HOME 目录：{error}"))?
    {
        let entry = entry.map_err(|error| format!("无法读取 DSH_HOME 条目：{error}"))?;
        *entries = entries.saturating_add(1);
        if *entries > MAX_BACKUP_ENTRIES {
            return Err("DSH_HOME 条目数异常，已中止备份。".to_string());
        }
        let relative_entry = relative_source.join(entry.file_name());
        if is_generated_profile_module_fallback(&relative_entry) {
            progress(20, "正在跳过可由 Runtime 重建的 profile 模块映射…");
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("无法检查 DSH_HOME 条目：{error}"))?;
        if is_reparse_or_symlink(&metadata) {
            return Err(format!(
                "DSH_HOME 包含符号链接或重解析点，拒绝跟随：{}",
                entry.path().display()
            ));
        }
        let target = destination.join(entry.file_name());
        if metadata.is_dir() {
            fs::create_dir(&target).map_err(|error| format!("无法创建备份子目录：{error}"))?;
            copy_tree_safely(&entry.path(), &target, &relative_entry, entries, progress)?;
        } else if metadata.is_file() {
            fs::copy(entry.path(), &target)
                .map_err(|error| format!("无法复制 DSH_HOME 文件：{error}"))?;
        } else {
            return Err("DSH_HOME 包含不支持的特殊文件。".to_string());
        }
        if *entries % 250 == 0 {
            progress(20, "正在备份生产 DSH_HOME；不会记录文件内容…");
        }
    }
    Ok(())
}

fn is_generated_profile_module_fallback(relative: &Path) -> bool {
    let mut components = relative.components();
    let Some(Component::Normal(profiles)) = components.next() else {
        return false;
    };
    let Some(Component::Normal(node_modules)) = components.next() else {
        return false;
    };
    if components.next().is_some() {
        return false;
    }

    let profiles = profiles.to_string_lossy();
    let node_modules = node_modules.to_string_lossy();
    #[cfg(windows)]
    {
        profiles.eq_ignore_ascii_case("profiles")
            && node_modules.eq_ignore_ascii_case("node_modules")
    }
    #[cfg(not(windows))]
    {
        profiles == "profiles" && node_modules == "node_modules"
    }
}

fn enforce_anti_replay(root: &Path, sequence: u64, digest: &str) -> Result<(), String> {
    let path = root.join("update-state.json");
    if let Ok(bytes) = fs::read(&path) {
        let seen: SeenManifest = serde_json::from_slice(&bytes)
            .map_err(|error| format!("本地更新防重放状态无效：{error}"))?;
        if seen.schema_version != 1 {
            return Err("本地更新防重放状态 schemaVersion 无效。".to_string());
        }
        if sequence < seen.sequence
            || (sequence == seen.sequence && !digest.eq_ignore_ascii_case(&seen.payload_sha256))
        {
            return Err("检测到降级、重放或同序列篡改的 Runtime manifest。".to_string());
        }
        if sequence == seen.sequence {
            return Ok(());
        }
    }
    runtime_manager::atomic_write_json(
        &path,
        &SeenManifest {
            schema_version: 1,
            sequence,
            payload_sha256: digest.to_string(),
        },
    )
}

fn decode_public_key(value: &str) -> Result<[u8; 32], String> {
    let bytes = BASE64
        .decode(value)
        .map_err(|_| "Runtime 更新公钥不是有效 Base64。".to_string())?;
    bytes
        .try_into()
        .map_err(|_| "Runtime 更新公钥必须是 32 字节 Ed25519 公钥。".to_string())
}

fn decode_bounded_base64(value: &str, maximum: usize) -> Result<Vec<u8>, String> {
    if value.len() > maximum.saturating_mul(2) {
        return Err("Runtime manifest payload 编码长度异常。".to_string());
    }
    let decoded = BASE64
        .decode(value)
        .map_err(|_| "Runtime manifest payload 不是有效 Base64。".to_string())?;
    if decoded.len() > maximum {
        return Err("Runtime manifest payload 超过大小限制。".to_string());
    }
    Ok(decoded)
}

fn verify_signature(
    public_key: &[u8; 32],
    message: &[u8],
    signature: &str,
    subject: &str,
) -> Result<(), String> {
    let signature_bytes = BASE64
        .decode(signature)
        .map_err(|_| format!("{subject} 签名不是有效 Base64。"))?;
    let signature =
        Signature::from_slice(&signature_bytes).map_err(|_| format!("{subject} 签名长度无效。"))?;
    let key = VerifyingKey::from_bytes(public_key)
        .map_err(|_| "Runtime 更新 Ed25519 公钥无效。".to_string())?;
    key.verify(message, &signature)
        .map_err(|_| format!("{subject} Ed25519 签名校验失败。"))
}

fn validate_https_url(url: &str) -> Result<(), String> {
    let parsed = reqwest::Url::parse(url).map_err(|_| "更新 URL 无效。".to_string())?;
    if parsed.scheme() != "https" || parsed.host_str().is_none() || !parsed.username().is_empty() {
        return Err("更新 URL 必须是无内嵌凭据的 HTTPS 地址。".to_string());
    }
    Ok(())
}

fn target_architecture() -> &'static str {
    if cfg!(all(windows, target_arch = "x86_64")) {
        "windows-x86_64"
    } else {
        "unsupported"
    }
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(15 * 60))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5
                || attempt.url().scheme() != "https"
                || attempt.url().host_str().is_none()
                || !attempt.url().username().is_empty()
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .user_agent(format!("dsh-launcher/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("无法创建更新 HTTP 客户端：{error}"))
}

fn is_reparse_or_symlink(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use zip::{ZipWriter, write::SimpleFileOptions};

    fn test_config(signing: &SigningKey) -> RuntimeUpdateConfig {
        RuntimeUpdateConfig {
            schema_version: 1,
            enabled: true,
            manifest_url: "https://updates.example.test/runtime.json".to_string(),
            key_id: "test-2026".to_string(),
            public_key: BASE64.encode(signing.verifying_key().as_bytes()),
        }
    }

    fn signed_manifest(signing: &SigningKey, sequence: u64, version: &str, now: u64) -> Vec<u8> {
        let payload = serde_json::json!({
            "schemaVersion": 1,
            "sequence": sequence,
            "issuedAtMs": now - 1000,
            "expiresAtMs": now + 60_000,
            "releases": [{
                "channel": "recommended",
                "dshVersion": version,
                "nodeVersion": "24.20.0",
                "architecture": target_architecture(),
                "bundleUrl": "https://updates.example.test/dsh.zip",
                "length": 1234,
                "sha256": "a".repeat(64),
                "signature": BASE64.encode([7_u8; 64]),
                "packageIntegrity": "sha512-test",
                "recipeId": "test-recipe",
                "minLauncherVersion": "0.3.0",
                "migration": { "required": false, "id": "none" }
            }]
        });
        let payload = serde_json::to_vec(&payload).unwrap();
        let signature = signing.sign(&payload);
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "keyId": "test-2026",
            "payload": BASE64.encode(&payload),
            "signature": BASE64.encode(signature.to_bytes())
        }))
        .unwrap()
    }

    fn fault_release(bundle_url: String, length: u64) -> VerifiedRuntimeRelease {
        VerifiedRuntimeRelease {
            release: RuntimeRelease {
                channel: "recommended".to_string(),
                dsh_version: "99.0.0-fault".to_string(),
                node_version: "24.20.0".to_string(),
                architecture: target_architecture().to_string(),
                bundle_url,
                length,
                sha256: "0".repeat(64),
                signature: BASE64.encode([0_u8; 64]),
                package_integrity: "sha512-download-fault".to_string(),
                recipe_id: "download-fault".to_string(),
                min_launcher_version: "0.4.0".to_string(),
                migration: MigrationPlan {
                    required: false,
                    id: "none".to_string(),
                },
            },
            public_key: [0_u8; 32],
        }
    }

    #[test]
    fn accepts_signed_manifest_and_rejects_tampering_and_replay() {
        let root =
            std::env::temp_dir().join(format!("dsh-signed-manifest-{}", runtime_manager::now_ms()));
        fs::create_dir_all(&root).unwrap();
        let signing = SigningKey::from_bytes(&[42_u8; 32]);
        let config = test_config(&signing);
        let now = runtime_manager::now_ms();
        let manifest = signed_manifest(&signing, 8, "9.0.0", now);
        let release = verify_and_select_manifest(
            &config,
            &root,
            "recommended",
            Some("0.1.0"),
            "0.3.0",
            now,
            &manifest,
        )
        .unwrap()
        .unwrap();
        assert_eq!(release.version(), "9.0.0");

        let mut tampered = manifest.clone();
        let index = tampered.len() / 2;
        tampered[index] ^= 1;
        assert!(
            verify_and_select_manifest(
                &config,
                &root,
                "recommended",
                Some("0.1.0"),
                "0.3.0",
                now,
                &tampered,
            )
            .is_err()
        );

        let older = signed_manifest(&signing, 7, "10.0.0", now);
        assert!(
            verify_and_select_manifest(
                &config,
                &root,
                "recommended",
                Some("0.1.0"),
                "0.3.0",
                now,
                &older,
            )
            .is_err()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_non_https_urls_and_unknown_migrations() {
        assert!(validate_https_url("http://updates.example.test/file").is_err());
        assert!(validate_https_url("https://user:pass@example.test/file").is_err());
        assert!(validate_https_url("https://updates.example.test/file").is_ok());
        assert!(validate_windows_archive_path(Path::new("node/file:stream"), "ads").is_err());
        assert!(validate_windows_archive_path(Path::new("app/CON.txt"), "device").is_err());
        assert!(validate_windows_archive_path(Path::new("app/normal.json"), "normal").is_ok());
    }

    #[test]
    fn rejects_tampered_runtime_bundle_digest() {
        let signing = SigningKey::from_bytes(&[19_u8; 32]);
        let digest = Sha256::digest(b"trusted bundle");
        let mut message = Vec::from(BUNDLE_SIGNATURE_DOMAIN);
        message.extend_from_slice(&digest);
        let signature = BASE64.encode(signing.sign(&message).to_bytes());
        let public_key = *signing.verifying_key().as_bytes();
        assert!(verify_signature(&public_key, &message, &signature, "bundle").is_ok());
        let last = message.len() - 1;
        message[last] ^= 1;
        assert!(verify_signature(&public_key, &message, &signature, "bundle").is_err());
    }

    #[test]
    fn interrupted_or_corrupted_download_never_replaces_verified_cache() {
        let root =
            std::env::temp_dir().join(format!("dsh-download-faults-{}", runtime_manager::now_ms()));
        fs::create_dir_all(&root).unwrap();
        let destination = root.join("runtime.zip");
        fs::write(&destination, b"previous verified cache").unwrap();

        let expected = b"new verified runtime bundle";
        let signing = SigningKey::from_bytes(&[27_u8; 32]);
        let digest = Sha256::digest(expected);
        let mut signed_message = Vec::from(BUNDLE_SIGNATURE_DOMAIN);
        signed_message.extend_from_slice(&digest);
        let release = VerifiedRuntimeRelease {
            release: RuntimeRelease {
                channel: "recommended".to_string(),
                dsh_version: "9.0.0".to_string(),
                node_version: "24.20.0".to_string(),
                architecture: target_architecture().to_string(),
                bundle_url: "https://updates.example.test/runtime.zip".to_string(),
                length: expected.len() as u64,
                sha256: format!("{digest:x}"),
                signature: BASE64.encode(signing.sign(&signed_message).to_bytes()),
                package_integrity: "sha512-test".to_string(),
                recipe_id: "test-recipe".to_string(),
                min_launcher_version: "0.3.0".to_string(),
                migration: MigrationPlan {
                    required: false,
                    id: "none".to_string(),
                },
            },
            public_key: *signing.verifying_key().as_bytes(),
        };

        let mut interrupted = std::io::Cursor::new(&expected[..expected.len() / 2]);
        assert!(
            persist_verified_bundle(
                &release,
                &destination,
                Some(expected.len() as u64),
                &mut interrupted,
                &mut |_, _, _, _| {},
            )
            .is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"previous verified cache");

        let corrupted = vec![b'x'; expected.len()];
        let mut corrupted = std::io::Cursor::new(corrupted);
        assert!(
            persist_verified_bundle(
                &release,
                &destination,
                Some(expected.len() as u64),
                &mut corrupted,
                &mut |_, _, _, _| {},
            )
            .is_err()
        );
        assert_eq!(fs::read(&destination).unwrap(), b"previous verified cache");
        assert!(
            fs::read_dir(&root)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().contains(".part-"))
        );

        let mut verified = std::io::Cursor::new(expected);
        persist_verified_bundle(
            &release,
            &destination,
            Some(expected.len() as u64),
            &mut verified,
            &mut |_, _, _, _| {},
        )
        .unwrap();
        assert_eq!(fs::read(&destination).unwrap(), expected);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn socket_disconnect_preserves_verified_cache_and_removes_partial() {
        let root = std::env::temp_dir().join(format!(
            "dsh-socket-disconnect-{}",
            runtime_manager::now_ms()
        ));
        fs::create_dir_all(&root).unwrap();
        let destination = root.join("runtime.zip");
        fs::write(&destination, b"previous verified cache").unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let expected_length = 1024_u64;
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept download request");
            let mut request = [0_u8; 2048];
            let _ = stream.read(&mut request).expect("read download request");
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {expected_length}\r\nConnection: close\r\n\r\n"
            );
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(&vec![b'x'; 128]).unwrap();
            stream.flush().unwrap();
        });
        let release = fault_release(format!("http://{address}/runtime.zip"), expected_length);
        assert!(download_bundle(&release, &destination, &mut |_, _, _, _| {}).is_err());
        server.join().unwrap();
        assert_eq!(fs::read(&destination).unwrap(), b"previous verified cache");
        assert_eq!(cleanup_stale_bundle_partials(&destination).unwrap(), 0);
        assert!(
            fs::read_dir(&root)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().contains(".part-"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn download_process_kill_preserves_verified_cache_and_partial_is_recoverable() {
        const HELPER_ENV: &str = "DSH_LAUNCHER_DOWNLOAD_CRASH_HELPER";
        const ROOT_ENV: &str = "DSH_LAUNCHER_DOWNLOAD_CRASH_ROOT";
        let helper = std::env::var_os(HELPER_ENV).as_deref() == Some(std::ffi::OsStr::new("1"));
        let root = std::env::var_os(ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "dsh-download-process-kill-{}",
                    runtime_manager::now_ms()
                ))
            });
        let destination = root.join("runtime.zip");
        let ready = root.join("download-ready");
        if helper {
            struct BlockingDownload {
                ready: PathBuf,
                first: bool,
            }
            impl Read for BlockingDownload {
                fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                    if self.first {
                        self.first = false;
                        fs::write(&self.ready, b"ready")?;
                        let length = buffer.len().min(64 * 1024);
                        buffer[..length].fill(b'x');
                        return Ok(length);
                    }
                    std::thread::sleep(Duration::from_secs(300));
                    Ok(0)
                }
            }
            let release = fault_release(
                "https://updates.example.test/runtime.zip".to_string(),
                1024 * 1024,
            );
            let mut input = BlockingDownload { ready, first: true };
            persist_verified_bundle(
                &release,
                &destination,
                Some(release.length()),
                &mut input,
                &mut |_, _, _, _| {},
            )
            .expect("download helper must be killed while blocked");
            unreachable!();
        }

        fs::create_dir_all(&root).unwrap();
        fs::write(&destination, b"previous verified cache").unwrap();
        let current_test = std::env::current_exe().expect("current unit test executable");
        let mut child = std::process::Command::new(current_test)
            .args([
                "--exact",
                "runtime_update::tests::download_process_kill_preserves_verified_cache_and_partial_is_recoverable",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(HELPER_ENV, "1")
            .env(ROOT_ENV, &root)
            .spawn()
            .expect("spawn download crash helper");
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !ready.is_file() && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(
            ready.is_file(),
            "download helper did not reach streaming state"
        );
        child.kill().expect("kill download helper process");
        let status = child.wait().expect("wait for killed download helper");
        assert!(!status.success());
        assert_eq!(fs::read(&destination).unwrap(), b"previous verified cache");
        assert_eq!(cleanup_stale_bundle_partials(&destination).unwrap(), 1);
        assert_eq!(fs::read(&destination).unwrap(), b"previous verified cache");
        assert!(
            fs::read_dir(&root)
                .unwrap()
                .flatten()
                .all(|entry| !entry.file_name().to_string_lossy().contains(".part-"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn signed_runtime_staging_process_crash_preserves_active_and_recovers() {
        const HELPER_ENV: &str = "DSH_LAUNCHER_STAGING_CRASH_HELPER";
        const ROOT_ENV: &str = "DSH_LAUNCHER_STAGING_CRASH_ROOT";
        let helper = std::env::var_os(HELPER_ENV).as_deref() == Some(std::ffi::OsStr::new("1"));
        let root = std::env::var_os(ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "dsh-signed-staging-crash-{}",
                    runtime_manager::now_ms()
                ))
            });
        let staging = root.join("staging/signed-dsh-2.0.0-12345");
        let destination = root.join("versions/dsh-2.0.0");
        let home = root.join("home");
        let workspace = root.join("workspace");
        let candidate = RuntimeRecord {
            schema_version: 1,
            id: "signed-2.0.0".to_string(),
            dsh_version: "2.0.0".to_string(),
            node_version: "24.20.0".to_string(),
            channel: "recommended".to_string(),
            recipe_id: "signed-staging-crash".to_string(),
            node_path: destination.join("node.exe").display().to_string(),
            dsh_entry: destination.join("bin.js").display().to_string(),
            dsh_home: home.display().to_string(),
            workspace: workspace.display().to_string(),
            package_integrity: "sha512-signed-staging-crash".to_string(),
            managed: true,
            smoke_tested: true,
            installed_at_ms: runtime_manager::now_ms(),
        };
        if helper {
            commit_runtime_staging_after_record(&staging, &destination, &candidate, || {
                std::process::exit(89)
            })
            .expect("staging helper must terminate before directory commit");
            unreachable!();
        }

        let old_root = root.join("versions/dsh-1.0.0");
        for directory in [&staging, &old_root, &home, &workspace] {
            fs::create_dir_all(directory).expect("signed staging crash directory");
        }
        fs::write(staging.join("node.exe"), b"new node").unwrap();
        fs::write(staging.join("bin.js"), b"new entry").unwrap();
        fs::write(old_root.join("node.exe"), b"old node").unwrap();
        fs::write(old_root.join("bin.js"), b"old entry").unwrap();
        let active = RuntimeRecord {
            id: "signed-1.0.0".to_string(),
            dsh_version: "1.0.0".to_string(),
            recipe_id: "old-active".to_string(),
            node_path: old_root.join("node.exe").display().to_string(),
            dsh_entry: old_root.join("bin.js").display().to_string(),
            package_integrity: "sha512-old-active".to_string(),
            ..candidate.clone()
        };
        runtime_manager::atomic_write_json(&root.join("active.json"), &active).unwrap();

        let current_test = std::env::current_exe().expect("current unit test executable");
        let status = std::process::Command::new(current_test)
            .args([
                "--exact",
                "runtime_update::tests::signed_runtime_staging_process_crash_preserves_active_and_recovers",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(HELPER_ENV, "1")
            .env(ROOT_ENV, &root)
            .status()
            .expect("run signed staging crash helper");
        assert_eq!(status.code(), Some(89));
        assert!(staging.join("runtime.json").is_file());
        assert!(!destination.exists());
        assert_eq!(
            runtime_manager::read_record(&root.join("active.json")).unwrap(),
            active
        );
        assert!(runtime_manager::record_valid(&active));
        assert_eq!(
            cleanup_stale_signed_runtime_staging(&root, "2.0.0").unwrap(),
            1
        );
        assert!(!staging.exists());
        assert!(!destination.exists());
        assert_eq!(
            runtime_manager::read_record(&root.join("active.json")).unwrap(),
            active
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_runtime_bundle_path_escape() {
        let root =
            std::env::temp_dir().join(format!("dsh-unsafe-bundle-{}", runtime_manager::now_ms()));
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("unsafe.zip");
        let archive = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(archive);
        writer
            .start_file("node/../../escape.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"escape").unwrap();
        writer.finish().unwrap();
        let destination = root.join("extract");
        fs::create_dir(&destination).unwrap();
        assert!(extract_bundle(&archive_path, &destination).is_err());
        assert!(!root.join("escape.txt").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_excludes_only_generated_profile_module_fallback() {
        let root =
            std::env::temp_dir().join(format!("dsh-home-backup-{}", runtime_manager::now_ms()));
        let home = root.join("home");
        let cache = root.join("cache");
        fs::create_dir_all(home.join("profiles/node_modules/@example/generated")).unwrap();
        fs::create_dir_all(home.join("profiles/web/node_modules/user-plugin")).unwrap();
        fs::create_dir_all(home.join("sessions")).unwrap();
        fs::write(home.join("settings.yaml"), b"theme: system\n").unwrap();
        fs::write(
            home.join("profiles/node_modules/@example/generated/package.json"),
            b"{}",
        )
        .unwrap();
        fs::write(
            home.join("profiles/web/node_modules/user-plugin/package.json"),
            b"{}",
        )
        .unwrap();
        fs::write(home.join("sessions/session.json"), b"{}").unwrap();

        let backup = backup_dsh_home(&cache, &home, |_, _| {}).unwrap();

        assert_eq!(
            fs::read(backup.join("settings.yaml")).unwrap(),
            b"theme: system\n"
        );
        assert!(backup.join("sessions/session.json").is_file());
        assert!(
            backup
                .join("profiles/web/node_modules/user-plugin/package.json")
                .is_file()
        );
        assert!(!backup.join("profiles/node_modules").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn backup_process_crash_preserves_active_runtime_and_staging_is_recoverable() {
        const HELPER_ENV: &str = "DSH_LAUNCHER_BACKUP_CRASH_HELPER";
        const ROOT_ENV: &str = "DSH_LAUNCHER_BACKUP_CRASH_ROOT";
        let helper = std::env::var_os(HELPER_ENV).as_deref() == Some(std::ffi::OsStr::new("1"));
        let root = std::env::var_os(ROOT_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                std::env::temp_dir().join(format!(
                    "dsh-backup-process-crash-{}",
                    runtime_manager::now_ms()
                ))
            });
        let cache = root.join("cache");
        let home = root.join("home");
        if helper {
            backup_dsh_home(&cache, &home, |percent, _| {
                if percent == 48 {
                    std::process::exit(87);
                }
            })
            .expect("backup helper must be terminated before returning");
            unreachable!();
        }

        let manager = root.join("manager");
        let runtime = root.join("runtime");
        let workspace = root.join("workspace");
        for directory in [&home, &manager, &runtime, &workspace] {
            fs::create_dir_all(directory).expect("backup crash test directory");
        }
        fs::write(home.join("settings.yaml"), b"theme: system\n").expect("test home file");
        fs::write(runtime.join("node.exe"), b"node").expect("test node");
        fs::write(runtime.join("bin.js"), b"entry").expect("test DSH entry");
        let active = RuntimeRecord {
            schema_version: 1,
            id: "active-before-backup-crash".to_string(),
            dsh_version: "1.0.0".to_string(),
            node_version: "24.20.0".to_string(),
            channel: "recommended".to_string(),
            recipe_id: "backup-crash-test".to_string(),
            node_path: runtime.join("node.exe").display().to_string(),
            dsh_entry: runtime.join("bin.js").display().to_string(),
            dsh_home: home.display().to_string(),
            workspace: workspace.display().to_string(),
            package_integrity: "sha512-backup-crash-test".to_string(),
            managed: true,
            smoke_tested: true,
            installed_at_ms: runtime_manager::now_ms(),
        };
        runtime_manager::atomic_write_json(&manager.join("active.json"), &active)
            .expect("active pointer before backup crash");

        let current_test = std::env::current_exe().expect("current unit test executable");
        let status = std::process::Command::new(current_test)
            .args([
                "--exact",
                "runtime_update::tests::backup_process_crash_preserves_active_runtime_and_staging_is_recoverable",
                "--nocapture",
                "--test-threads=1",
            ])
            .env(HELPER_ENV, "1")
            .env(ROOT_ENV, &root)
            .status()
            .expect("run backup crash helper process");
        assert_eq!(status.code(), Some(87));
        assert_eq!(
            runtime_manager::read_record(&manager.join("active.json"))
                .expect("active pointer after backup crash"),
            active
        );
        assert!(runtime_manager::record_valid(&active));

        let backups = cache.join("backups");
        let interrupted: Vec<_> = fs::read_dir(&backups)
            .expect("backup directory after crash")
            .flatten()
            .collect();
        assert_eq!(interrupted.len(), 1, "only interrupted staging may remain");
        assert!(
            interrupted[0]
                .file_name()
                .to_string_lossy()
                .ends_with(".staging")
        );
        assert_eq!(cleanup_stale_backup_staging(&backups).unwrap(), 1);
        assert_eq!(fs::read_dir(&backups).unwrap().count(), 0);
        assert_eq!(
            runtime_manager::read_record(&manager.join("active.json")).unwrap(),
            active
        );
        fs::remove_dir_all(root).expect("cleanup backup crash test root");
    }

    #[test]
    fn generated_profile_module_fallback_match_is_exact() {
        assert!(is_generated_profile_module_fallback(Path::new(
            "profiles/node_modules"
        )));
        assert!(!is_generated_profile_module_fallback(Path::new(
            "profiles/web/node_modules"
        )));
        assert!(!is_generated_profile_module_fallback(Path::new(
            "other/node_modules"
        )));
    }
}
