# DeepSeek Harness rollback notes

> Legacy note: this file describes the pre-managed D-drive installation only. New DSH Launcher installations use the current
> user's `LOCALAPPDATA` directories, while an existing legacy installation is preserved in place. Do not apply the paths below
> to a new installation.

The desktop launcher currently targets DSH `0.1.1-rc.2`.

Runtime packages are intentionally not stored in Git. They contain tens of
thousands of generated dependency files and native binaries. User data under
`D:\Caches\deepseek-harness\home` must also stay out of Git because it may
contain sessions and credentials.

## Restore the previous version

Run the following commands in PowerShell:

```powershell
$node = 'D:\Tools\node-v24.19.0-win-x64\node.exe'
$npm = 'D:\Tools\node-v24.19.0-win-x64\node_modules\npm\bin\npm-cli.js'
$target = 'D:\Tools\dsh-runtime-0.1.0-rc.6'

& $node $npm install `
  --prefix $target `
  --cache 'D:\Caches\npm' `
  '@deepseek-ai/dsh@0.1.0-rc.6' `
  --no-audit `
  --no-fund
```

Then change `dshEntry` in `DeepSeek-Harness-Launcher.vbs` from
`dsh-runtime-0.1.1-rc.2` to `dsh-runtime-0.1.0-rc.6` and restart DSH.

The local pre-upgrade user-data backup is recorded in:
`D:\Caches\deepseek-harness\latest-pre-upgrade-backup.txt`.
