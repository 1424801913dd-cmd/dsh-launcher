export type DshPhase =
  | "notInstalled"
  | "stopped"
  | "starting"
  | "running"
  | "stoppingGracefully"
  | "forceStopping"
  | "updating"
  | "rollbackRequired"
  | "crashed"
  | "externalServiceDetected";

export type RuntimeChannel = "recommended" | "alpha";

export interface RuntimeInfo {
  installed: boolean;
  runtimeId: string | null;
  channel: RuntimeChannel | null;
  managedPrivate: boolean;
  nodePath: string;
  nodeVersion: string | null;
  dshEntry: string;
  dshVersion: string | null;
  dshHome: string;
  workspace: string;
}

export interface InstalledRuntime {
  id: string;
  dshVersion: string;
  nodeVersion: string;
  channel: RuntimeChannel;
  recipeId: string;
  managed: boolean;
  smokeTested: boolean;
  active: boolean;
  installedAtMs: number;
}

export interface PreflightInfo {
  windowsSupported: boolean;
  architecture: string;
  architectureSupported: boolean;
  webview2Available: boolean;
  freeBytes: number | null;
  enoughDiskSpace: boolean;
  runtimeRoot: string;
  cacheRoot: string;
}

export interface VersionManagerSnapshot {
  channel: RuntimeChannel;
  recommendedVersion: string | null;
  alphaVersion: string | null;
  activeVersion: string | null;
  previousVersion: string | null;
  installedVersions: InstalledRuntime[];
  firstRunRequired: boolean;
  busy: boolean;
  operation: string | null;
  progress: number;
  message: string | null;
  lastCheckedMs: number | null;
  preflight: PreflightInfo;
}

export interface SecureUpdateSnapshot {
  configured: boolean;
  status:
    | "disabled"
    | "idle"
    | "checking"
    | "available"
    | "current"
    | "downloading"
    | "ready"
    | "applying"
    | "applied"
    | "rolled-back"
    | "rollback-required"
    | "error";
  availableVersion: string | null;
  downloadedVersion: string | null;
  downloadedBytes: number;
  totalBytes: number | null;
  lastCheckedMs: number | null;
  backupPath: string | null;
  launcherUpdateConfigured: boolean;
}

export interface LogEntry {
  timestampMs: number;
  level: "info" | "warn" | "error";
  message: string;
}

export interface LauncherSnapshot {
  phase: DshPhase;
  runtime: RuntimeInfo;
  webUrl: string | null;
  pid: number | null;
  lastError: string | null;
  logs: LogEntry[];
  versionManager: VersionManagerSnapshot;
  secureUpdate: SecureUpdateSnapshot;
}
