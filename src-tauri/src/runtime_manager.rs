use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::OnceLock,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zip::ZipArchive;

#[cfg(windows)]
use std::os::windows::{ffi::OsStrExt, process::CommandExt};
#[cfg(windows)]
use windows_sys::Win32::{
    Storage::FileSystem::{
        GetDiskFreeSpaceExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    },
    System::Threading::CREATE_NO_WINDOW,
};

const REGISTRY_PACKAGE_URL: &str = "https://registry.npmjs.org/@deepseek-ai%2fdsh";
const NODE_VERSION: &str = "24.20.0";
const NODE_ARCHIVE: &str = "node-v24.20.0-win-x64.zip";
const NODE_URL: &str = "https://nodejs.org/dist/v24.20.0/node-v24.20.0-win-x64.zip";
const NODE_SHA256: &str = "6cac9ffbca8f6a47091e4b5c772e0606049c3871cb67d900c0cedde630e545ba";
const RC2_VERSION: &str = "0.1.1-rc.2";
const RC2_INTEGRITY: &str = "sha512-UP1UIh6q3Gme/yXRn/QL2P8IsVlv8Shpg22TRJIZPsCRWLm4CBiA1MUvXmJAfsOEETBMLAl+xWPtFw6ICsN3wg==";
const ALPHA2_VERSION: &str = "0.1.2-alpha.2";
const MIN_FREE_BYTES: u64 = 3 * 1024 * 1024 * 1024;
const MAX_NODE_ARCHIVE_BYTES: u64 = 150 * 1024 * 1024;
const MAX_NODE_EXTRACTED_BYTES: u64 = 1024 * 1024 * 1024;

static RUNTIME_ROOT: OnceLock<PathBuf> = OnceLock::new();
static CACHE_ROOT: OnceLock<PathBuf> = OnceLock::new();

pub const LEGACY_NODE: &str = r"D:\Tools\node-v24.19.0-win-x64\node.exe";
pub const LEGACY_DSH_ENTRY: &str =
    r"D:\Tools\dsh-runtime-0.1.1-rc.2\node_modules\@deepseek-ai\dsh\lib\bin.js";
