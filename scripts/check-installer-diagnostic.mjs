import fs from 'node:fs';
import assert from 'node:assert/strict';
const rawSource = fs.readFileSync('src-tauri/installer-diagnostics/installer.nsi', 'utf8');
const config = JSON.parse(fs.readFileSync('src-tauri/tauri.diagnostic.conf.json', 'utf8'));
assert.equal(config.bundle.windows.nsis.template, 'installer-diagnostics/installer.nsi');
assert.equal(config.bundle.windows.nsis.displayLanguageSelector, false);
assert.equal(config.bundle.windows.nsis.installMode, 'currentUser');
assert.equal(config.bundle.createUpdaterArtifacts, false);
function assertSafe(raw) {
const source = raw.replace(/\r\n/g, '\n');
assert.doesNotMatch(source, /^\s*(?:Exec\w*|WriteReg\w*|DeleteReg\w*|CreateShortCut|RMDir|Delete|CopyFiles|WriteUninstaller|File|WriteINIStr)\s/gm);
for (const name of ['EarlyChecks', 'WebView2', 'Install']) {
  const body = source.match(new RegExp(`Section ${name}\\n([\\s\\S]*?)SectionEnd`))?.[1];
  assert.ok(body?.includes('action=install-blocked'));
  assert.ok(body?.includes('Quit'));
}
assert.match(source, /Section Uninstall\s+Abort\s+SectionEnd/);
assert.match(source, /action=uninstall-blocked/);
assert.match(source, /decision=no-existing-install/);
assert.match(source, /decision=maintenance-page/);
assert.match(source, /IfFileExists "\$DiagnosticPath" diag_open_failed/);
for (const hive of ['HKCU','HKLM']) for (const view of [32,64]) {
  for (const field of ['uninstallDefault','displayVersion','uninstallString','installLocation']) {
    assert.ok(source.includes(`observe=${hive}/${view}/${field};`));
  }
}
}
assertSafe(rawSource);
assertSafe(rawSource.replace(/\r?\n/g, '\r\n'));
assert.throws(() => assertSafe(rawSource + '\nExecWait "must-not-run.exe"\n'));
assert.throws(() => assertSafe(rawSource.replace('action=uninstall-blocked', 'action=unsafe')));
console.log('PASS: diagnostic template has no install/uninstall/registry-write commands; sections block, all four registry views logged.');
