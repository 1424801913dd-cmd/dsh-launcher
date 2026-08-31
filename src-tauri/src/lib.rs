#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod runtime_manager;
mod runtime_update;

use runtime_manager::{RuntimeRecord, VersionManagerSnapshot, VersionManagerState};
use runtime_update::{RuntimeUpdateConfig, SecureUpdateSnapshot, VerifiedRuntimeRelease};
use serde::Serialize;
use std::{
    collections::VecDeque,
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    Manager, Runtime,
    menu::{Menu, MenuItem},
    path::BaseDirectory,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

#[cfg(windows)]
use std::os::windows::{io::AsRawHandle, process::CommandExt};
#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject, TerminateJobObject,
        },
        Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW},
    },
};

const MANAGED_NODE: &str = runtime_manager::LEGACY_NODE;
const MANAGED_DSH_ENTRY: &str = runtime_manager::LEGACY_DSH_ENTRY;
const FALLBACK_WEB_URL: &str = "http://127.0.0.1:3080/";
const TRAY_ICON_ID: &str = "dsh-launcher-tray";
const MAX_LOG_LINES: usize = 400;
const LOG_ROTATE_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum DshPhase {
    NotInstalled,
    Stopped,
    Starting,
    Running,
    StoppingGracefully,
    ForceStopping,
    Updating,
    RollbackRequired,
    Crashed,
    ExternalServiceDetected,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeInfo {
    installed: bool,
    runtime_id: Option<String>,
    channel: Option<String>,
    managed_private: bool,
    node_path: String,
    node_version: Option<String>,
    dsh_entry: String,
    dsh_version: Option<String>,
    dsh_home: String,
    workspace: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEntry {
    timestamp_ms: u64,
    level: String,
    message: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LauncherSnapshot {
    phase: DshPhase,
    runtime: RuntimeInfo,
    web_url: Option<String>,
    pid: Option<u32>,
    last_error: Option<String>,
    logs: Vec<LogEntry>,
    version_manager: VersionManagerSnapshot,
    secure_update: SecureUpdateSnapshot,
}

struct ManagedDsh {
    child: Child,
    stdin: ChildStdin,
    job: JobHandle,
    pid: u32,
}

struct Supervisor {
    phase: DshPhase,
    runtime: RuntimeInfo,
    bridge_path: PathBuf,
    managed: Option<ManagedDsh>,
    web_url: Option<String>,
    last_error: Option<String>,
    logs: VecDeque<LogEntry>,
    version_manager: VersionManagerState,
    runtime_update_config: RuntimeUpdateConfig,
    secure_update: SecureUpdateSnapshot,
    available_update: Option<VerifiedRuntimeRelease>,
    pending_update: Option<VerifiedRuntimeRelease>,
}

#[derive(Clone)]
struct AppState {
    supervisor: Arc<Mutex<Supervisor>>,
}

#[cfg(windows)]
struct JobHandle(HANDLE);

#[cfg(windows)]
unsafe impl Send for JobHandle {}
#[cfg(windows)]
unsafe impl Sync for JobHandle {}

#[cfg(windows)]
impl JobHandle {
    fn new() -> Result<Self, String> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(format!(
                "CreateJobObjectW 失败：{}",
                std::io::Error::last_os_error()
            ));
        }

        let mut information: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        information.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let success = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &information as *const _ as *const std::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if success == 0 {
            unsafe { CloseHandle(handle) };
            return Err(format!(
                "SetInformationJobObject 失败：{}",
                std::io::Error::last_os_error()
            ));
        }

        Ok(Self(handle))
    }

    fn assign(&self, child: &Child) -> Result<(), String> {
        let process_handle = child.as_raw_handle() as HANDLE;
        let success = unsafe { AssignProcessToJobObject(self.0, process_handle) };
        if success == 0 {
            return Err(format!(
                "AssignProcessToJobObject 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }

    fn terminate(&self, exit_code: u32) -> Result<(), String> {
        let success = unsafe { TerminateJobObject(self.0, exit_code) };
        if success == 0 {
            return Err(format!(
                "TerminateJobObject 失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

#[cfg(windows)]
impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn append_persistent_log(entry: &LogEntry) {
    let path = runtime_manager::log_path();
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }

    if path.metadata().map(|metadata| metadata.len()).unwrap_or(0) >= LOG_ROTATE_BYTES {
        let previous = parent.join("launcher.previous.log");
        let _ = fs::remove_file(&previous);
        let _ = fs::rename(&path, previous);
    }

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            file,
            "[{}] {:<5} {}",
            entry.timestamp_ms,
            entry.level.to_uppercase(),
            entry.message
        );
    }
}

impl Supervisor {
    fn push_log(&mut self, level: &str, message: impl Into<String>) -> LogEntry {
        let message = message.into();
        let message = if message.chars().count() > 4000 {
            message.chars().take(4000).collect::<String>() + "…"
        } else {
            message
        };
        let entry = LogEntry {
            timestamp_ms: now_ms(),
            level: level.to_string(),
            message,
        };
        self.logs.push_back(entry.clone());
        while self.logs.len() > MAX_LOG_LINES {
            self.logs.pop_front();
        }
        entry
    }

    fn snapshot(&self) -> LauncherSnapshot {
        LauncherSnapshot {
            phase: self.phase,
            runtime: self.runtime.clone(),
            web_url: self.web_url.clone(),
            pid: self.managed.as_ref().map(|process| process.pid),
            last_error: self.last_error.clone(),
            logs: self.logs.iter().cloned().collect(),
            version_manager: self.version_manager.snapshot.clone(),
            secure_update: self.secure_update.clone(),
        }
    }
}

fn shared_log(shared: &Arc<Mutex<Supervisor>>, level: &str, message: impl Into<String>) {
    let entry = {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        supervisor.push_log(level, message)
    };
    append_persistent_log(&entry);
}

fn command_version(executable: &Path, args: &[&str], dsh_home: Option<&Path>) -> Option<String> {
    let mut command = Command::new(executable);
    command
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dsh_home) = dsh_home {
        command.env("DSH_HOME", dsh_home);
    }
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);
    command
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

fn detect_runtime(active: Option<&RuntimeRecord>) -> RuntimeInfo {
    let node_path = active
        .map(|record| PathBuf::from(&record.node_path))
        .unwrap_or_else(|| PathBuf::from(MANAGED_NODE));
    let dsh_entry = active
        .map(|record| PathBuf::from(&record.dsh_entry))
        .unwrap_or_else(|| PathBuf::from(MANAGED_DSH_ENTRY));
    let dsh_home = active
        .map(|record| PathBuf::from(&record.dsh_home))
        .unwrap_or_else(runtime_manager::default_dsh_home);
    let workspace = active
        .map(|record| PathBuf::from(&record.workspace))
        .unwrap_or_else(runtime_manager::default_workspace);
    let installed = node_path.is_file() && dsh_entry.is_file() && dsh_home.is_dir();

    RuntimeInfo {
        installed,
        runtime_id: active.map(|record| record.id.clone()),
        channel: active.map(|record| record.channel.clone()),
        managed_private: active.is_some_and(|record| record.managed),
        node_path: node_path.display().to_string(),
        node_version: active
            .map(|record| format!("v{}", record.node_version))
            .or_else(|| {
                node_path
                    .is_file()
                    .then(|| command_version(&node_path, &["--version"], None))
                    .flatten()
            }),
        dsh_entry: dsh_entry.display().to_string(),
        dsh_version: active.map(|record| record.dsh_version.clone()).or_else(|| {
            (node_path.is_file() && dsh_entry.is_file())
                .then(|| {
                    command_version(
                        &node_path,
                        &[dsh_entry.to_string_lossy().as_ref(), "--version"],
                        Some(&dsh_home),
                    )
                })
                .flatten()
        }),
        dsh_home: dsh_home.display().to_string(),
        workspace: workspace.display().to_string(),
    }
}

fn parse_loopback_url(line: &str) -> Option<String> {
    let start = line.find("http://127.0.0.1:")?;
    let candidate = line[start..]
        .split_whitespace()
        .next()?
        .trim_end_matches(|character: char| matches!(character, ')' | ']' | ',' | ';'));
    let remainder = candidate.strip_prefix("http://127.0.0.1:")?;
    let authority = remainder.split(['/', '?', '#']).next()?;
    if authority.parse::<u16>().is_err() {
        return None;
    }
    let suffix = &remainder[authority.len()..];
    Some(if suffix.is_empty() {
        format!("{candidate}/")
    } else if suffix.starts_with('?') || suffix.starts_with('#') {
        let origin_length = candidate.len() - suffix.len();
        format!("{}/{}", &candidate[..origin_length], suffix)
    } else {
        candidate.to_string()
    })
}

fn loopback_http_response(url: &str) -> Option<reqwest::blocking::Response> {
    let mut current = reqwest::Url::parse(url).ok()?;
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?;
    let mut cookies = Vec::<String>::new();
    for _ in 0..=5 {
        if current.scheme() != "http"
            || current.host_str() != Some("127.0.0.1")
            || current.port().is_none()
        {
            return None;
        }
        let mut request = client.get(current.clone());
        if !cookies.is_empty() {
            request = request.header(reqwest::header::COOKIE, cookies.join("; "));
        }
        let response = request.send().ok()?;
        if !response.status().is_redirection() {
            return Some(response);
        }
        for value in response.headers().get_all(reqwest::header::SET_COOKIE) {
            let pair = value.to_str().ok()?.split(';').next()?.trim();
            if !pair.is_empty() {
                cookies.push(pair.to_string());
            }
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)?
            .to_str()
            .ok()?;
        current = current.join(location).ok()?;
    }
    None
}

fn http_health(url: &str) -> bool {
    loopback_http_response(url).is_some_and(|response| response.status().is_success())
}

fn http_page_has_dsh_title(url: &str) -> bool {
    let Some(response) = loopback_http_response(url) else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let mut body = Vec::new();
    if response.take(256 * 1024).read_to_end(&mut body).is_err() {
        return false;
    }
    String::from_utf8_lossy(&body).contains("DeepSeek Harness")
}

fn spawn_log_reader<R>(shared: Arc<Mutex<Supervisor>>, reader: R, level: &'static str)
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        for line in BufReader::new(reader).lines().map_while(Result::ok) {
            let line = line.trim_end().to_string();
            if line.is_empty() {
                continue;
            }
            if let Some(url) = parse_loopback_url(&line) {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.web_url = Some(url);
            }
            shared_log(&shared, level, line);
        }
    });
}

fn fail_start(
    shared: &Arc<Mutex<Supervisor>>,
    message: String,
) -> Result<LauncherSnapshot, String> {
    let entry = {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(mut managed) = supervisor.managed.take() {
            let _ = managed.job.terminate(1);
            let _ = managed.child.wait();
        }
        supervisor.phase = if runtime_manager::has_previous(&supervisor.version_manager) {
            DshPhase::RollbackRequired
        } else {
            DshPhase::Crashed
        };
        supervisor.web_url = None;
        supervisor.last_error = Some(message.clone());
        supervisor.push_log("error", message.clone())
    };
    append_persistent_log(&entry);
    Err(message)
}

fn start_dsh_blocking(shared: Arc<Mutex<Supervisor>>) -> Result<LauncherSnapshot, String> {
    let (runtime, bridge_path) = {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !supervisor.runtime.installed {
            return Err("未检测到可用的 Node.js、DSH 或 DSH_HOME。".to_string());
        }
        if supervisor.managed.is_some() || supervisor.phase == DshPhase::Running {
            return Err("DSH 已由本启动器管理。".to_string());
        }
        if supervisor.phase == DshPhase::ExternalServiceDetected {
            return Err("检测到外部 3080 服务；为避免误杀或双开，未启动新的 DSH。".to_string());
        }
        supervisor.phase = DshPhase::Starting;
        supervisor.last_error = None;
        supervisor.web_url = None;
        (supervisor.runtime.clone(), supervisor.bridge_path.clone())
    };
    shared_log(&shared, "info", "开始启动受管 DSH 实例。".to_string());

    if !bridge_path.is_file() {
        return fail_start(
            &shared,
            format!("找不到 DSH bridge：{}", bridge_path.display()),
        );
    }

    let node_path = child_process_compatible_path(Path::new(&runtime.node_path));
    let bridge_path = child_process_compatible_path(&bridge_path);
    let dsh_entry = child_process_compatible_path(Path::new(&runtime.dsh_entry));
    let mut command = Command::new(node_path);
    command
        .arg(&bridge_path)
        .arg(&dsh_entry)
        .args(["web", "--host", "127.0.0.1", "--port", "0", "--no-open"])
        .env("DSH_HOME", &runtime.dsh_home)
        .env("NO_COLOR", "1")
        .current_dir(&runtime.workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);

    let mut child = command
        .spawn()
        .map_err(|error| format!("无法启动 Node.js：{error}"))?;
    let job = JobHandle::new().map_err(|error| {
        let _ = child.kill();
        error
    })?;
    job.assign(&child).map_err(|error| {
        let _ = child.kill();
        error
    })?;

    let pid = child.id();
    let mut stdin = child.stdin.take().ok_or("无法建立 DSH 控制管道。")?;
    let stdout = child.stdout.take().ok_or("无法读取 DSH stdout。")?;
    let stderr = child.stderr.take().ok_or("无法读取 DSH stderr。")?;
    spawn_log_reader(shared.clone(), stdout, "info");
    spawn_log_reader(shared.clone(), stderr, "warn");

    stdin
        .write_all(b"start\n")
        .and_then(|_| stdin.flush())
        .map_err(|error| format!("无法启动 DSH bridge：{error}"))?;

    {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        supervisor.managed = Some(ManagedDsh {
            child,
            stdin,
            job,
            pid,
        });
    }
    shared_log(
        &shared,
        "info",
        format!("DSH 进程已加入 Windows Job，PID {pid}。"),
    );

    let deadline = Instant::now() + Duration::from_secs(90);
    loop {
        let (exit_status, web_url) = {
            let mut supervisor = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let exit_status = supervisor
                .managed
                .as_mut()
                .and_then(|managed| managed.child.try_wait().ok().flatten());
            (exit_status, supervisor.web_url.clone())
        };

        if let Some(status) = exit_status {
            return fail_start(&shared, format!("DSH 在健康检查前退出：{status}"));
        }

        if let Some(url) = web_url.filter(|url| http_health(url)) {
            let snapshot = {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.phase = DshPhase::Running;
                supervisor.last_error = None;
                supervisor.web_url = Some(url.clone());
                supervisor.snapshot()
            };
            shared_log(&shared, "info", format!("DSH 健康检查通过：{url}"));
            return Ok(snapshot);
        }

        if Instant::now() >= deadline {
            return fail_start(&shared, "DSH 启动超时（90 秒）。".to_string());
        }
        thread::sleep(Duration::from_millis(250));
    }
}

fn finish_stopped(shared: &Arc<Mutex<Supervisor>>, message: &str) -> LauncherSnapshot {
    let entry;
    let snapshot;
    {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        supervisor.managed.take();
        supervisor.phase = DshPhase::Stopped;
        supervisor.web_url = None;
        supervisor.last_error = None;
        entry = supervisor.push_log("info", message.to_string());
        snapshot = supervisor.snapshot();
    }
    append_persistent_log(&entry);
    snapshot
}

fn stop_dsh_blocking(shared: Arc<Mutex<Supervisor>>) -> Result<LauncherSnapshot, String> {
    {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if supervisor.managed.is_none() {
            if supervisor.phase == DshPhase::ExternalServiceDetected {
                return Err("该服务不是本启动器创建的实例，拒绝按端口或进程名结束。".to_string());
            }
            supervisor.phase = if supervisor.runtime.installed {
                DshPhase::Stopped
            } else {
                DshPhase::NotInstalled
            };
            return Ok(supervisor.snapshot());
        }
        supervisor.phase = DshPhase::StoppingGracefully;
        supervisor.last_error = None;
        if let Some(managed) = supervisor.managed.as_mut() {
            let _ = managed.stdin.write_all(b"shutdown\n");
            let _ = managed.stdin.flush();
        }
    }
    shared_log(&shared, "info", "已请求 DSH 执行完整 dispose。".to_string());

    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        let exited = {
            let mut supervisor = shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            supervisor
                .managed
                .as_mut()
                .and_then(|managed| managed.child.try_wait().ok().flatten())
                .is_some()
        };
        if exited {
            return Ok(finish_stopped(&shared, "DSH 已完成清理并退出。"));
        }
        if Instant::now() >= deadline {
            break;
        }
        thread::sleep(Duration::from_millis(150));
    }

    shared_log(
        &shared,
        "warn",
        "DSH 未在 8 秒内退出，开始终止整个 Windows Job。".to_string(),
    );
    {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        supervisor.phase = DshPhase::ForceStopping;
        if let Some(managed) = supervisor.managed.as_mut() {
            managed.job.terminate(1)?;
            let _ = managed.child.wait();
        }
    }

    Ok(finish_stopped(
        &shared,
        "Windows Job 已终止，DSH 进程树已清理。",
    ))
}

fn refresh_terminated_process(supervisor: &mut Supervisor) {
    let exited = supervisor
        .managed
        .as_mut()
        .and_then(|managed| managed.child.try_wait().ok().flatten());
    if let Some(status) = exited {
        supervisor.managed.take();
        supervisor.web_url = None;
        if matches!(
            supervisor.phase,
            DshPhase::StoppingGracefully | DshPhase::ForceStopping
        ) {
            supervisor.phase = DshPhase::Stopped;
        } else {
            supervisor.phase = if runtime_manager::has_previous(&supervisor.version_manager) {
                DshPhase::RollbackRequired
            } else {
                DshPhase::Crashed
            };
            supervisor.last_error = Some(format!("DSH 意外退出：{status}"));
        }
    }
}

#[tauri::command]
fn get_launcher_snapshot(state: tauri::State<'_, AppState>) -> LauncherSnapshot {
    let mut supervisor = state
        .supervisor
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    refresh_terminated_process(&mut supervisor);
    supervisor.snapshot()
}

#[tauri::command]
async fn start_dsh(state: tauri::State<'_, AppState>) -> Result<LauncherSnapshot, String> {
    let shared = state.supervisor.clone();
    tauri::async_runtime::spawn_blocking(move || start_dsh_blocking(shared))
        .await
        .map_err(|error| format!("启动任务失败：{error}"))?
}

#[tauri::command]
async fn stop_dsh(state: tauri::State<'_, AppState>) -> Result<LauncherSnapshot, String> {
    let shared = state.supervisor.clone();
    tauri::async_runtime::spawn_blocking(move || stop_dsh_blocking(shared))
        .await
        .map_err(|error| format!("停止任务失败：{error}"))?
}

#[tauri::command]
fn open_dsh(state: tauri::State<'_, AppState>) -> Result<LauncherSnapshot, String> {
    let url = {
        let supervisor = state
            .supervisor
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if supervisor.phase != DshPhase::Running {
            return Err("DSH 尚未运行。".to_string());
        }
        supervisor.web_url.clone().ok_or("DSH Web 地址尚未就绪。")?
    };
    open::that(&url).map_err(|error| format!("无法打开默认浏览器：{error}"))?;
    shared_log(
        &state.supervisor,
        "info",
        format!("已交给默认浏览器打开：{url}"),
    );
    let supervisor = state
        .supervisor
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(supervisor.snapshot())
}

fn ensure_runtime_change_allowed(supervisor: &Supervisor) -> Result<(), String> {
    if supervisor.version_manager.snapshot.busy {
        return Err("另一个版本管理操作正在进行。".to_string());
    }
    if supervisor.managed.is_some()
        || matches!(
            supervisor.phase,
            DshPhase::Starting
                | DshPhase::Running
                | DshPhase::StoppingGracefully
                | DshPhase::ForceStopping
        )
    {
        return Err("请先停止受管 DSH，再安装、切换或回滚 Runtime。".to_string());
    }
    Ok(())
}

fn smoke_test_runtime_record(
    record: &RuntimeRecord,
    bridge_path: &Path,
    version_manager: VersionManagerState,
) -> Result<(), String> {
    let smoke_home = runtime_manager::cache_root()
        .join("tests")
        .join(format!("install-smoke-{}", now_ms()));
    fs::create_dir_all(&smoke_home)
        .map_err(|error| format!("无法创建隔离 smoke DSH_HOME：{error}"))?;
    let mut smoke_record = record.clone();
    smoke_record.dsh_home = smoke_home.display().to_string();
    let runtime = detect_runtime(Some(&smoke_record));
    let shared = Arc::new(Mutex::new(Supervisor {
        phase: DshPhase::Stopped,
        runtime,
        bridge_path: bridge_path.to_path_buf(),
        managed: None,
        web_url: None,
        last_error: None,
        logs: VecDeque::new(),
        version_manager,
        runtime_update_config: RuntimeUpdateConfig {
            schema_version: 1,
            enabled: false,
            manifest_url: String::new(),
            key_id: String::new(),
            public_key: String::new(),
        },
        secure_update: SecureUpdateSnapshot::new(false),
        available_update: None,
        pending_update: None,
    }));

    let result = start_dsh_blocking(shared.clone()).and_then(|snapshot| {
        let url = snapshot.web_url.ok_or("smoke test 未产生 Web URL。")?;
        if !http_health(&url) {
            return Err("smoke test 健康检查未通过。".to_string());
        }
        if !http_page_has_dsh_title(&url) {
            return Err("smoke test 页面标题不是 DeepSeek Harness。".to_string());
        }
        let stopped = stop_dsh_blocking(shared.clone())?;
        if stopped.phase != DshPhase::Stopped || http_health(&url) {
            return Err("smoke test 未能真正停止 DSH。".to_string());
        }
        if stopped.logs.iter().any(|entry| {
            let message = entry.message.to_ascii_lowercase();
            message.contains("err_module_not_found")
                || message.contains("cannot find module")
                || message.contains("module not found")
        }) {
            return Err("smoke test 日志包含模块缺失错误。".to_string());
        }
        Ok(())
    });
    if result.is_err() {
        let _ = stop_dsh_blocking(shared);
    }
    let _ = fs::remove_dir_all(&smoke_home);
    result
}

#[tauri::command]
async fn check_runtime_versions(
    state: tauri::State<'_, AppState>,
) -> Result<LauncherSnapshot, String> {
    let shared = state.supervisor.clone();
    {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if supervisor.version_manager.snapshot.busy {
            return Err("另一个版本管理操作正在进行。".to_string());
        }
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            Some("check"),
            10,
            Some("正在查询 npm 官方 dist-tags…".to_string()),
        );
        supervisor.last_error = None;
    }
    let result = match tauri::async_runtime::spawn_blocking(runtime_manager::check_versions).await {
        Ok(result) => result,
        Err(error) => Err(format!("版本查询任务失败：{error}")),
    };
    match result {
        Ok((latest, alpha)) => {
            let snapshot = {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime_manager::apply_checked_versions(
                    &mut supervisor.version_manager,
                    latest,
                    alpha,
                );
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    100,
                    Some("版本信息已刷新。".to_string()),
                );
                supervisor.snapshot()
            };
            shared_log(
                &shared,
                "info",
                "已刷新推荐版与 Alpha dist-tags。".to_string(),
            );
            Ok(snapshot)
        }
        Err(error) => {
            {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    0,
                    Some("版本查询失败；已安装 Runtime 仍可正常使用。".to_string()),
                );
                supervisor.last_error = Some(error.clone());
            }
            shared_log(&shared, "error", error.clone());
            Err(error)
        }
    }
}

