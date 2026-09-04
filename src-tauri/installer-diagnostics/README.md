# Read-only installer diagnostic

Derived from Tauri CLI 2.11.4's MIT-licensed NSIS template:
https://github.com/tauri-apps/tauri/blob/tauri-cli-v2.11.4/crates/tauri-bundler/src/bundle/windows/nsis/installer.nsi

The normal application configuration does not select this template. Only
`tauri.diagnostic.conf.json` does. Product name, manufacturer, current-user
registry identity, version comparison and maintenance-page choices are retained
to observe the reported 0.4.1 behavior. The label `older` is an age description,
not a literal installed version; the log records the actual DisplayVersion.

All installation sections, all uninstall execution and shortcut/app launch
functions are blocked. No application binary is installed or launched. The only
persistent output is a new PID-named local diagnostic log beside the probe EXE;
NSIS may also extract its own temporary plugins. A pre-existing log is not
overwritten. Logs include account name and registry path/command values: review
and redact them before sharing. Do not treat log values as commands to execute.

This is an evidence-gathering artifact, not a fix or a normal installation
candidate. Never upload it as a Release or replace `trial-candidate.json` with it.
The normal installer and prior trial ZIPs remain unchanged.
