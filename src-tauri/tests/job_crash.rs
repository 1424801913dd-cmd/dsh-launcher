#![cfg(windows)]

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
    System::{
        JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        },
        Threading::{
            CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, OpenProcess,
            PROCESS_QUERY_LIMITED_INFORMATION, WaitForSingleObject,
        },
    },
};

const HELPER_ENV: &str = "DSH_LAUNCHER_JOB_CRASH_HELPER";
const READY_ENV: &str = "DSH_LAUNCHER_JOB_CRASH_READY";
const TEST_ROOT_ENV: &str = "DSH_LAUNCHER_JOB_CRASH_ROOT";
const ACTIVE_POINTER_ENV: &str = "DSH_LAUNCHER_JOB_CRASH_ACTIVE_POINTER";
const CREATE_PROCESS_FLAGS: u32 = CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP;
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRecord {
    id: String,
    dsh_version: String,
    node_version: String,
    node_path: String,
    dsh_entry: String,
    workspace: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadyRecord {
    pid: u32,
    url: String,
    runtime_id: String,
    dsh_version: String,
    node_version: String,
}

struct JobHandle(HANDLE);

impl JobHandle {
    fn new() -> Result<Self, String> {
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(format!(
                "CreateJobObjectW failed: {}",
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
                "SetInformationJobObject failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self(handle))
    }

    fn assign(&self, process: &std::process::Child) -> Result<(), String> {
        use std::os::windows::io::AsRawHandle;
        let success =
            unsafe { AssignProcessToJobObject(self.0, process.as_raw_handle() as HANDLE) };
        if success == 0 {
            return Err(format!(
                "AssignProcessToJobObject failed: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(())
    }
}

impl Drop for JobHandle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn active_pointer() -> PathBuf {
    std::env::var_os(ACTIVE_POINTER_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"D:\Tools\dsh-launcher\active.json"))
}

fn test_root() -> PathBuf {
    std::env::var_os(TEST_ROOT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::temp_dir().join(format!("dsh-job-crash-{}", now_ms())))
}

fn parse_loopback_url(line: &str) -> Option<String> {
    let start = line.find("http://127.0.0.1:")?;
    let candidate = line[start..]
        .split_whitespace()
        .next()?
        .trim_end_matches(|character: char| matches!(character, ')' | ']' | ',' | ';'));
    let parsed = reqwest::Url::parse(candidate).ok()?;
    if parsed.scheme() != "http"
        || parsed.host_str() != Some("127.0.0.1")
        || parsed.port().is_none()
    {
        return None;
    }
    Some(candidate.to_string())
}

fn loopback_health(url: &str) -> bool {
    let Some(mut current) = reqwest::Url::parse(url).ok() else {
        return false;
    };
    let Ok(client) = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_millis(500))
        .timeout(Duration::from_secs(3))
        .build()
    else {
        return false;
    };
    let mut cookies = Vec::<String>::new();
    for _ in 0..=5 {
        if current.scheme() != "http"
            || current.host_str() != Some("127.0.0.1")
            || current.port().is_none()
        {
            return false;
        }
        let mut request = client.get(current.clone());
        if !cookies.is_empty() {
            request = request.header(reqwest::header::COOKIE, cookies.join("; "));
        }
        let Ok(response) = request.send() else {
            return false;
        };
        if !response.status().is_redirection() {
            return response.status().is_success();
        }
        for value in response.headers().get_all(reqwest::header::SET_COOKIE) {
            let Ok(value) = value.to_str() else {
                return false;
            };
            let pair = value.split(';').next().unwrap_or_default().trim();
            if !pair.is_empty() {
                cookies.push(pair.to_string());
            }
        }
        let Some(location) = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
        else {
            return false;
        };
        let Ok(next) = current.join(location) else {
            return false;
        };
        current = next;
    }
    false
}

fn process_has_exited(pid: u32) -> bool {
    let handle = unsafe {
        OpenProcess(
            PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
            0,
            pid,
        )
    };
    if handle.is_null() {
        return true;
    }
    let result = unsafe { WaitForSingleObject(handle, 0) };
    unsafe { CloseHandle(handle) };
    result == WAIT_OBJECT_0
}

fn write_ready(path: &Path, ready: &ReadyRecord) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(ready).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create ready record: {error}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.write_all(b"\n"))
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("write ready record: {error}"))
}

fn run_crashing_helper() -> Result<(), String> {
    let ready_path = PathBuf::from(std::env::var_os(READY_ENV).ok_or("missing ready path")?);
    let root = test_root();
    let home = root.join("home");
    fs::create_dir_all(&home).map_err(|error| format!("create isolated home: {error}"))?;
    let runtime: RuntimeRecord = serde_json::from_slice(
        &fs::read(active_pointer()).map_err(|error| format!("read active pointer: {error}"))?,
    )
    .map_err(|error| format!("parse active pointer: {error}"))?;
    let bridge = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/dsh-bridge.mjs");
    let mut child = Command::new(&runtime.node_path)
        .arg(&bridge)
        .arg(&runtime.dsh_entry)
        .args(["web", "--host", "127.0.0.1", "--port", "0", "--no-open"])
        .env("DSH_HOME", &home)
        .env("NO_COLOR", "1")
        .current_dir(&runtime.workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_PROCESS_FLAGS)
        .spawn()
        .map_err(|error| format!("spawn DSH: {error}"))?;
    let job = JobHandle::new()?;
    job.assign(&child)?;
    let pid = child.id();
    let mut stdin = child.stdin.take().ok_or("missing DSH stdin")?;
    let stdout = child.stdout.take().ok_or("missing DSH stdout")?;
    let stderr = child.stderr.take().ok_or("missing DSH stderr")?;
    let (url_sender, url_receiver) = mpsc::channel::<String>();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(url) = parse_loopback_url(&line) {
                let _ = url_sender.send(url);
            }
        }
    });
    thread::spawn(
        move || {
            for _ in BufReader::new(stderr).lines().map_while(Result::ok) {}
        },
    );
    stdin
        .write_all(b"start\n")
        .and_then(|_| stdin.flush())
        .map_err(|error| format!("start bridge: {error}"))?;

    let deadline = Instant::now() + Duration::from_secs(45);
    let mut url = None;
    while Instant::now() < deadline {
        if let Ok(candidate) = url_receiver.recv_timeout(Duration::from_millis(500)) {
            url = Some(candidate);
        }
        if url.as_deref().is_some_and(loopback_health) {
            break;
        }
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_some()
        {
            return Err("DSH exited before becoming healthy".to_string());
        }
    }
    let url = url
        .filter(|url| loopback_health(url))
        .ok_or("DSH health timeout")?;
    write_ready(
        &ready_path,
        &ReadyRecord {
            pid,
            url,
            runtime_id: runtime.id,
            dsh_version: runtime.dsh_version,
            node_version: runtime.node_version,
        },
    )?;
    std::hint::black_box(&job);
    std::process::exit(86);
}