#[tauri::command]
fn set_runtime_channel(
    channel: String,
    state: tauri::State<'_, AppState>,
) -> Result<LauncherSnapshot, String> {
    let snapshot = {
        let mut supervisor = state
            .supervisor
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if supervisor.version_manager.snapshot.busy {
            return Err("版本管理操作进行中，暂时不能更改通道。".to_string());
        }
        runtime_manager::set_channel(&mut supervisor.version_manager, &channel)?;
        supervisor.snapshot()
    };
    shared_log(
        &state.supervisor,
        "info",
        format!("版本通道已切换为 {channel}。"),
    );
    Ok(snapshot)
}

#[tauri::command]
async fn install_runtime_channel(
    channel: String,
    state: tauri::State<'_, AppState>,
) -> Result<LauncherSnapshot, String> {
    let shared = state.supervisor.clone();
    let (root, cache, dsh_home, workspace, bridge_path, first_run, manager_for_smoke) = {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ensure_runtime_change_allowed(&supervisor)?;
        runtime_manager::set_channel(&mut supervisor.version_manager, &channel)?;
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            Some("install"),
            1,
            Some("正在准备安装…".to_string()),
        );
        supervisor.phase = DshPhase::Updating;
        supervisor.last_error = None;
        (
            PathBuf::from(&supervisor.version_manager.snapshot.preflight.runtime_root),
            PathBuf::from(&supervisor.version_manager.snapshot.preflight.cache_root),
            PathBuf::from(&supervisor.runtime.dsh_home),
            PathBuf::from(&supervisor.runtime.workspace),
            supervisor.bridge_path.clone(),
            supervisor.version_manager.snapshot.first_run_required,
            supervisor.version_manager.clone(),
        )
    };
    shared_log(
        &shared,
        "info",
        format!("开始安装 {channel} 通道 Runtime。"),
    );

    let operation_shared = shared.clone();
    let result = match tauri::async_runtime::spawn_blocking(move || {
        runtime_manager::install_runtime(
            &root,
            &cache,
            &dsh_home,
            &workspace,
            &channel,
            |progress, message| {
                let mut supervisor = operation_shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    Some("install"),
                    progress,
                    Some(message.to_string()),
                );
            },
            |record| smoke_test_runtime_record(record, &bridge_path, manager_for_smoke),
        )
        .and_then(|record| {
            if first_run {
                runtime_manager::switch_to(&root, &record.id)
            } else {
                Ok(record)
            }
        })
    })
    .await
    {
        Ok(result) => result,
        Err(error) => Err(format!("Runtime 安装任务失败：{error}")),
    };

    match result {
        Ok(record) => {
            let snapshot = {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime_manager::refresh_catalog(&mut supervisor.version_manager)?;
                let active = runtime_manager::active_record(&supervisor.version_manager);
                supervisor.runtime = detect_runtime(active.as_ref());
                supervisor.phase = if supervisor.runtime.installed {
                    DshPhase::Stopped
                } else {
                    DshPhase::NotInstalled
                };
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    100,
                    Some(if first_run {
                        "首次安装完成，Runtime 已激活。".to_string()
                    } else {
                        format!("DSH {} 已安装，可手动切换。", record.dsh_version)
                    }),
                );
                supervisor.snapshot()
            };
            shared_log(
                &shared,
                "info",
                format!("DSH {} 安装与 smoke test 完成。", record.dsh_version),
            );
            Ok(snapshot)
        }
        Err(error) => {
            {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                let active = runtime_manager::active_record(&supervisor.version_manager);
                supervisor.runtime = detect_runtime(active.as_ref());
                supervisor.phase = if supervisor.runtime.installed {
                    DshPhase::Stopped
                } else {
                    DshPhase::NotInstalled
                };
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    0,
                    Some("安装失败；活动 Runtime 未被覆盖。".to_string()),
                );
                supervisor.last_error = Some(error.clone());
            }
            shared_log(&shared, "error", error.clone());
            Err(error)
        }
    }
}

