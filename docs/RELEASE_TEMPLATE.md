# DSH Launcher {{LAUNCHER_VERSION}}

> Unofficial and independently maintained; not reviewed, sponsored, or endorsed by DeepSeek.

DSH Launcher is a local Windows lifecycle manager compatible with DeepSeek Harness (`dsh`). DeepSeek Harness is currently a developer preview and may introduce compatibility-breaking changes.

## This release

- Launcher version: `{{LAUNCHER_VERSION}}`
- Managed Runtime channel: `{{RUNTIME_CHANNEL}}`
- Managed DSH version: `{{DSH_VERSION}}`
- Platform: Windows 10/11 x64, current-user installation
- Runtime and launcher updates are accepted only after their independent signatures and pinned metadata validate.
- Runtime installation uses a reviewed full dependency lock; the client does not run npm installation or lifecycle scripts.

## Install and data

Download the Windows x64 NSIS setup executable from this draft's assets. Before promoting the draft, the release owner must verify a valid Authenticode chain and RFC 3161 timestamp on both the installer and internal executable, then record clean Windows 10/11 installation results and the observed SmartScreen state.

Uninstalling the launcher removes the application and shortcuts but intentionally preserves the user's independent DSH data directory. The launcher provides no telemetry or analytics service and does not read or upload DSH credentials or complete session content. See `PRIVACY.md` in the installed `resources` directory.

## Licenses and source

DSH Launcher is MIT licensed. `THIRD_PARTY_NOTICES.md` and `RELEASE_REVIEW.md` are included in the installed `resources` directory. Each separately signed Runtime bundle includes its actual Windows x64 dependency notice, Node.js license, GNU GPL/LGPL texts, native package notices, and a versioned source/build-material index.

For the Runtime's libvips Windows x64 static DLL family, a corresponding-source archive is attached to this release as `lgpl-source-materials-vips-8.18.6.tar.gz` (plus `lgpl-source-materials-vips-8.18.6.tar.gz.json` with its SHA-256 and build provenance). See [docs/LGPL_REVIEW.md](https://github.com/1424801913dd-cmd/dsh-launcher/blob/main/docs/LGPL_REVIEW.md) for the manual review record.

This draft must remain unpublished while any release quality gate reports `FAIL`.

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/). Builds are submitted from this repository's GitHub Actions trusted build;
every release requires explicit SignPath approval. See
[docs/CODE_SIGNING_POLICY.md](https://github.com/1424801913dd-cmd/dsh-launcher/blob/main/docs/CODE_SIGNING_POLICY.md).