pub const LEGACY_DSH_HOME: &str = r"D:\Caches\deepseek-harness\home";
pub const LEGACY_WORKSPACE: &str = r"D:\Bian_CHENG\dsmax";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRecord {
    pub schema_version: u32,
    pub id: String,
    pub dsh_version: String,
    pub node_version: String,
    pub channel: String,
    pub recipe_id: String,
    pub node_path: String,
    pub dsh_entry: String,
    pub dsh_home: String,
    pub workspace: String,
    pub package_integrity: String,
    pub managed: bool,
    pub smoke_tested: bool,
    pub installed_at_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledRuntime {
    pub id: String,
    pub dsh_version: String,
    pub node_version: String,
    pub channel: String,
    pub recipe_id: String,
    pub managed: bool,
    pub smoke_tested: bool,
    pub active: bool,
    pub installed_at_ms: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreflightInfo {
    pub windows_supported: bool,
    pub architecture: String,
    pub architecture_supported: bool,
    pub webview2_available: bool,
    pub free_bytes: Option<u64>,
    pub enough_disk_space: bool,
    pub runtime_root: String,
    pub cache_root: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionManagerSnapshot {
    pub channel: String,
    pub recommended_version: Option<String>,
    pub alpha_version: Option<String>,
    pub active_version: Option<String>,
    pub previous_version: Option<String>,
    pub installed_versions: Vec<InstalledRuntime>,
    pub first_run_required: bool,
    pub busy: bool,
    pub operation: Option<String>,
    pub progress: u8,
    pub message: Option<String>,
    pub last_checked_ms: Option<u64>,
    pub preflight: PreflightInfo,
}

#[derive(Clone, Debug)]
pub struct VersionManagerState {
    pub snapshot: VersionManagerSnapshot,
}

#[derive(Debug, Deserialize)]
struct RegistryDocument {
    #[serde(rename = "dist-tags")]
    dist_tags: DistTags,
}

#[derive(Debug, Deserialize)]
struct DistTags {
    latest: String,
    alpha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PackageManifest {
    version: String,
    dist: PackageDist,
}

#[derive(Debug, Deserialize)]
struct PackageDist {
    integrity: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledRecipeDocument {
    schema_version: u32,
    recipes: Vec<BundledRecipe>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BundledRecipe {
    id: String,
    dsh_version: String,
    package_integrity: String,
    node_version: String,
    node_url: String,
    node_sha256: String,
    legacy_peer_deps: bool,
    supplemental_dependencies: BTreeMap<String, String>,
}

struct CompatibilityRecipe {
    id: String,
    node_version: String,
    node_url: String,
    node_sha256: String,
    legacy_peer_deps: bool,
    supplemental_dependencies: Vec<(String, String)>,
}

struct StagingGuard {
    path: PathBuf,
    armed: bool,
}

impl StagingGuard {
    fn new(path: PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for StagingGuard {
    fn drop(&mut self) {
        if self.armed {
            if self.path.is_dir() {
                let _ = fs::remove_dir_all(&self.path);
            } else if self.path.is_file() {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn runtime_root() -> PathBuf {
    RUNTIME_ROOT
        .get_or_init(|| {
            if let Some(configured) = env::var_os("DSH_LAUNCHER_RUNTIME_ROOT") {
                return PathBuf::from(configured);
            }
            if Path::new(r"D:\").is_dir() {
                let preferred = PathBuf::from(r"D:\Tools\dsh-launcher");
                if writable_directory(&preferred) {
                    return preferred;
                }
            }
            local_app_data().join("DSH Launcher").join("runtime")
        })
        .clone()
}

pub fn cache_root() -> PathBuf {
    CACHE_ROOT
        .get_or_init(|| {
            if let Some(configured) = env::var_os("DSH_LAUNCHER_CACHE_ROOT") {
                return PathBuf::from(configured);
            }
            if Path::new(r"D:\").is_dir() {
                let preferred = PathBuf::from(r"D:\Caches\dsh-launcher");
                if writable_directory(&preferred) {
                    return preferred;
                }
            }
            local_app_data().join("DSH Launcher").join("cache")
        })
        .clone()
}

pub fn default_dsh_home() -> PathBuf {
    let legacy = PathBuf::from(LEGACY_DSH_HOME);
    if Path::new(r"D:\").is_dir() && writable_directory(&legacy) {
        return legacy;
    }
    local_app_data().join("DeepSeek Harness").join("home")
}

pub fn default_workspace() -> PathBuf {
    let legacy = PathBuf::from(LEGACY_WORKSPACE);
    if legacy.is_dir() {
        return legacy;
    }
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| local_app_data())
        .join("Documents")
        .join("DSH Workspace")
}

fn local_app_data() -> PathBuf {
    env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Users\Public\AppData\Local"))
}

fn writable_directory(path: &Path) -> bool {
    if fs::create_dir_all(path).is_err() {
        return false;
    }
    let probe = path.join(format!(
        ".dsh-launcher-write-test-{}-{}",
        std::process::id(),
        now_ms()
    ));
    match OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(mut file) => {
            let wrote = file.write_all(b"ok").and_then(|_| file.sync_all()).is_ok();
            drop(file);
            let removed = fs::remove_file(&probe).is_ok();
            wrote && removed
        }
        Err(_) => false,
    }
}

pub fn log_path() -> PathBuf {
    cache_root().join("logs").join("launcher.log")
}

pub fn initialize() -> Result<(VersionManagerState, Option<RuntimeRecord>), String> {
    let root = runtime_root();
    let cache = cache_root();
    let dsh_home = default_dsh_home();
    let workspace = default_workspace();
    for directory in [
        root.join("runtime").join("node"),
        root.join("versions"),
        root.join("staging"),
        cache.join("downloads"),
        cache.join("npm"),
        dsh_home.clone(),
        workspace.clone(),
    ] {
        fs::create_dir_all(&directory)
            .map_err(|error| format!("无法创建目录 {}：{error}", directory.display()))?;
    }

    let active_path = root.join("active.json");
    if !active_path.is_file()
        && Path::new(LEGACY_NODE).is_file()
        && Path::new(LEGACY_DSH_ENTRY).is_file()
    {
        let legacy = RuntimeRecord {
            schema_version: 1,
            id: "legacy-0.1.1-rc.2".to_string(),
            dsh_version: RC2_VERSION.to_string(),
            node_version: "24.19.0".to_string(),
            channel: "recommended".to_string(),
            recipe_id: "legacy-rc2-import".to_string(),
            node_path: LEGACY_NODE.to_string(),
            dsh_entry: LEGACY_DSH_ENTRY.to_string(),
            dsh_home: dsh_home.display().to_string(),
            workspace: workspace.display().to_string(),
            package_integrity: RC2_INTEGRITY.to_string(),
            managed: false,
            smoke_tested: true,
            installed_at_ms: now_ms(),
        };
        atomic_write_json(&active_path, &legacy)?;
    }

    let active = read_record(&active_path).ok().filter(record_valid);
    let channel = read_channel(&root).unwrap_or_else(|| "recommended".to_string());
    let preflight = preflight(&root, &cache);
    let mut state = VersionManagerState {
        snapshot: VersionManagerSnapshot {
            channel,
            recommended_version: Some(RC2_VERSION.to_string()),
            alpha_version: Some(ALPHA2_VERSION.to_string()),
            active_version: None,
            previous_version: None,
            installed_versions: Vec::new(),
            first_run_required: active.is_none(),
            busy: false,
            operation: None,
            progress: 0,
            message: None,
            last_checked_ms: None,
            preflight,
        },
    };
    refresh_catalog(&mut state)?;
    Ok((state, active))
}

pub fn refresh_catalog(state: &mut VersionManagerState) -> Result<(), String> {
    let root = PathBuf::from(&state.snapshot.preflight.runtime_root);
    let active = read_record(&root.join("active.json"))
        .ok()
        .filter(record_valid);
    let previous = read_record(&root.join("previous.json"))
        .ok()
        .filter(record_valid);
    let records = scan_records(&root)?;
    state.snapshot.active_version = active.as_ref().map(|record| record.dsh_version.clone());
    state.snapshot.previous_version = previous.as_ref().map(|record| record.dsh_version.clone());
    state.snapshot.first_run_required = active.is_none();
    state.snapshot.installed_versions = records
        .into_values()
        .map(|record| InstalledRuntime {
            active: active.as_ref().is_some_and(|active| active.id == record.id),
            id: record.id,
            dsh_version: record.dsh_version,
            node_version: record.node_version,
            channel: record.channel,
            recipe_id: record.recipe_id,
            managed: record.managed,
            smoke_tested: record.smoke_tested,
            installed_at_ms: record.installed_at_ms,
        })
        .collect();
    state
        .snapshot
        .installed_versions
        .sort_by(|left, right| right.installed_at_ms.cmp(&left.installed_at_ms));
    Ok(())
}

pub fn active_record(state: &VersionManagerState) -> Option<RuntimeRecord> {
    let root = PathBuf::from(&state.snapshot.preflight.runtime_root);
    read_record(&root.join("active.json"))
        .ok()
        .filter(record_valid)
}

pub fn previous_record(state: &VersionManagerState) -> Option<RuntimeRecord> {
    let root = PathBuf::from(&state.snapshot.preflight.runtime_root);
    read_record(&root.join("previous.json"))
        .ok()
        .filter(record_valid)
}

pub fn set_operation(
    state: &mut VersionManagerState,
    operation: Option<&str>,
    progress: u8,
    message: Option<String>,
) {
    state.snapshot.busy = operation.is_some();
    state.snapshot.operation = operation.map(str::to_string);
    state.snapshot.progress = progress.min(100);
    state.snapshot.message = message;
}

pub fn set_channel(state: &mut VersionManagerState, channel: &str) -> Result<(), String> {
    validate_channel(channel)?;
    let root = PathBuf::from(&state.snapshot.preflight.runtime_root);
    atomic_write_json(
        &root.join("preferences.json"),
        &json!({ "schemaVersion": 1, "channel": channel }),
    )?;
    state.snapshot.channel = channel.to_string();
    Ok(())
}

pub fn check_versions() -> Result<(String, Option<String>), String> {
    let document: RegistryDocument = http_client()?
        .get(REGISTRY_PACKAGE_URL)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("无法查询 npm dist-tags：{error}"))?
        .json()
        .map_err(|error| format!("npm dist-tags 响应无法解析：{error}"))?;
    validate_version(&document.dist_tags.latest)?;
    if let Some(alpha) = document.dist_tags.alpha.as_deref() {
        validate_version(alpha)?;
    }
    Ok((document.dist_tags.latest, document.dist_tags.alpha))
}

pub fn apply_checked_versions(
    state: &mut VersionManagerState,
    latest: String,
    alpha: Option<String>,
) {
    state.snapshot.recommended_version = Some(latest);
    state.snapshot.alpha_version = alpha;
    state.snapshot.last_checked_ms = Some(now_ms());
}

pub fn install_runtime<F, P>(
    root: &Path,
    cache: &Path,
    dsh_home: &Path,
    workspace: &Path,
    channel: &str,
    mut progress: P,
    smoke_test: F,
) -> Result<RuntimeRecord, String>
where
    F: FnOnce(&RuntimeRecord) -> Result<(), String>,
    P: FnMut(u8, &str),
{
    validate_channel(channel)?;
    progress(3, "正在查询 npm 官方 dist-tags…");
    let (latest, alpha) = check_versions()?;
    let version = match channel {
        "recommended" => latest,
        "alpha" => alpha.ok_or("npm 当前没有 alpha dist-tag。")?,
        _ => unreachable!(),
    };
    let manifest = fetch_manifest(&version)?;
    if manifest.version != version {
        return Err("npm manifest 版本与请求版本不一致。".to_string());
    }
    let recipe = recipe_for(&version, &manifest.dist.integrity)?;

    let destination = root.join("versions").join(format!("dsh-{version}"));
    fs::create_dir_all(root.join("versions"))
        .map_err(|error| format!("无法创建版本目录：{error}"))?;
    let existing_record = destination.join("runtime.json");
    if existing_record.is_file() {
        let record = read_record(&existing_record)?;
        if record_valid(&record) && record.package_integrity == manifest.dist.integrity {
            progress(100, "该版本已安装并通过验证。");
            return Ok(record);
        }
        return Err(format!(
            "版本目录已存在但记录无效，请人工检查：{}",
            destination.display()
        ));
    }

    progress(10, "正在准备私有 Node.js Runtime…");
    let node_path = ensure_node_runtime(root, cache, &recipe, &mut progress)?;
    let staging = root
        .join("staging")
        .join(format!("dsh-{version}-{}", now_ms()));
    fs::create_dir_all(&staging)
        .map_err(|error| format!("无法创建 staging 目录 {}：{error}", staging.display()))?;
    let mut staging_guard = StagingGuard::new(staging.clone());

    progress(35, "正在生成精确 compatibility recipe…");
    write_package_json(&staging, &version, &recipe)?;
    progress(42, "正在安装 DSH 与生产依赖；安装脚本会正常执行…");
    run_npm_install(&node_path, &staging, cache, recipe.legacy_peer_deps)?;
    progress(68, "正在校验 npm integrity、CLI 和原生模块…");
    validate_installation(
        &node_path,
        &staging,
        dsh_home,
        &version,
        &manifest.dist.integrity,
    )?;

    let staged_record = RuntimeRecord {
        schema_version: 1,
        id: format!("managed-{version}"),
        dsh_version: version.clone(),
        node_version: recipe.node_version.clone(),
        channel: channel.to_string(),
        recipe_id: recipe.id,
        node_path: node_path.display().to_string(),
        dsh_entry: staging
            .join("node_modules")
            .join("@deepseek-ai")
            .join("dsh")
            .join("lib")
            .join("bin.js")
            .display()
            .to_string(),
        dsh_home: dsh_home.display().to_string(),
        workspace: workspace.display().to_string(),
        package_integrity: manifest.dist.integrity,
        managed: true,
        smoke_tested: false,
        installed_at_ms: now_ms(),
    };

    progress(76, "正在隔离 DSH_HOME 执行启动与真正停止 smoke test…");
    smoke_test(&staged_record)?;

    let final_record = RuntimeRecord {
        dsh_entry: destination
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
    atomic_write_json(&staging.join("runtime.json"), &final_record)?;
    progress(92, "正在把已验证版本原子移入不可变版本目录…");
    fs::rename(&staging, &destination).map_err(|error| {
        format!(
            "无法将 staging 移入版本目录 {}：{error}",
            destination.display()
        )
    })?;
    staging_guard.disarm();
    progress(100, "安装与 smoke test 已完成，可手动切换到该版本。");
    Ok(final_record)
}

pub fn switch_to(root: &Path, version_or_id: &str) -> Result<RuntimeRecord, String> {
    let records = scan_records(root)?;
    let target = records
        .into_values()
        .find(|record| record.id == version_or_id || record.dsh_version == version_or_id)
        .ok_or_else(|| format!("未找到已安装版本：{version_or_id}"))?;
    if !record_valid(&target) || !target.smoke_tested {
        return Err("目标 Runtime 未通过完整验证，拒绝切换。".to_string());
    }
    let active_path = root.join("active.json");
    if let Ok(current) = read_record(&active_path) {
        if current.id == target.id {
            return Ok(target);
        }
        atomic_write_json(&root.join("previous.json"), &current)?;
    }
    atomic_write_json(&active_path, &target)?;
    Ok(target)
}

pub fn rollback(root: &Path) -> Result<RuntimeRecord, String> {
    let active_path = root.join("active.json");
    let previous_path = root.join("previous.json");
    let previous = read_record(&previous_path).map_err(|_| "没有可回滚的上一版本。".to_string())?;
    if !record_valid(&previous) || !previous.smoke_tested {
        return Err("上一 Runtime 记录无效或未经 smoke test，拒绝回滚。".to_string());
    }
    let current = read_record(&active_path).ok();
    atomic_write_json(&active_path, &previous)?;
    if let Some(current) = current {
        atomic_write_json(&previous_path, &current)?;
    }
    Ok(previous)
}

pub fn has_previous(state: &VersionManagerState) -> bool {
    previous_record(state).is_some()
}

fn scan_records(root: &Path) -> Result<BTreeMap<String, RuntimeRecord>, String> {
    let mut records = BTreeMap::new();
    let versions = root.join("versions");
    if versions.is_dir() {
        for entry in fs::read_dir(&versions)
            .map_err(|error| format!("无法读取版本目录 {}：{error}", versions.display()))?
            .flatten()
        {
            let record_path = entry.path().join("runtime.json");
            if let Ok(record) = read_record(&record_path)
                && record_valid(&record)
            {
                records.insert(record.id.clone(), record);
            }
        }
    }
    for pointer in ["active.json", "previous.json"] {
        if let Ok(record) = read_record(&root.join(pointer))
            && record_valid(&record)
        {
            records.insert(record.id.clone(), record);
        }
    }
    Ok(records)
}

pub(crate) fn read_record(path: &Path) -> Result<RuntimeRecord, String> {
    let bytes = fs::read(path).map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("{} 格式无效：{error}", path.display()))
}

pub(crate) fn record_valid(record: &RuntimeRecord) -> bool {
    record.schema_version == 1
        && Path::new(&record.node_path).is_file()
        && Path::new(&record.dsh_entry).is_file()
        && Path::new(&record.dsh_home).is_dir()
        && Path::new(&record.workspace).is_dir()
}

fn read_channel(root: &Path) -> Option<String> {
    let document: Value =
        serde_json::from_slice(&fs::read(root.join("preferences.json")).ok()?).ok()?;
    let channel = document.get("channel")?.as_str()?;
    validate_channel(channel).ok()?;
    Some(channel.to_string())
}

fn validate_channel(channel: &str) -> Result<(), String> {
    match channel {
        "recommended" | "alpha" => Ok(()),
        _ => Err("版本通道只能是 recommended 或 alpha。".to_string()),
    }
}

fn validate_version(version: &str) -> Result<(), String> {
    if version.is_empty()
        || version.len() > 80
        || !version
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || ".+-".contains(character))
    {
        return Err(format!("npm 返回了无效版本号：{version}"));
    }
    Ok(())
}

fn fetch_manifest(version: &str) -> Result<PackageManifest, String> {
    validate_version(version)?;
    let manifest: PackageManifest = http_client()?
        .get(format!("{REGISTRY_PACKAGE_URL}/{version}"))
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("无法读取 DSH {version} manifest：{error}"))?
        .json()
        .map_err(|error| format!("DSH {version} manifest 无法解析：{error}"))?;
    if !manifest.dist.integrity.starts_with("sha512-") {
        return Err("npm manifest 缺少 SHA-512 integrity。".to_string());
    }
    Ok(manifest)
}

fn http_client() -> Result<Client, String> {
    Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(60))
        .user_agent(concat!("dsh-launcher/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| format!("无法初始化 HTTPS 客户端：{error}"))
}

fn recipe_for(version: &str, integrity: &str) -> Result<CompatibilityRecipe, String> {
    let document: BundledRecipeDocument =
        serde_json::from_str(include_str!("../resources/compatibility-recipes.json"))
            .map_err(|error| format!("内置 compatibility recipe 无法解析：{error}"))?;
    if document.schema_version != 1 {
        return Err("内置 compatibility recipe schema 不受支持。".to_string());
    }
    if let Some(recipe) = document
        .recipes
        .into_iter()
        .find(|recipe| recipe.dsh_version == version)
    {
        if recipe.package_integrity != integrity {
            return Err(format!(
                "DSH {version} npm integrity 与内置 compatibility recipe 不一致。"
            ));
        }
        return Ok(CompatibilityRecipe {
            id: recipe.id,
            node_version: recipe.node_version,
            node_url: recipe.node_url,
            node_sha256: recipe.node_sha256,
            legacy_peer_deps: recipe.legacy_peer_deps,
            supplemental_dependencies: recipe.supplemental_dependencies.into_iter().collect(),
        });
    }
    Ok(CompatibilityRecipe {
        id: format!("standard-{version}-smoke-v1"),
        node_version: NODE_VERSION.to_string(),
        node_url: NODE_URL.to_string(),
        node_sha256: NODE_SHA256.to_string(),
        legacy_peer_deps: false,
        supplemental_dependencies: Vec::new(),
    })
}

fn write_package_json(
    staging: &Path,
    version: &str,
    recipe: &CompatibilityRecipe,
) -> Result<(), String> {
    let mut dependencies = serde_json::Map::new();
    dependencies.insert(
        "@deepseek-ai/dsh".to_string(),
        Value::String(version.to_string()),
    );
    for (name, version) in &recipe.supplemental_dependencies {
        dependencies.insert(name.clone(), Value::String(version.clone()));
    }
    atomic_write_json(
        &staging.join("package.json"),
        &json!({
            "name": format!("dsh-runtime-{version}"),
            "version": "0.0.0",
            "private": true,
            "description": "Generated by DSH Launcher compatibility recipe",
            "dependencies": dependencies
        }),
    )
}

fn ensure_node_runtime<P>(
    root: &Path,
    cache: &Path,
    recipe: &CompatibilityRecipe,
    progress: &mut P,
) -> Result<PathBuf, String>
where
    P: FnMut(u8, &str),
{
    let node_root = root.join("runtime").join("node").join(&recipe.node_version);
    let node_parent = node_root.parent().ok_or("私有 Node 路径没有父目录。")?;
    fs::create_dir_all(node_parent).map_err(|error| format!("无法创建私有 Node 目录：{error}"))?;
    let node_path = node_root.join("node.exe");
    if node_path.is_file() {
        let output = run_output(&node_path, &["--version"], None, None)?;
        if output.trim() == format!("v{}", recipe.node_version) {
            return Ok(node_path);
        }
        return Err(format!(
            "私有 Node 目录版本异常，请人工检查：{}",
            node_root.display()
        ));
    }

    let download = cache.join("downloads").join(NODE_ARCHIVE);
    if download.is_file() && sha256_file(&download)? != recipe.node_sha256 {
        fs::remove_file(&download)
            .map_err(|error| format!("无法移除校验失败的 Node 下载：{error}"))?;
    }
    if !download.is_file() {
        progress(14, "正在从 nodejs.org 下载便携 Node.js…");
        download_file(&recipe.node_url, &download, MAX_NODE_ARCHIVE_BYTES)?;
    }
    let digest = sha256_file(&download)?;
    if digest != recipe.node_sha256 {
        return Err(format!(
            "Node ZIP SHA-256 不匹配：期望 {}，实际 {digest}",
            recipe.node_sha256
        ));
    }

    progress(25, "Node.js SHA-256 已通过，正在安全解压…");
    let extraction =
        root.join("staging")
            .join(format!("node-{}-{}", recipe.node_version, now_ms()));
    fs::create_dir_all(&extraction).map_err(|error| format!("无法创建 Node staging：{error}"))?;
    let mut guard = StagingGuard::new(extraction.clone());
    extract_node_zip(&download, &extraction)?;
    if !extraction.join("node.exe").is_file()
        || !extraction
            .join("node_modules")
            .join("npm")
            .join("bin")
            .join("npm-cli.js")
            .is_file()
    {
        return Err("Node ZIP 缺少 node.exe 或 npm-cli.js。".to_string());
    }
    fs::rename(&extraction, &node_root)
        .map_err(|error| format!("无法激活私有 Node Runtime：{error}"))?;
    guard.disarm();
    Ok(node_path)
}

fn download_file(url: &str, destination: &Path, maximum_bytes: u64) -> Result<(), String> {
    let mut response = http_client()?
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| format!("下载失败 {url}：{error}"))?;
    if response
        .content_length()
        .is_some_and(|length| length > maximum_bytes)
    {
        return Err("下载文件超过允许的大小。".to_string());
    }
    let parent = destination.parent().ok_or("下载路径没有父目录。")?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建下载目录：{error}"))?;
    let temporary = destination.with_extension(format!("download-{}", now_ms()));
    let mut guard = StagingGuard {
        path: temporary.clone(),
        armed: true,
    };
    let mut file =
        File::create(&temporary).map_err(|error| format!("无法创建下载文件：{error}"))?;
    let copied = io::copy(&mut response.by_ref().take(maximum_bytes + 1), &mut file)
        .map_err(|error| format!("写入下载文件失败：{error}"))?;
    if copied > maximum_bytes {
        return Err("下载文件超过允许的大小。".to_string());
    }
    file.sync_all()
        .map_err(|error| format!("无法同步下载文件：{error}"))?;
    fs::rename(&temporary, destination).map_err(|error| format!("无法完成下载文件：{error}"))?;
    guard.disarm();
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file =
        File::open(path).map_err(|error| format!("无法读取 {}：{error}", path.display()))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("无法计算 SHA-256：{error}"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn extract_node_zip(archive_path: &Path, destination: &Path) -> Result<(), String> {
    let archive_file = File::open(archive_path)
        .map_err(|error| format!("无法打开 Node ZIP {}：{error}", archive_path.display()))?;
    let mut archive =
        ZipArchive::new(archive_file).map_err(|error| format!("Node ZIP 无效：{error}"))?;
    let mut total_size = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("无法读取 Node ZIP 条目：{error}"))?;
        total_size = total_size.saturating_add(entry.size());
        if total_size > MAX_NODE_EXTRACTED_BYTES {
            return Err("Node ZIP 解压总大小超过限制。".to_string());
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!("Node ZIP 包含符号链接，已拒绝：{}", entry.name()));
        }
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| format!("Node ZIP 包含不安全路径：{}", entry.name()))?;
        let mut components = enclosed.components();
        let Some(Component::Normal(top_level)) = components.next() else {
            return Err("Node ZIP 顶层目录无效。".to_string());
        };
        if top_level != NODE_ARCHIVE.trim_end_matches(".zip") {
            return Err(format!("Node ZIP 顶层目录不符合预期：{}", entry.name()));
        }
        let relative: PathBuf = components.collect();
        if relative.as_os_str().is_empty() {
            continue;
        }
        let output = destination.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&output)
                .map_err(|error| format!("无法创建解压目录 {}：{error}", output.display()))?;
            continue;
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("无法创建解压目录 {}：{error}", parent.display()))?;
        }
        let mut output_file = File::create(&output)
            .map_err(|error| format!("无法创建解压文件 {}：{error}", output.display()))?;
        io::copy(&mut entry, &mut output_file)
            .map_err(|error| format!("无法解压 {}：{error}", output.display()))?;
    }
    Ok(())
}

fn run_npm_install(
    node_path: &Path,
    staging: &Path,
    cache: &Path,
    legacy_peer_deps: bool,
) -> Result<(), String> {
    let node_root = node_path.parent().ok_or("Node 路径没有父目录。")?;
    let npm_cli = node_root
        .join("node_modules")
        .join("npm")
        .join("bin")
        .join("npm-cli.js");
    let mut arguments = vec![
        npm_cli.display().to_string(),
        "install".to_string(),
        "--omit=dev".to_string(),
        "--no-audit".to_string(),
        "--no-fund".to_string(),
        "--save-exact".to_string(),
        "--registry=https://registry.npmjs.org/".to_string(),
        format!("--cache={}", cache.join("npm").display()),
    ];
    if legacy_peer_deps {
        arguments.push("--legacy-peer-deps".to_string());
    }
    let argument_refs: Vec<&str> = arguments.iter().map(String::as_str).collect();
    run_output(node_path, &argument_refs, Some(staging), None).map(|_| ())
}

pub(crate) fn validate_installation(
    node_path: &Path,
    staging: &Path,
    dsh_home: &Path,
    expected_version: &str,
    expected_integrity: &str,
) -> Result<(), String> {
    let package_root = staging
        .join("node_modules")
        .join("@deepseek-ai")
        .join("dsh");
    let package_document: Value = serde_json::from_slice(
        &fs::read(package_root.join("package.json"))
            .map_err(|error| format!("无法读取安装后的 DSH package.json：{error}"))?,
    )
    .map_err(|error| format!("安装后的 DSH package.json 无效：{error}"))?;
    if package_document.get("version").and_then(Value::as_str) != Some(expected_version) {
        return Err("安装后的 DSH 版本与目标版本不一致。".to_string());
    }
    let lock: Value = serde_json::from_slice(
        &fs::read(staging.join("package-lock.json"))
            .map_err(|error| format!("无法读取 npm lockfile：{error}"))?,
    )
    .map_err(|error| format!("npm lockfile 无效：{error}"))?;
    let locked = lock
        .pointer("/packages/node_modules~1@deepseek-ai~1dsh/integrity")
        .and_then(Value::as_str);
    if locked != Some(expected_integrity) {
        return Err("npm lockfile 中的 DSH integrity 与 registry manifest 不一致。".to_string());
    }
    let dsh_entry = package_root.join("lib").join("bin.js");
    let version = run_output(
        node_path,
        &[dsh_entry.to_string_lossy().as_ref(), "--version"],
        Some(staging),
        Some(dsh_home),
    )?;
    if version.trim() != expected_version {
        return Err(format!(
            "dsh --version 不匹配：期望 {expected_version}，实际 {}",
            version.trim()
        ));
    }
    run_output(
        node_path,
        &[
            "-e",
            "const sharp=require('sharp');const pty=require('node-pty');if(!sharp||typeof pty.spawn!=='function')process.exit(2)",
        ],
        Some(staging),
        Some(dsh_home),
    )?;
    let config_home = staging.join(".config-smoke-home");
    fs::create_dir_all(&config_home)
        .map_err(|error| format!("无法创建配置 smoke 目录：{error}"))?;
    let config_result = run_output(
        node_path,
        &[
            dsh_entry.to_string_lossy().as_ref(),
            "--profile",
            "web",
            "--dump-default-config",
        ],
        Some(staging),
        Some(&config_home),
    );
    let _ = fs::remove_dir_all(&config_home);
    config_result.map(|_| ())
}

fn run_output(
    executable: &Path,
    arguments: &[&str],
    current_dir: Option<&Path>,
    dsh_home: Option<&Path>,
) -> Result<String, String> {
    let mut command = Command::new(executable);
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(current_dir) = current_dir {
        command.current_dir(current_dir);
    }
    if let Some(dsh_home) = dsh_home {
        command.env("DSH_HOME", dsh_home);
    }
    if let Some(node_root) = executable.parent() {
        let mut path = OsString::from(node_root.as_os_str());
        if let Some(existing) = env::var_os("PATH") {
            path.push(";");
            path.push(existing);
        }
        command.env("PATH", path);
    }
    command.env("NO_COLOR", "1");
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    let output = command
        .output()
        .map_err(|error| format!("无法运行 {}：{error}", executable.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let details = format!("{}\n{}", stderr.trim(), stdout.trim());
        let details: String = details.chars().take(4000).collect();
        return Err(format!(
            "{} 执行失败（{}）：{}",
            executable.display(),
            output.status,
            details
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn preflight(root: &Path, cache: &Path) -> PreflightInfo {
    let free_bytes = disk_free_bytes(root);
    PreflightInfo {
        windows_supported: cfg!(windows),
        architecture: env::consts::ARCH.to_string(),
        architecture_supported: cfg!(target_arch = "x86_64"),
        webview2_available: true,
        enough_disk_space: free_bytes.is_some_and(|bytes| bytes >= MIN_FREE_BYTES),
        free_bytes,
        runtime_root: root.display().to_string(),
        cache_root: cache.display().to_string(),
    }
}

#[cfg(windows)]
fn disk_free_bytes(path: &Path) -> Option<u64> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut available = 0_u64;
    let success = unsafe {
        GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    (success != 0).then_some(available)
}

#[cfg(not(windows))]
fn disk_free_bytes(_path: &Path) -> Option<u64> {
    None
}

pub(crate) fn atomic_write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path.parent().ok_or("JSON 指针路径没有父目录。")?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("无法创建 {}：{error}", parent.display()))?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name().unwrap_or_default().to_string_lossy(),
        now_ms()
    ));
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| format!("无法序列化 JSON：{error}"))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("无法创建临时指针 {}：{error}", temporary.display()))?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("无法写入临时指针：{error}"))?;
    drop(file);
    replace_file(&temporary, path).inspect_err(|_| {
        let _ = fs::remove_file(&temporary);
    })
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    let source_wide: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination_wide: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    let success = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if success == 0 {
        return Err(format!(
            "无法原子更新 {}：{}",
            destination.display(),
            io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), String> {
    fs::rename(source, destination)
        .map_err(|error| format!("无法更新 {}：{error}", destination.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!("dsh-launcher-{name}-{}", now_ms()))
    }

    #[test]
    fn validates_channels_and_versions() {
        assert!(validate_channel("recommended").is_ok());
        assert!(validate_channel("alpha").is_ok());
        assert!(validate_channel("latest").is_err());
        assert!(validate_version("0.1.2-alpha.2").is_ok());
        assert!(validate_version("../../escape").is_err());
    }

    #[test]
    fn rc2_recipe_is_pinned_and_special_cased() {
        let recipe = recipe_for(RC2_VERSION, RC2_INTEGRITY).expect("known recipe");
        assert_eq!(recipe.node_version, NODE_VERSION);
        assert!(recipe.legacy_peer_deps);
        assert!(
            recipe
                .supplemental_dependencies
                .iter()
                .any(|(name, version)| name == "react" && version == "19.2.8")
        );
        assert!(recipe_for(RC2_VERSION, "sha512-tampered").is_err());
    }

    #[test]
    fn atomic_runtime_pointer_round_trips() {
        let root = temporary_root("pointer");
        let node = root.join("node.exe");
        let entry = root.join("bin.js");
        let home = root.join("home");
        let workspace = root.join("workspace");
        fs::create_dir_all(&home).expect("home");
        fs::create_dir_all(&workspace).expect("workspace");
        fs::write(&node, b"node").expect("node");
        fs::write(&entry, b"entry").expect("entry");
        let record = RuntimeRecord {
            schema_version: 1,
            id: "test".to_string(),
            dsh_version: "1.0.0".to_string(),
            node_version: "24.20.0".to_string(),
            channel: "recommended".to_string(),
            recipe_id: "test".to_string(),
            node_path: node.display().to_string(),
            dsh_entry: entry.display().to_string(),
            dsh_home: home.display().to_string(),
            workspace: workspace.display().to_string(),
            package_integrity: "sha512-test".to_string(),
            managed: true,
            smoke_tested: true,
            installed_at_ms: now_ms(),
        };
        atomic_write_json(&root.join("active.json"), &record).expect("write pointer");
        assert_eq!(read_record(&root.join("active.json")).unwrap(), record);
        fs::remove_dir_all(root).expect("cleanup");
    }
}