#[tauri::command]
fn switch_runtime(
    version_or_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<LauncherSnapshot, String> {
    let snapshot = {
        let mut supervisor = state
            .supervisor
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ensure_runtime_change_allowed(&supervisor)?;
        let root = PathBuf::from(&supervisor.version_manager.snapshot.preflight.runtime_root);
        let record = runtime_manager::switch_to(&root, &version_or_id)?;
        supervisor.runtime = detect_runtime(Some(&record));
        supervisor.phase = DshPhase::Stopped;
        supervisor.last_error = None;
        runtime_manager::refresh_catalog(&mut supervisor.version_manager)?;
        supervisor.snapshot()
    };
    shared_log(
        &state.supervisor,
        "info",
        format!("活动 Runtime 已切换为 {version_or_id}。"),
    );
    Ok(snapshot)
}

#[tauri::command]
fn rollback_runtime(state: tauri::State<'_, AppState>) -> Result<LauncherSnapshot, String> {
    let (snapshot, version) = {
        let mut supervisor = state
            .supervisor
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        ensure_runtime_change_allowed(&supervisor)?;
        let root = PathBuf::from(&supervisor.version_manager.snapshot.preflight.runtime_root);
        let record = runtime_manager::rollback(&root)?;
        supervisor.runtime = detect_runtime(Some(&record));
        supervisor.phase = DshPhase::Stopped;
        supervisor.last_error = None;
        runtime_manager::refresh_catalog(&mut supervisor.version_manager)?;
        (supervisor.snapshot(), record.dsh_version)
    };
    shared_log(
        &state.supervisor,
        "warn",
        format!("已手动回滚到 DSH {version}。"),
    );
    Ok(snapshot)
}

#[tauri::command]
async fn check_signed_runtime_update(
    state: tauri::State<'_, AppState>,
) -> Result<LauncherSnapshot, String> {
    let shared = state.supervisor.clone();
    let (config, root, channel, current) = {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if supervisor.version_manager.snapshot.busy {
            return Err("另一个 Runtime 操作正在进行。".to_string());
        }
        if !supervisor.runtime_update_config.enabled {
            return Err("签名 Runtime 更新尚未配置；本地构建保持安全关闭。".to_string());
        }
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            Some("signed-check"),
            5,
            Some("正在下载并验证签名 Runtime manifest…".to_string()),
        );
        supervisor.secure_update.status = "checking".to_string();
        supervisor.last_error = None;
        (
            supervisor.runtime_update_config.clone(),
            PathBuf::from(&supervisor.version_manager.snapshot.preflight.runtime_root),
            supervisor.version_manager.snapshot.channel.clone(),
            supervisor.runtime.dsh_version.clone(),
        )
    };
    let result = tauri::async_runtime::spawn_blocking(move || {
        runtime_update::check_for_update(&config, &root, &channel, current.as_deref())
    })
    .await
    .map_err(|error| format!("签名更新检查任务失败：{error}"))?;

    match result {
        Ok(release) => {
            let message = release
                .as_ref()
                .map(|release| format!("发现签名 Runtime {}。", release.version()))
                .unwrap_or_else(|| "当前通道没有更高版本。".to_string());
            let snapshot = {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.secure_update.available_version = release
                    .as_ref()
                    .map(|release| release.version().to_string());
                supervisor.secure_update.last_checked_ms = Some(now_ms());
                supervisor.secure_update.status = if release.is_some() {
                    "available"
                } else {
                    "current"
                }
                .to_string();
                supervisor.available_update = release;
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    100,
                    Some(message.clone()),
                );
                supervisor.snapshot()
            };
            shared_log(&shared, "info", message);
            Ok(snapshot)
        }
        Err(error) => {
            {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.secure_update.status = "error".to_string();
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    0,
                    Some("签名 Runtime manifest 校验失败。".to_string()),
                );
                supervisor.last_error = Some(error.clone());
            }
            shared_log(&shared, "error", error.clone());
            Err(error)
        }
    }
}

