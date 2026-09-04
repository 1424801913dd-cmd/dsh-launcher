# Proposed code signing policy

## Status

This policy describes the prepared signing design; it is **not currently active**. As of 2026-09-04, the SignPath Foundation
application has not been approved because the project does not yet have sufficient public trust and visibility signals.
No current artifact may claim a SignPath Foundation signature. The signed release workflow remains fail-closed until an
approved signing provider supplies the real project configuration.

If a future SignPath Foundation application is approved, the required attribution for releases signed under that program will
be: Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by
[SignPath Foundation](https://signpath.org/).

## Roles

- Authors: [repository contributors](https://github.com/1424801913dd-cmd/dsh-launcher/graphs/contributors)
- Reviewers: [repository owner](https://github.com/1424801913dd-cmd)
- Approvers: [repository owner](https://github.com/1424801913dd-cmd)

The same person may currently hold more than one role because this is a single-maintainer project. If SignPath signing is
activated, release approval will remain a separate, explicit SignPath action. Multi-factor authentication is required for the
repository and any future signing-service account.

## Planned trusted build and signing boundary

- Future release signing requests will be accepted only from the repository's GitHub Actions trusted build integration.
- The signed release job accepts only the `main` branch and serializes release runs so two approvals cannot race.
- The workflow builds from the selected repository commit and locked dependencies; locally supplied binaries are not eligible.
- Runtime license review runs before either Authenticode signing request.
- CI first performs a discardable unsigned NSIS prebundle so Tauri finalizes the executable's bundle metadata. SignPath then
  signs `dsh-launcher.exe`; CI rebundles that signed executable into NSIS, and SignPath signs the final installer in a second
  request.
- CI verifies the SignPath Foundation Authenticode signer and trusted timestamp on both artifacts before creating the Tauri
  updater archive, update signature, or draft GitHub Release.
- Every future release signing request requires manual approval in SignPath. A successful signature does not by itself authorize
  publication of the GitHub draft release.
- Runtime and updater private keys are scoped only to their individual signing steps; dependency installation, compilation,
  SignPath submission, and installer testing do not receive those private-key environment variables.

## Privacy and release safety

The project privacy policy is [docs/PRIVACY.md](PRIVACY.md). The application has no telemetry or analytics service and does
not upload DSH credentials or complete session content. User-requested update and installation operations can contact the
documented GitHub, Node.js, and npm endpoints. DSH itself, model providers, and user-installed plugins are separate components
and can have their own privacy policies.

The project is unofficial and independently maintained. It is not reviewed, sponsored, or endorsed by DeepSeek. Signing
must not use DeepSeek as the publisher identity.
