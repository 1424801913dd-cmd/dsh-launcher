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
- libvips Windows MXE build source: <https://github.com/libvips/build-win64-mxe>
- GNU LGPL version 3: <https://www.gnu.org/licenses/lgpl-3.0.html>
- GNU GPL version 3: <https://www.gnu.org/licenses/gpl-3.0.html>

The sharp native addon loads libvips DLL files separately. DSH Launcher verifies a Runtime bundle at download and installation time but does not pin native DLL hashes after installation, so an interface-compatible replacement is not blocked by the launcher.

This distribution note does not assert that source links alone satisfy every relinking obligation for LGPL components statically included in the upstream Windows libvips DLL. Before public release, the release owner must preserve or archive all corresponding source, build scripts, and any additional relinking material required for the exact binary set.