#[tauri::command]
async fn download_signed_runtime_update(
    state: tauri::State<'_, AppState>,
) -> Result<LauncherSnapshot, String> {
    let shared = state.supervisor.clone();
    let (release, root, cache, dsh_home, workspace, bridge_path, manager_for_smoke) = {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if supervisor.version_manager.snapshot.busy {
            return Err("另一个 Runtime 操作正在进行。".to_string());
        }
        let release = supervisor
            .available_update
            .clone()
            .ok_or("请先检查并验证签名 Runtime 更新。")?;
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            Some("signed-download"),
            1,
            Some("正在准备后台下载签名 Runtime Bundle…".to_string()),
        );
        supervisor.secure_update.status = "downloading".to_string();
        supervisor.secure_update.downloaded_bytes = 0;
        supervisor.secure_update.total_bytes = Some(release.length());
        supervisor.last_error = None;
        (
            release,
            PathBuf::from(&supervisor.version_manager.snapshot.preflight.runtime_root),
            PathBuf::from(&supervisor.version_manager.snapshot.preflight.cache_root),
            PathBuf::from(&supervisor.runtime.dsh_home),
            PathBuf::from(&supervisor.runtime.workspace),
            supervisor.bridge_path.clone(),
            supervisor.version_manager.clone(),
        )
    };
    shared_log(
        &shared,
        "info",
        format!("开始后台下载签名 Runtime {}。", release.version()),
    );
    let release_for_result = release.clone();
    let progress_shared = shared.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        runtime_update::download_and_install(
            &root,
            &cache,
            &dsh_home,
            &workspace,
            &release,
            |percent, message, downloaded, total| {
                let mut supervisor = progress_shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.secure_update.downloaded_bytes = downloaded;
                supervisor.secure_update.total_bytes = total;
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    Some("signed-download"),
                    percent,
                    Some(message.to_string()),
                );
            },
            |record| smoke_test_runtime_record(record, &bridge_path, manager_for_smoke),
        )
    })
    .await
    .map_err(|error| format!("Runtime Bundle 下载任务失败：{error}"))?;

    match result {
        Ok(record) => {
            let snapshot = {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                runtime_manager::refresh_catalog(&mut supervisor.version_manager)?;
                supervisor.secure_update.status = "ready".to_string();
                supervisor.secure_update.downloaded_version = Some(record.dsh_version.clone());
                supervisor.pending_update = Some(release_for_result);
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    100,
                    Some(format!(
                        "签名 Runtime {} 已旁路验证，等待确认切换。",
                        record.dsh_version
                    )),
                );
                supervisor.snapshot()
            };
            shared_log(
                &shared,
                "info",
                format!(
                    "签名 Runtime {} 已下载并通过 smoke test。",
                    record.dsh_version
                ),
            );
            Ok(snapshot)
        }
        Err(error) => {
            {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.secure_update.status = "error".to_string();
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    0,
                    Some("Runtime Bundle 下载或验证失败；活动版本未改变。".to_string()),
                );
                supervisor.last_error = Some(error.clone());
            }
            shared_log(&shared, "error", error.clone());
            Err(error)
        }
    }
}

