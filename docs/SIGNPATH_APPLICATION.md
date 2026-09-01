# SignPath Foundation application record

This page collects the project facts needed for the SignPath Foundation application. It is an engineering record, not an
approval from SignPath Foundation.

## Project

- Name: DSH Launcher
- Repository and project homepage: <https://github.com/1424801913dd-cmd/dsh-launcher>
- License: [MIT](../LICENSE)
- Releases: <https://github.com/1424801913dd-cmd/dsh-launcher/releases>
- Code signing policy: [CODE_SIGNING_POLICY.md](CODE_SIGNING_POLICY.md)
- Privacy policy: [PRIVACY.md](PRIVACY.md)
- Security and release process: [RELEASE.md](RELEASE.md)
- Maintainer/contact account: <https://github.com/1424801913dd-cmd>

DSH Launcher is an unofficial Windows launcher and lifecycle manager compatible with DeepSeek Harness. It is independently
maintained and is not reviewed, sponsored, or endorsed by DeepSeek.

## Eligibility statements to confirm in the application

- The project is distributed under the OSI-approved MIT license and is not commercially dual licensed.
- The repository contains no proprietary project component. Redistributed upstream dependencies retain their own licenses and
  notices; release remains blocked while the recorded sharp/libvips LGPL review is unresolved.
- The project is actively maintained, documented, and has existing Windows releases in the form that will be signed.
- The installer provides a standard uninstaller and preserves independent user data to prevent accidental data loss.
- GitHub and SignPath access for authors, reviewers, and approvers uses multi-factor authentication.
- Only artifacts built from this public repository by the configured GitHub Actions trusted build system may be signed.
- Every release signing request requires manual approval.

## Requested SignPath configuration

Create one project and one release signing policy using the SignPath Foundation certificate and trusted timestamping. Configure
two artifact configurations because the signed launcher must be embedded before the final NSIS installer is built:

1. Executable configuration: input contains exactly one root file named `dsh-launcher.exe`; Authenticode-sign that PE file.
2. Installer configuration: input contains exactly one root file matching `DSH.Launcher_*_x64-setup.exe`; Authenticode-sign
   that PE file.

Bind the project to the GitHub trusted build system for `1424801913dd-cmd/dsh-launcher`. After SignPath supplies the real
identifiers, store the API token as GitHub Secret `SIGNPATH_API_TOKEN` and store the five non-secret identifiers listed in
`docs/RELEASE.md` as GitHub Actions Variables. Do not store the token in a variable, workflow file, issue, or build artifact.

## Manual boundary

The repository owner must submit the application, accept the SignPath terms, enable MFA, and later approve release signing
requests in SignPath. CI deliberately fails before building if the SignPath values are absent, and it never falls back to an
unsigned public release.
