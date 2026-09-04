import assert from "node:assert/strict";
import test from "node:test";
import { captureInstallRequest, selectedRuntimeVersion, unqueriedVersions } from "../src/runtimeSelection.ts";

test("startup/failed query never offers hard-coded versions", () => {
  const manager = { channel: "recommended", ...unqueriedVersions, busy: false };
  assert.equal(selectedRuntimeVersion(manager), null);
  assert.throws(() => captureInstallRequest(manager));
  manager.recommendedVersion = "0.1.1-rc.2";
  assert.throws(() => captureInstallRequest(manager)); // stale data without a successful check
});

for (const channel of ["recommended", "alpha"]) {
  test(`${channel}: immutable exact version survives snapshot refresh`, () => {
    const manager = { channel, recommendedVersion: "0.1.1-rc.2", alphaVersion: "0.1.2-alpha.2", lastCheckedMs: 123, busy: false };
    const expected = selectedRuntimeVersion(manager);
    const request = captureInstallRequest(manager);
    manager.recommendedVersion = "0.1.2-rc.1";
    manager.alphaVersion = "0.1.2-alpha.5";
    manager.channel = channel === "alpha" ? "recommended" : "alpha";
    assert.deepEqual(request, { channel, expectedVersion: expected });
    assert.ok(Object.isFrozen(request));
  });
}

test("missing tag and in-flight operations block install/retry", () => {
  const manager = { channel: "alpha", alphaVersion: null, lastCheckedMs: 123, busy: false };
  assert.throws(() => captureInstallRequest(manager));
  manager.alphaVersion = "0.1.2-alpha.2";
  manager.busy = true;
  assert.throws(() => captureInstallRequest(manager));
});