#[tauri::command]
async fn apply_signed_runtime_update(
    approved: bool,
    state: tauri::State<'_, AppState>,
) -> Result<LauncherSnapshot, String> {
    if !approved {
        return Err("应用更新需要用户明确确认。".to_string());
    }
    let shared = state.supervisor.clone();
    let (release, root, cache, dsh_home, was_running) = {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if supervisor.version_manager.snapshot.busy {
            return Err("另一个 Runtime 操作正在进行。".to_string());
        }
        if matches!(
            supervisor.phase,
            DshPhase::Starting | DshPhase::StoppingGracefully | DshPhase::ForceStopping
        ) {
            return Err("DSH 正在切换状态，请稍后再应用更新。".to_string());
        }
        let release = supervisor
            .pending_update
            .clone()
            .ok_or("没有已下载并验证的签名 Runtime。")?;
        if release.migration_required() {
            return Err(format!(
                "该更新需要尚未支持的数据迁移 {}，已拒绝切换。",
                release.migration_id()
            ));
        }
        let was_running = supervisor.managed.is_some() || supervisor.phase == DshPhase::Running;
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            Some("signed-apply"),
            2,
            Some("正在安全停止 DSH 并准备生产数据备份…".to_string()),
        );
        supervisor.secure_update.status = "applying".to_string();
        supervisor.last_error = None;
        (
            release,
            PathBuf::from(&supervisor.version_manager.snapshot.preflight.runtime_root),
            PathBuf::from(&supervisor.version_manager.snapshot.preflight.cache_root),
            PathBuf::from(&supervisor.runtime.dsh_home),
            was_running,
        )
    };

    if was_running {
        if let Err(error) = stop_dsh_blocking(shared.clone()) {
            finish_secure_update_error(&shared, &error);
            return Err(error);
        }
    }
    let backup_shared = shared.clone();
    let backup_result = tauri::async_runtime::spawn_blocking(move || {
        runtime_update::backup_dsh_home(&cache, &dsh_home, |percent, message| {
            let mut supervisor = backup_shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime_manager::set_operation(
                &mut supervisor.version_manager,
                Some("signed-apply"),
                percent,
                Some(message.to_string()),
            );
        })
    })
    .await
    .map_err(|error| format!("DSH_HOME 备份任务失败：{error}"))
    .and_then(|result| result);
    let backup = match backup_result {
        Ok(backup) => backup,
        Err(error) => {
            finish_secure_update_error(&shared, &error);
            return Err(error);
        }
    };
    {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        supervisor.secure_update.backup_path = Some(backup.display().to_string());
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            Some("signed-apply"),
            55,
            Some("备份完成，正在原子切换活动 Runtime…".to_string()),
        );
    }

    let switch_result = (|| -> Result<Option<String>, String> {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let old_version = supervisor.runtime.dsh_version.clone();
        let record = runtime_manager::switch_to(&root, release.version())?;
        supervisor.runtime = detect_runtime(Some(&record));
        supervisor.phase = DshPhase::Stopped;
        runtime_manager::refresh_catalog(&mut supervisor.version_manager)?;
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            Some("signed-apply"),
            70,
            Some("新 Runtime 已激活，正在执行生产健康检查…".to_string()),
        );
        Ok(old_version)
    })();
    let old_version = match switch_result {
        Ok(version) => version,
        Err(error) => {
            finish_secure_update_error(&shared, &error);
            return Err(error);
        }
    };

    match start_dsh_blocking(shared.clone()) {
        Ok(_) => {
            let snapshot = {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.secure_update.status = "applied".to_string();
                supervisor.secure_update.available_version = None;
                supervisor.secure_update.downloaded_version = None;
                supervisor.available_update = None;
                supervisor.pending_update = None;
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    100,
                    Some(format!(
                        "DSH {} 已通过生产健康检查并完成切换。",
                        release.version()
                    )),
                );
                supervisor.snapshot()
            };
            shared_log(
                &shared,
                "info",
                format!(
                    "签名 Runtime {} 已生效；DSH_HOME 备份位于 {}。",
                    release.version(),
                    backup.display()
                ),
            );
            Ok(snapshot)
        }
        Err(start_error) => {
            shared_log(
                &shared,
                "error",
                format!("新 Runtime 健康检查失败，开始自动回滚：{start_error}"),
            );
            let rollback_result = (|| -> Result<(), String> {
                let record = runtime_manager::rollback(&root)?;
                {
                    let mut supervisor = shared
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    supervisor.runtime = detect_runtime(Some(&record));
                    supervisor.phase = DshPhase::Stopped;
                    runtime_manager::refresh_catalog(&mut supervisor.version_manager)?;
                }
                start_dsh_blocking(shared.clone()).map(|_| ())
            })();
            let rollback_succeeded = rollback_result.is_ok();
            let message = match &rollback_result {
                Ok(()) => format!(
                    "新 Runtime {} 启动失败，已自动回滚并重新启动 {}。原错误：{}",
                    release.version(),
                    old_version.as_deref().unwrap_or("上一 Runtime"),
                    start_error
                ),
                Err(rollback_error) => format!(
                    "新 Runtime 启动失败，自动回滚也未完成：{rollback_error}。原错误：{start_error}"
                ),
            };
            {
                let mut supervisor = shared
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                supervisor.secure_update.status = if rollback_succeeded {
                    "rolled-back"
                } else {
                    "rollback-required"
                }
                .to_string();
                runtime_manager::set_operation(
                    &mut supervisor.version_manager,
                    None,
                    0,
                    Some(message.clone()),
                );
                supervisor.last_error = Some(message.clone());
                if !rollback_succeeded {
                    supervisor.phase = DshPhase::RollbackRequired;
                }
            }
            shared_log(&shared, "error", message.clone());
            Err(message)
        }
    }
}

