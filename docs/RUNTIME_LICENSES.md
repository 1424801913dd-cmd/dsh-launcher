# Managed Runtime license materials

The managed DSH Runtime is a separately signed bundle containing Node.js, DeepSeek Harness, and its production npm dependency tree. It is not covered only by the DSH Launcher MIT license.

Each published Runtime bundle must contain:

- `node/LICENSE`, supplied by the pinned official Node.js distribution;
- `app/THIRD_PARTY_NOTICES.md`, generated from the package directories actually installed for Windows x64;
- `app/licenses/GPL-3.0.txt` and `app/licenses/LGPL-3.0.txt`, downloaded from the official GNU license endpoints during the controlled build;
- the original license, README, version manifest, DLL files, and native addon retained inside `app/node_modules/@img/sharp-win32-x64`;
- this file as `app/RUNTIME_LICENSES.md`.

For the exact installed `@img/sharp-win32-x64@{{SHARP_VERSION}}` binary family, reproducible packaging and build material is published at:

- sharp-libvips v{{SHARP_LIBVIPS_VERSION}} source and Windows build scripts: <https://github.com/lovell/sharp-libvips/tree/v{{SHARP_LIBVIPS_VERSION}}>
- libvips Windows MXE build source: <https://github.com/libvips/build-win64-mxe/releases/download/v8.18.6/vips-dev-x64-web-8.18.6-static.zip> (build recipes: <https://github.com/libvips/build-win64-mxe/tree/v8.18.6>)
- MXE fork base used by the build (llvm-mingw toolchain plugin): <https://github.com/kleisauke/mxe/tree/llvm-mingw-20260605>
- GNU LGPL version 3: <https://www.gnu.org/licenses/lgpl-3.0.html>
- GNU GPL version 3: <https://www.gnu.org/licenses/gpl-3.0.html>

The corresponding-source archive for the exact DLL family is produced by `scripts/prepare-lgpl-source-materials.ps1` from the review-verified manifest `scripts/data/lgpl-source-manifest.json`; the 2026-09-02 review record is [LGPL_REVIEW.md](LGPL_REVIEW.md). The upstream npm package `@img/sharp-win32-x64` ships only the Apache-2.0 license text; the GPL-3.0 and LGPL-3.0 texts above and the per-component `/licenses` materials in this bundle cover the LGPL side.

The sharp native addon loads libvips DLL files separately. DSH Launcher verifies a Runtime bundle at download and installation time but does not pin native DLL hashes after installation, so an interface-compatible replacement is not blocked by the launcher.

The LGPL manual review completed on 2026-09-02 verified all four release-review items. The corresponding-source archive (all 9 statically included / MPL components, including cairo-1.18.4) has been generated, its SHA-256 recorded in the companion `.json` report, and its generation/upload is wired into the release workflow. `scripts/license-audit.mjs` records the sharp disposition as `documented-source-availability` only while that archive report is present, lists no pending sources, and the archive file hash matches the report; otherwise the package returns to `manual-review-required` and the license gate fails closed.

Corresponding-source archive: `release-assets/lgpl-source-materials-vips-8.18.6.tar.gz`, SHA-256 `5A6B85A33DA69292A08401C14279DDC3863EF678FE04FBA3E391F331B6981B1C`.
