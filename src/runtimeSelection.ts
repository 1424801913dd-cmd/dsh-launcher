import type { VersionManagerSnapshot } from "./types";

export const unqueriedVersions = {
  recommendedVersion: null,
  alphaVersion: null,
  lastCheckedMs: null,
};

export function selectedRuntimeVersion(manager: VersionManagerSnapshot): string | null {
  if (manager.lastCheckedMs === null) return null;
  return manager.channel === "alpha" ? manager.alphaVersion : manager.recommendedVersion;
}

// Copy primitive values at click time: later polling must never mutate the request.
export function captureInstallRequest(manager: VersionManagerSnapshot) {
  const expectedVersion = selectedRuntimeVersion(manager);
  if (manager.busy || !expectedVersion) {
    throw new Error("请先成功查询并确认目标版本，再开始安装。");
  }
  return Object.freeze({ channel: manager.channel, expectedVersion });
}