fn finish_secure_update_error(shared: &Arc<Mutex<Supervisor>>, error: &str) {
    {
        let mut supervisor = shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        supervisor.secure_update.status = "error".to_string();
        runtime_manager::set_operation(
            &mut supervisor.version_manager,
            None,
            0,
            Some("安全更新未完成；活动 Runtime 指针保持不变。".to_string()),
        );
        supervisor.last_error = Some(error.to_string());
    }
    shared_log(shared, "error", error.to_string());
}

fn show_main_window<R: Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

fn setup_tray<R: Runtime>(
    app: &mut tauri::App<R>,
    supervisor: Arc<Mutex<Supervisor>>,
) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "显示启动器", true, None::<&str>)?;
    let open_web = MenuItem::with_id(app, "open", "打开 DSH 界面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "停止 DSH 并退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &open_web, &quit])?;
    let icon = app.default_window_icon().cloned();

    let mut builder = TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("DSH Launcher · 左键显示窗口");
    if let Some(icon) = icon {
        builder = builder.icon(icon);
    }
    builder
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "show" => show_main_window(app),
            "open" => {
                let url = supervisor
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .web_url
                    .clone();
                if let Some(url) = url {
                    let _ = open::that(url);
                } else {
                    show_main_window(app);
                }
            }
            "quit" => {
                let shared = supervisor.clone();
                let app_handle = app.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    let _ = stop_dsh_blocking(shared);
                    app_handle.exit(0);
                });
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn resolve_bridge_path<R: Runtime>(app: &tauri::App<R>) -> PathBuf {
    app.path()
        .resolve("resources/dsh-bridge.mjs", BaseDirectory::Resource)
        .ok()
        .filter(|path| path.is_file())
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dsh-bridge.mjs")
        })
}

