import { useCallback, useEffect, useMemo, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import type { DshPhase, LauncherSnapshot, RuntimeChannel } from "./types";

const phaseLabels: Record<DshPhase, string> = {
  notInstalled: "尚未安装",
  stopped: "已停止",
  starting: "正在启动",
  running: "正在运行",
  stoppingGracefully: "正在安全停止",
  forceStopping: "正在强制清理",
  updating: "正在管理 Runtime",
  rollbackRequired: "需要回滚",
  crashed: "运行异常",
  externalServiceDetected: "检测到外部服务",
};

const initialSnapshot: LauncherSnapshot = {
  phase: "notInstalled",
  runtime: {
    installed: false,
    runtimeId: null,
    channel: null,
    managedPrivate: false,
    nodePath: "",
    nodeVersion: null,
    dshEntry: "",
    dshVersion: null,
    dshHome: "",
    workspace: "",
  },
  webUrl: null,
  pid: null,
  lastError: null,
  logs: [],
  versionManager: {
    channel: "recommended",
    recommendedVersion: "0.1.1-rc.2",
    alphaVersion: "0.1.2-alpha.2",
    activeVersion: null,
    previousVersion: null,
    installedVersions: [],
    firstRunRequired: true,
    busy: false,
    operation: null,
    progress: 0,
    message: null,
    lastCheckedMs: null,
    preflight: {
      windowsSupported: true,
      architecture: "x86_64",
      architectureSupported: true,
      webview2Available: true,
      freeBytes: null,
      enoughDiskSpace: false,
      runtimeRoot: "",
      cacheRoot: "",
    },
  },
  secureUpdate: {
    configured: false,
    status: "disabled",
    availableVersion: null,
    downloadedVersion: null,
    downloadedBytes: 0,
    totalBytes: null,
    lastCheckedMs: null,
    backupPath: null,
    launcherUpdateConfigured: false,
  },
};

function formatTime(timestampMs: number) {
  return new Intl.DateTimeFormat("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  }).format(new Date(timestampMs));
}

function formatBytes(bytes: number | null) {
  if (bytes === null) return "无法读取";
  return `${(bytes / 1024 ** 3).toFixed(1)} GB 可用`;
}

export default function App() {
  const [snapshot, setSnapshot] = useState(initialSnapshot);
  const [busy, setBusy] = useState(false);
  const [requestError, setRequestError] = useState<string | null>(null);
  const [logsExpanded, setLogsExpanded] = useState(true);
  const [launcherUpdateBusy, setLauncherUpdateBusy] = useState(false);
  const [launcherUpdateProgress, setLauncherUpdateProgress] = useState<string | null>(null);
  const [launcherVersion, setLauncherVersion] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<LauncherSnapshot>("get_launcher_snapshot");
      setSnapshot(next);
    } catch (error) {
      setRequestError(String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), 1000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  useEffect(() => {
    void getVersion()
      .then(setLauncherVersion)
      .catch(() => setLauncherVersion(null));
  }, []);

  const call = useCallback(
    async (command: string, arguments_?: Record<string, unknown>) => {
      setBusy(true);
      setRequestError(null);
      try {
        const next = await invoke<LauncherSnapshot>(command, arguments_);
        setSnapshot(next);
        return true;
      } catch (error) {
        setRequestError(String(error));
        return false;
      } finally {
        setBusy(false);
        await refresh();
      }
    },
    [refresh],
  );

  const runLifecycleAction = useCallback(
    async (command: "start_dsh" | "stop_dsh" | "open_dsh") => {
      if (
        command === "stop_dsh" &&
        !window.confirm("停止 DSH 会中断当前正在运行的任务。确认继续吗？")
      ) {
        return;
      }
      await call(command);
    },
    [call],
  );

  const chooseChannel = useCallback(
    async (channel: RuntimeChannel) => {
      if (channel === snapshot.versionManager.channel) return;
      await call("set_runtime_channel", { channel });
    },
    [call, snapshot.versionManager.channel],
  );

  const installSelectedChannel = useCallback(async () => {
    const channel = snapshot.versionManager.channel;
    if (
      channel === "alpha" &&
      !window.confirm(
        "Alpha 是主动选择的预览通道，可能包含不稳定改动。它会旁路安装并经过 smoke test，不会静默覆盖当前版本。继续吗？",
      )
    ) {
      return;
    }
    await call("install_runtime_channel", { channel });
  }, [call, snapshot.versionManager.channel]);

  const switchVersion = useCallback(
    async (id: string, version: string) => {
      if (
        !window.confirm(
          `切换到 DSH ${version}？当前版本会保留为上一版本，切换后需要手动启动验证。`,
        )
      ) {
        return;
      }
      await call("switch_runtime", { versionOrId: id });
    },
    [call],
  );

  const rollback = useCallback(async () => {
    if (
      !window.confirm(
        `回滚到 DSH ${snapshot.versionManager.previousVersion ?? "上一版本"}？当前版本不会被删除。`,
      )
    ) {
      return;
    }
    await call("rollback_runtime");
  }, [call, snapshot.versionManager.previousVersion]);

  const applySignedUpdate = useCallback(async () => {
    const target = snapshot.secureUpdate.downloadedVersion;
    if (
      !target ||
      !window.confirm(
        `应用已验证的 DSH ${target}？启动器会停止当前 DSH、备份生产 DSH_HOME、切换并启动新版本；健康检查失败会自动回滚。`,
      )
    ) {
      return;
    }
    await call("apply_signed_runtime_update", { approved: true });
  }, [call, snapshot.secureUpdate.downloadedVersion]);

  const updateLauncher = useCallback(async () => {
    setLauncherUpdateBusy(true);
    setRequestError(null);
    setLauncherUpdateProgress("正在检查启动器签名更新…");
    try {
      const [{ check }, { relaunch }] = await Promise.all([
        import("@tauri-apps/plugin-updater"),
        import("@tauri-apps/plugin-process"),
      ]);
      const update = await check();
      if (!update) {
        setLauncherUpdateProgress("启动器已是最新版本。");
        return;
      }
      if (!window.confirm(`安装已签名的 DSH Launcher ${update.version} 并重启？`)) {
        setLauncherUpdateProgress(null);
        return;
      }
      let downloaded = 0;
      let total: number | undefined;
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength;
        if (event.event === "Progress") downloaded += event.data.chunkLength;
        if (event.event === "Finished") {
          setLauncherUpdateProgress("安装完成，正在重启启动器…");
        } else {
          const amount = total
            ? `${Math.min(100, Math.round((downloaded / total) * 100))}%`
            : `${(downloaded / 1024 ** 2).toFixed(1)} MB`;
          setLauncherUpdateProgress(`正在下载启动器更新：${amount}`);
        }
      });
      await relaunch();
    } catch (error) {
      setRequestError(String(error));
      setLauncherUpdateProgress("启动器更新未完成；当前版本保持不变。");
    } finally {
      setLauncherUpdateBusy(false);
    }
  }, []);

  const isTransitioning = [
    "starting",
    "stoppingGracefully",
    "forceStopping",
    "updating",
  ].includes(snapshot.phase);
  const runtimeLocked =
    busy ||
    snapshot.versionManager.busy ||
    ["starting", "running", "stoppingGracefully", "forceStopping", "updating"].includes(
      snapshot.phase,
    );
  const canStart = snapshot.runtime.installed && ["stopped", "crashed"].includes(snapshot.phase);
  const canStop = snapshot.phase === "running";
  const canOpen = snapshot.phase === "running" && Boolean(snapshot.webUrl);
  const selectedTarget =
    snapshot.versionManager.channel === "alpha"
      ? snapshot.versionManager.alphaVersion
      : snapshot.versionManager.recommendedVersion;
  const selectedInstalled = snapshot.versionManager.installedVersions.some(
    (runtime) => runtime.dshVersion === selectedTarget,
  );
  const preflightReady =
    snapshot.versionManager.preflight.windowsSupported &&
    snapshot.versionManager.preflight.architectureSupported &&
    snapshot.versionManager.preflight.webview2Available &&
    snapshot.versionManager.preflight.enoughDiskSpace;

  const statusDetail = useMemo(() => {
    if (snapshot.phase === "externalServiceDetected") {
      return "3080 端口存在响应，但不是本启动器管理的实例；为避免误杀，停止按钮已禁用。";
    }
    if (snapshot.phase === "rollbackRequired") {
      return "当前 Runtime 启动验证失败；可在版本管理中回滚到上一版本。";
    }
    if (snapshot.versionManager.busy && snapshot.versionManager.message) {
      return snapshot.versionManager.message;
    }
    if (snapshot.lastError) return snapshot.lastError;
    if (snapshot.phase === "running" && snapshot.pid) return `受管进程 PID ${snapshot.pid}`;
    if (snapshot.phase === "starting") return "正在等待 DSH 输出地址并通过 HTTP 健康检查。";
    if (snapshot.versionManager.firstRunRequired) return "完成首次安装向导后即可启动 DSH。";
    return "浏览器窗口与 DSH 后台服务相互独立。";
  }, [snapshot]);

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand-mini">
          <img src="/dsh-logo.svg" alt="DSH" />
          <div>
            <strong>DSH Launcher</strong>
            <span>非官方本地启动器</span>
          </div>
        </div>
        <div className="topbar-note">关闭窗口将隐藏到系统托盘</div>
      </header>

      {snapshot.versionManager.firstRunRequired && (
        <section className="panel setup-wizard" aria-labelledby="setup-title">
          <div className="wizard-copy">
            <span className="eyebrow">首次安装 · 1 / 3</span>
            <h2 id="setup-title">准备私有 DSH Runtime</h2>
            <p>
              启动器会下载经过 SHA-256 校验的便携 Node.js，在旁路目录安装 DSH，并使用隔离的
              DSH_HOME 完成启动与真正停止测试。不会修改系统 PATH，也不会读取凭据。
            </p>
          </div>
          <div className="preflight-grid">
            <span className={snapshot.versionManager.preflight.windowsSupported ? "check-ok" : "check-bad"}>
              Windows 10/11
            </span>
            <span
              className={
                snapshot.versionManager.preflight.architectureSupported ? "check-ok" : "check-bad"
              }
            >
              {snapshot.versionManager.preflight.architecture}
            </span>
            <span className={snapshot.versionManager.preflight.webview2Available ? "check-ok" : "check-bad"}>
              WebView2
            </span>
            <span className={snapshot.versionManager.preflight.enoughDiskSpace ? "check-ok" : "check-bad"}>
              {formatBytes(snapshot.versionManager.preflight.freeBytes)}
            </span>
          </div>
          <div className="wizard-paths">
            <span>Runtime</span>
            <code>{snapshot.versionManager.preflight.runtimeRoot}</code>
            <span>缓存</span>
            <code>{snapshot.versionManager.preflight.cacheRoot}</code>
          </div>
          <div className="channel-picker" aria-label="版本通道">
            <button
              className={snapshot.versionManager.channel === "recommended" ? "channel-active" : ""}
              disabled={runtimeLocked}
              onClick={() => void chooseChannel("recommended")}
            >
              <strong>推荐版</strong>
              <span>npm latest · {snapshot.versionManager.recommendedVersion ?? "待查询"}</span>
            </button>
            <button
              className={snapshot.versionManager.channel === "alpha" ? "channel-active" : ""}
              disabled={runtimeLocked}
              onClick={() => void chooseChannel("alpha")}
            >
              <strong>Alpha 预览版</strong>
              <span>需主动选择 · {snapshot.versionManager.alphaVersion ?? "暂无"}</span>
            </button>
          </div>
          <button
            className="button button-primary wizard-install"
            disabled={!preflightReady || runtimeLocked || !selectedTarget}
            onClick={() => void installSelectedChannel()}
          >
            安装并验证 {selectedTarget ?? "所选版本"}
          </button>
          {snapshot.versionManager.busy && (
            <div className="operation-progress" aria-live="polite">
              <div>
                <span style={{ width: `${snapshot.versionManager.progress}%` }} />
              </div>
              <p>{snapshot.versionManager.message}</p>
            </div>
          )}
        </section>
      )}

      <section className="hero-card">
        <div className="logo-orbit" aria-hidden="true">
          <div className="orbit-ring" />
          <img src="/dsh-logo.svg" alt="" />
        </div>

        <div className="hero-content">
          <div className={`status-pill status-${snapshot.phase}`}>
            <span className="status-dot" />
            {phaseLabels[snapshot.phase]}
          </div>
          <h1>DeepSeek Harness</h1>
          <p className="version-line">
            DSH {snapshot.runtime.dshVersion ?? "—"}
            <span>·</span>
            Node {snapshot.runtime.nodeVersion ?? "—"}
          </p>
          <p className="status-detail">{statusDetail}</p>

          <div className="action-row">
            {snapshot.phase === "running" ? (
              <button
                className="button button-danger"
                disabled={!canStop || busy}
                onClick={() => void runLifecycleAction("stop_dsh")}
              >
                停止 DSH
              </button>
            ) : (
              <button
                className="button button-primary"
                disabled={!canStart || busy || isTransitioning}
                onClick={() => void runLifecycleAction("start_dsh")}
              >
                启动 DSH
              </button>
            )}
            <button
              className="button button-secondary"
              disabled={!canOpen || busy}
              onClick={() => void runLifecycleAction("open_dsh")}
            >
              打开 DSH 界面
            </button>
          </div>
        </div>
      </section>

      {(requestError || snapshot.lastError) && (
        <section className="error-banner" role="alert">
          <strong>操作未完成</strong>
          <span>{requestError ?? snapshot.lastError}</span>
        </section>
      )}

      <section className="content-grid">
        <article className="panel update-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">Runtime</span>
              <h2>安装与版本管理</h2>
            </div>
            <span className="phase-badge">第二阶段</span>
          </div>
          <div className="channel-toolbar">
            <div className="channel-tabs">
              <button
                className={snapshot.versionManager.channel === "recommended" ? "selected" : ""}
                disabled={runtimeLocked}
                onClick={() => void chooseChannel("recommended")}
              >
                推荐版
              </button>
              <button
                className={snapshot.versionManager.channel === "alpha" ? "selected" : ""}
                disabled={runtimeLocked}
                onClick={() => void chooseChannel("alpha")}
              >
                Alpha
              </button>
            </div>
            <button
              className="button button-ghost"
              disabled={snapshot.versionManager.busy || busy}
              onClick={() => void call("check_runtime_versions")}
            >
              检查版本
            </button>
          </div>
          <div className="runtime-summary">
            <div>
              <span>{snapshot.versionManager.channel === "alpha" ? "Alpha 目标" : "推荐目标"}</span>
              <strong>{selectedTarget ?? "当前无可用版本"}</strong>
            </div>
            <button
              className="button button-secondary"
              disabled={runtimeLocked || !selectedTarget || selectedInstalled}
              onClick={() => void installSelectedChannel()}
            >
              {selectedInstalled ? "已安装" : "旁路安装"}
            </button>
          </div>
          {snapshot.versionManager.message && (
            <div className="operation-status" aria-live="polite">
              {snapshot.versionManager.busy && (
                <progress max={100} value={snapshot.versionManager.progress} />
              )}
              <span>{snapshot.versionManager.message}</span>
            </div>
          )}
          <div className="version-list">
            {snapshot.versionManager.installedVersions.length === 0 ? (
              <p className="version-empty">尚无已验证 Runtime。</p>
            ) : (
              snapshot.versionManager.installedVersions.map((runtime) => (
                <div className="version-row" key={runtime.id}>
                  <div>
                    <strong>DSH {runtime.dshVersion}</strong>
                    <span>
                      Node {runtime.nodeVersion} · {runtime.managed ? "私有" : "已导入"} · {runtime.recipeId}
                    </span>
                  </div>
                  {runtime.active ? (
                    <span className="active-label">活动版本</span>
                  ) : (
                    <button
                      className="button button-ghost"
                      disabled={runtimeLocked || !runtime.smokeTested}
                      onClick={() => void switchVersion(runtime.id, runtime.dshVersion)}
                    >
                      切换
                    </button>
                  )}
                </div>
              ))
            )}
          </div>
          <button
            className="rollback-button"
            disabled={runtimeLocked || !snapshot.versionManager.previousVersion}
            onClick={() => void rollback()}
          >
            回滚到 {snapshot.versionManager.previousVersion ?? "上一版本"}
          </button>
        </article>

        <article className="panel environment-panel">
          <div className="panel-heading">
            <div>
              <span className="eyebrow">环境</span>
              <h2>活动路径</h2>
            </div>
            <span className="phase-badge">
              {snapshot.runtime.managedPrivate ? "私有 Runtime" : "兼容导入"}
            </span>
          </div>
          <dl className="path-list">
            <div>
              <dt>工作区</dt>
              <dd title={snapshot.runtime.workspace}>{snapshot.runtime.workspace || "未检测"}</dd>
            </div>
            <div>
              <dt>DSH_HOME</dt>
              <dd title={snapshot.runtime.dshHome}>{snapshot.runtime.dshHome || "未检测"}</dd>
            </div>
            <div>
              <dt>Node</dt>
              <dd title={snapshot.runtime.nodePath}>{snapshot.runtime.nodePath || "未检测"}</dd>
            </div>
            <div>
              <dt>版本目录</dt>
              <dd title={snapshot.versionManager.preflight.runtimeRoot}>
                {snapshot.versionManager.preflight.runtimeRoot || "未检测"}
              </dd>
            </div>
          </dl>
          <div className="safety-note">
            DSH_HOME 独立于版本目录；切换与回滚不会删除用户数据。Alpha 不会静默安装。
          </div>
        </article>
      </section>

      <section className="panel secure-update-panel" aria-labelledby="secure-update-title">
        <div className="panel-heading">
          <div>
            <span className="eyebrow">供应链</span>
            <h2 id="secure-update-title">签名自动更新</h2>
          </div>
          <span className="phase-badge">第三阶段</span>
        </div>
        <div className="secure-update-grid">
          <div>
            <strong>DSH Runtime</strong>
            <span>
              {snapshot.secureUpdate.configured
                ? snapshot.secureUpdate.availableVersion
                  ? `发现 ${snapshot.secureUpdate.availableVersion}`
                  : "Ed25519 manifest 已配置"
                : "发布端点尚未配置，本地构建安全关闭"}
            </span>
          </div>
          <div className="secure-update-actions">
            <button
              className="button button-ghost"
              disabled={!snapshot.secureUpdate.configured || snapshot.versionManager.busy || busy}
              onClick={() => void call("check_signed_runtime_update")}
            >
              校验更新
            </button>
            <button
              className="button button-secondary"
              disabled={!snapshot.secureUpdate.availableVersion || snapshot.versionManager.busy || busy}
              onClick={() => void call("download_signed_runtime_update")}
            >
              后台下载
            </button>
            <button
              className="button button-primary"
              disabled={!snapshot.secureUpdate.downloadedVersion || snapshot.versionManager.busy || busy}
              onClick={() => void applySignedUpdate()}
            >
              备份并切换
            </button>
          </div>
        </div>
        {snapshot.secureUpdate.totalBytes && snapshot.secureUpdate.status === "downloading" ? (
          <div className="operation-status" aria-live="polite">
            <progress
              max={snapshot.secureUpdate.totalBytes}
              value={snapshot.secureUpdate.downloadedBytes}
            />
            <span>{snapshot.versionManager.message}</span>
          </div>
        ) : null}
        {snapshot.secureUpdate.backupPath ? (
          <p className="backup-path" title={snapshot.secureUpdate.backupPath}>
            最近备份：{snapshot.secureUpdate.backupPath}
          </p>
        ) : null}
        <div className="launcher-update-row">
          <div>
            <strong>DSH Launcher</strong>
            <span>
              {snapshot.secureUpdate.launcherUpdateConfigured
                ? "Tauri 签名更新已配置"
                : "等待发布流水线注入端点与公钥"}
            </span>
          </div>
          <button
            className="button button-ghost"
            disabled={!snapshot.secureUpdate.launcherUpdateConfigured || launcherUpdateBusy}
            onClick={() => void updateLauncher()}
          >
            检查启动器更新
          </button>
        </div>
        {launcherUpdateProgress ? <p className="launcher-update-progress">{launcherUpdateProgress}</p> : null}
        <p className="supply-chain-note">
          客户端只接受内嵌 Ed25519 公钥验证的 manifest 与 Runtime Bundle；不会在更新时执行 npm
          install 或网络生命周期脚本。
        </p>
      </section>

      <section className="panel log-panel">
        <button className="log-toggle" onClick={() => setLogsExpanded((value) => !value)}>
          <span>
            <span className="eyebrow">诊断</span>
            <strong>启动器日志</strong>
          </span>
          <span>{logsExpanded ? "收起" : "展开"}</span>
        </button>
        {logsExpanded && (
          <div className="log-view" aria-live="polite">
            {snapshot.logs.length === 0 ? (
              <p className="log-empty">暂无日志。</p>
            ) : (
              snapshot.logs.map((entry, index) => (
                <div className={`log-line log-${entry.level}`} key={`${entry.timestampMs}-${index}`}>
                  <time>{formatTime(entry.timestampMs)}</time>
                  <span>{entry.level.toUpperCase()}</span>
                  <p>{entry.message}</p>
                </div>
              ))
            )}
          </div>
        )}
      </section>

      <footer>
        <span>DSH Launcher {launcherVersion ?? "—"}</span>
        <span>兼容 DeepSeek Harness · 非官方项目</span>
      </footer>
    </main>
  );
}