#[test]
#[ignore = "spawns the active DSH and deliberately crashes its supervisor process"]
fn job_object_kills_dsh_tree_when_supervisor_process_crashes() {
    if std::env::var_os(HELPER_ENV).as_deref() == Some(std::ffi::OsStr::new("1")) {
        if let Err(error) = run_crashing_helper() {
            panic!("crash helper failed: {error}");
        }
        unreachable!();
    }

    let root = test_root();
    fs::create_dir_all(&root).expect("create job crash test root");
    let ready_path = root.join("ready.json");
    let current_test = std::env::current_exe().expect("current integration test executable");
    let status = Command::new(current_test)
        .args([
            "--exact",
            "job_object_kills_dsh_tree_when_supervisor_process_crashes",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(HELPER_ENV, "1")
        .env(READY_ENV, &ready_path)
        .env(TEST_ROOT_ENV, &root)
        .status()
        .expect("run crash helper test process");
    assert_eq!(
        status.code(),
        Some(86),
        "helper must exit abruptly with probe code"
    );

    let ready: ReadyRecord =
        serde_json::from_slice(&fs::read(&ready_path).expect("read crash helper ready record"))
            .expect("parse crash helper ready record");
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline
        && (!process_has_exited(ready.pid) || loopback_health(&ready.url))
    {
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        process_has_exited(ready.pid),
        "managed DSH PID must exit when Job handle closes"
    );
    assert!(
        !loopback_health(&ready.url),
        "managed DSH port must close after supervisor crash"
    );
    eprintln!(
        "Job crash cleanup verified for {} / DSH {} / Node {}",
        ready.runtime_id, ready.dsh_version, ready.node_version
    );
    fs::remove_dir_all(root).expect("remove job crash test root");
}