fn child_process_compatible_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.as_os_str().to_string_lossy();
        let verbatim_prefix: String = ['\\', '\\', '?', '\\'].into_iter().collect();
        let unc_prefix = format!("{verbatim_prefix}UNC{}", std::path::MAIN_SEPARATOR);
        if let Some(unc_path) = value.strip_prefix(&unc_prefix) {
            return PathBuf::from(format!(
                "{}{}{}",
                std::path::MAIN_SEPARATOR,
                std::path::MAIN_SEPARATOR,
                unc_path
            ));
        }
        if let Some(dos_path) = value.strip_prefix(&verbatim_prefix) {
            return PathBuf::from(dos_path);
        }
    }
    path.to_path_buf()
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .setup(|app| {
            let (version_manager, active_record) =
                runtime_manager::initialize().map_err(|error| {
                    std::io::Error::other(format!("Runtime Manager 初始化失败：{error}"))
                })?;
            let runtime = detect_runtime(active_record.as_ref());
            let phase = if !runtime.installed {
                DshPhase::NotInstalled
            } else if http_health(FALLBACK_WEB_URL) {
                DshPhase::ExternalServiceDetected
            } else {
                DshPhase::Stopped
            };
            let bridge_path = resolve_bridge_path(app);
            let runtime_update_config = runtime_update::load_config().map_err(|error| {
                std::io::Error::other(format!("签名 Runtime 更新配置初始化失败：{error}"))
            })?;
            let secure_update = SecureUpdateSnapshot::new(runtime_update_config.enabled);
            let supervisor = Arc::new(Mutex::new(Supervisor {
                phase,
                runtime,
                bridge_path,
                managed: None,
                web_url: None,
                last_error: None,
                logs: VecDeque::new(),
                version_manager,
                runtime_update_config,
                secure_update,
                available_update: None,
                pending_update: None,
            }));
            shared_log(
                &supervisor,
                "info",
                "DSH Launcher 第三阶段安全更新组件已初始化。".to_string(),
            );
            if phase == DshPhase::ExternalServiceDetected {
                shared_log(
                    &supervisor,
                    "warn",
                    "3080 端口存在外部 HTTP 响应，启动器不会接管或结束该进程。".to_string(),
                );
            }
            app.manage(AppState {
                supervisor: supervisor.clone(),
            });
            setup_tray(app, supervisor)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let app = window.app_handle();
                let state = app.state::<AppState>();
                if app.tray_by_id(TRAY_ICON_ID).is_none() {
                    shared_log(
                        &state.supervisor,
                        "error",
                        "托盘图标不可用，已取消隐藏窗口以避免启动器失联。".to_string(),
                    );
                    return;
                }
                match window.hide() {
                    Ok(()) => shared_log(
                        &state.supervisor,
                        "info",
                        "主窗口已隐藏到系统托盘。".to_string(),
                    ),
                    Err(error) => shared_log(
                        &state.supervisor,
                        "error",
                        format!("无法隐藏主窗口：{error}"),
                    ),
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_launcher_snapshot,
            start_dsh,
            stop_dsh,
            open_dsh,
            check_runtime_versions,
            set_runtime_channel,
            install_runtime_channel,
            switch_runtime,
            rollback_runtime,
            check_signed_runtime_update,
            download_signed_runtime_update,
            apply_signed_runtime_update
        ])
        .run(tauri::generate_context!())
        .expect("failed to run DSH Launcher");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version_manager_for_test(root: &Path, cache: &Path) -> VersionManagerState {
        VersionManagerState {
            snapshot: VersionManagerSnapshot {
                channel: "recommended".to_string(),
                recommended_version: Some("0.1.1-rc.2".to_string()),
                alpha_version: Some("0.1.2-alpha.2".to_string()),
                active_version: None,
                previous_version: None,
                installed_versions: Vec::new(),
                first_run_required: true,
                busy: false,
                operation: None,
                progress: 0,
                message: None,
                last_checked_ms: None,
                preflight: runtime_manager::PreflightInfo {
                    windows_supported: true,
                    architecture: "x86_64".to_string(),
                    architecture_supported: true,
                    webview2_available: true,
                    free_bytes: None,
                    enough_disk_space: true,
                    runtime_root: root.display().to_string(),
                    cache_root: cache.display().to_string(),
                },
            },
        }
    }

    #[test]
    fn parses_only_loopback_http_urls() {
        assert_eq!(
            parse_loopback_url("dsh web: http://127.0.0.1:43123"),
            Some("http://127.0.0.1:43123/".to_string())
        );
        assert_eq!(
            parse_loopback_url("dsh web: http://127.0.0.1:43123/?token=signed-value"),
            Some("http://127.0.0.1:43123/?token=signed-value".to_string())
        );
        assert_eq!(
            parse_loopback_url("dsh web: http://127.0.0.1:43123?token=signed-value"),
            Some("http://127.0.0.1:43123/?token=signed-value".to_string())
        );
        assert_eq!(parse_loopback_url("http://localhost:43123"), None);
        assert_eq!(parse_loopback_url("http://127.0.0.1:not-a-port"), None);
    }

    fn redirecting_loopback_server() -> (String, thread::JoinHandle<()>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().expect("test server address");
        let handle = thread::spawn(move || {
            for index in 0..2 {
                let (mut stream, _) = listener.accept().expect("accept test request");
                let mut request = [0_u8; 2048];
                let read = stream.read(&mut request).expect("read test request");
                let request = String::from_utf8_lossy(&request[..read]);
                if index == 0 {
                    assert!(request.starts_with("GET /?token=test-value HTTP/1.1"));
                    stream
                        .write_all(
                            b"HTTP/1.1 303 See Other\r\nLocation: /\r\nSet-Cookie: dsh_session=ok; Path=/; HttpOnly\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        )
                        .expect("write redirect response");
                } else {
                    assert!(request.starts_with("GET / HTTP/1.1"));
                    assert!(
                        request
                            .to_ascii_lowercase()
                            .contains("cookie: dsh_session=ok")
                    );
                    let body = "<title>DeepSeek Harness</title>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    stream
                        .write_all(response.as_bytes())
                        .expect("write healthy response");
                }
            }
        });
        (format!("http://{address}/?token=test-value"), handle)
    }

    #[test]
    fn follows_only_loopback_cookie_redirects_for_health_checks() {
        let (url, server) = redirecting_loopback_server();
        assert!(http_health(&url));
        server.join().expect("health server thread");

        let (url, server) = redirecting_loopback_server();
        assert!(http_page_has_dsh_title(&url));
        server.join().expect("title server thread");
    }

    #[cfg(windows)]
    #[test]
    fn normalizes_windows_verbatim_paths_for_node() {
        assert_eq!(
            child_process_compatible_path(Path::new(
                r"\\?\D:\Bian_CHENG\dsh-launcher\resources\dsh-bridge.mjs"
            )),
            PathBuf::from(r"D:\Bian_CHENG\dsh-launcher\resources\dsh-bridge.mjs")
        );
        assert_eq!(
            child_process_compatible_path(Path::new(r"\\?\UNC\server\share\bridge.mjs")),
            PathBuf::from(r"\\server\share\bridge.mjs")
        );
    }

    #[test]
    #[ignore = "requires the locally installed DSH runtime"]
    fn starts_and_truly_stops_real_dsh_in_isolated_home() {
        let test_root = runtime_manager::cache_root()
            .join("tests")
            .join(format!("lifecycle-{}", now_ms()));
        fs::create_dir_all(&test_root).expect("create isolated DSH_HOME");

        let runtime = RuntimeInfo {
            installed: true,
            runtime_id: Some("lifecycle-test".to_string()),
            channel: Some("recommended".to_string()),
            managed_private: false,
            node_path: MANAGED_NODE.to_string(),
            node_version: Some("test".to_string()),
            dsh_entry: MANAGED_DSH_ENTRY.to_string(),
            dsh_version: Some("test".to_string()),
            dsh_home: test_root.display().to_string(),
            workspace: runtime_manager::LEGACY_WORKSPACE.to_string(),
        };
        let bridge_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("dsh-bridge.mjs");
        #[cfg(windows)]
        let bridge_path = PathBuf::from(format!(r"\\?\{}", bridge_path.display()));
        let version_manager =
            version_manager_for_test(&test_root.join("manager"), &runtime_manager::cache_root());

        let shared = Arc::new(Mutex::new(Supervisor {
            phase: DshPhase::Stopped,
            runtime,
            bridge_path,
            managed: None,
            web_url: None,
            last_error: None,
            logs: VecDeque::new(),
            version_manager,
            runtime_update_config: RuntimeUpdateConfig {
                schema_version: 1,
                enabled: false,
                manifest_url: String::new(),
                key_id: String::new(),
                public_key: String::new(),
            },
            secure_update: SecureUpdateSnapshot::new(false),
            available_update: None,
            pending_update: None,
        }));

        let running = start_dsh_blocking(shared.clone()).expect("start real DSH");
        assert_eq!(running.phase, DshPhase::Running);
        let url = running.web_url.expect("managed web URL");
        assert!(http_health(&url), "health endpoint must return HTTP 200");

        let stopped = stop_dsh_blocking(shared.clone()).expect("stop real DSH");
        assert_eq!(stopped.phase, DshPhase::Stopped);
        assert!(!http_health(&url), "health endpoint must be closed");
        assert!(
            shared
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .managed
                .is_none(),
            "managed process and Job handle must be released"
        );

        fs::remove_dir_all(&test_root).expect("remove isolated DSH_HOME");
    }

    #[test]
    #[ignore = "downloads Node and installs the full recommended DSH runtime"]
    fn installs_recommended_runtime_from_scratch() {
        let cache = runtime_manager::cache_root();
        let root = std::env::var_os("DSH_LAUNCHER_TEST_INSTALL_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                cache
                    .join("tests")
                    .join(format!("private-install-{}", now_ms()))
            });
        let dsh_home = root.join("production-home");
        let workspace = PathBuf::from(runtime_manager::LEGACY_WORKSPACE);
        fs::create_dir_all(root.join("versions")).expect("versions directory");
        fs::create_dir_all(&dsh_home).expect("production DSH_HOME");
        assert!(workspace.is_dir(), "real workspace is required");

        let bridge_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("dsh-bridge.mjs");
        let manager = version_manager_for_test(&root, &cache);
        let record = runtime_manager::install_runtime(
            &root,
            &cache,
            &dsh_home,
            &workspace,
            "recommended",
            |_progress, message| println!("{message}"),
            |record| smoke_test_runtime_record(record, &bridge_path, manager),
        )
        .expect("install and smoke test recommended runtime");
        assert!(record.managed);
        assert!(record.smoke_tested);
        assert!(Path::new(&record.node_path).is_file());
        assert!(Path::new(&record.dsh_entry).is_file());

        let active = runtime_manager::switch_to(&root, &record.id).expect("activate runtime");
        assert_eq!(active.id, record.id);
        fs::remove_dir_all(&root).expect("remove private install test root");
    }
}
