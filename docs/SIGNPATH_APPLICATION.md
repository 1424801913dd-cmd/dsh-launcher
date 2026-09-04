# SignPath Foundation application record

This page records the project facts and the outcome of the SignPath Foundation application. It is an engineering record, not
an approval from or endorsement by SignPath Foundation.

## Current status

As of 2026-09-04, the application has **not been approved**. SignPath Foundation explained that the project does not yet show
enough external signals of public trust and visibility for the Foundation to issue a certificate in its name. Examples named
in the response included community adoption, independent articles or discussions, institutional backing, and evidence of
sustained activity and engagement.

The response did not identify a defect in the source code, licensing controls, privacy policy, or proposed trusted-build
design. The project may reapply after it has gained broader independent recognition. Until approval, no release may claim to
be signed by SignPath Foundation and the SignPath-dependent release workflow remains unavailable.

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

## Eligibility record

- The project is distributed under the OSI-approved MIT license and is not commercially dual licensed.
- The repository contains no proprietary project component. Redistributed upstream dependencies retain their own licenses and
  notices. The recorded sharp/libvips LGPL review was completed on 2026-09-02; every future Runtime bundle must still reproduce
  the required source archive and pass the license gate.
- The project is actively maintained and documented, but it does not yet have the established public release history,
  independent adoption, or community participation required for a successful Foundation application.
- The installer provides a standard uninstaller and preserves independent user data to prevent accidental data loss.
- GitHub and SignPath access for authors, reviewers, and approvers uses multi-factor authentication.
- If a signing subscription is approved in the future, only artifacts built from this public repository by the configured
  GitHub Actions trusted build system may be signed.
- Every future release signing request will require manual approval.

## Proposed SignPath configuration

If a future application is approved, create one project and one release signing policy using the SignPath Foundation
certificate and trusted timestamping. Configure two artifact configurations because the signed launcher must be embedded
before the final NSIS installer is built:

1. Executable configuration: input contains exactly one root file named `dsh-launcher.exe`; Authenticode-sign that PE file.
2. Installer configuration: input contains exactly one root file matching `DSH.Launcher_*_x64-setup.exe`; Authenticode-sign
   that PE file.

Bind the project to the GitHub trusted build system for `1424801913dd-cmd/dsh-launcher`. Only after SignPath supplies the real
identifiers, store the API token as GitHub Secret `SIGNPATH_API_TOKEN` and store the five non-secret identifiers listed in
`docs/RELEASE.md` as GitHub Actions Variables. Do not store the token in a variable, workflow file, issue, or build artifact.

## Reapplication evidence

Do not resubmit the same evidence or treat elapsed time alone as sufficient. Before reapplying, collect verifiable public
evidence such as:

- multiple usable Windows releases with clear source commits, checksums, installation instructions, and release notes;
- sustained maintenance through issues, fixes, and releases rather than a short initial commit burst;
- genuine users, downloads, stars, forks, contributors, and issue or discussion participation;
- independent articles, videos, forum discussions, or references from projects and communities not controlled by the
  maintainer;
- any legitimate institutional or upstream recognition, without implying endorsement that was not explicitly granted.

Do not purchase, exchange, or fabricate stars, downloads, reviews, contributors, or external references.

## Manual boundary and next application

The repository owner must decide when the public evidence is mature enough to reapply, submit the new application, accept the
SignPath terms, enable MFA, and later approve release signing requests in SignPath. CI deliberately fails before building if
the SignPath values are absent, and it never falls back to an unsigned public release.
