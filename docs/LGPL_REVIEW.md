# DSH Launcher – sharp/libvips LGPL 人工复核备忘录

复核日期：2026-09-02。本文是发布工程记录，不是法律意见；对应 `docs/RELEASE_REVIEW.md`
中 `@img/sharp-win32-x64` 项的 `manual-review-required` 处置。证据全部来自本机只读检查与
本地证据缓存 `.tmp-lgpl-review`（已加入 `.gitignore`，不提交仓库）。

## 一、交付链溯源

实际携带的 `@img/sharp-win32-x64@0.35.4`（安装于活动 Runtime
`app/node_modules/@img/sharp-win32-x64`）包含：

| 文件 | 说明 |
| --- | --- |
| `lib/libvips-42.dll`（18,614,784 B） | libvips 本体，Windows x64 web 变体 |
| `lib/libvips-cpp-8.18.6.dll`（333,312 B） | C++ 绑定层 |
| `lib/sharp-win32-x64-0.35.4.node`（442,368 B） | Node 原生插件 |
| `LICENSE` | 仅 Apache-2.0 全文（见第三节项 1 观察） |
| `README.md`、`versions.json`、`package.json` | `package.json` 声明 `Apache-2.0 AND LGPL-3.0-or-later` |

`versions.json` 记录 29 个组件的精确版本（`vips` 8.18.6、`glib` 2.89.4、`pango` 1.58.2、
`fribidi` 1.0.16、`exif` 0.6.26、`heif` 1.23.2、`rsvg` 2.62.91、`proxy-libintl` 0.5、
`cairo` 1.18.4 等），是“包内实际是什么”的权威记录。

构建链（三层，全部有本地快照）：

```text
libvips 8.18.6 + 依赖（上游源码，MXE 配方固定版本与 SHA-256）
  -> libvips/build-win64-mxe v8.18.6
       build.sh + container/* + build/overrides.mk + build/plugins/* + build/patches/*
       MXE 环境：kleisauke/mxe 分支 llvm-mingw-20260605（凭 plugins/llvm-mingw 识别），
       本地快照 mxe-d973945bb92c7783d5afa41bb2b8d2e1a04eaba3
  -> vips-dev-x64-web-8.18.6-static.zip（“static”= 依赖静态并入；vips 本体始终为 DLL，
      见 build/vips.mk 注释 “Always build as shared library, we need libvips-42.dll”）
  -> lovell/sharp-libvips v1.3.3 的 build/win.sh（下载 zip、整理 lib/、附 THIRD-PARTY-NOTICES）
  -> @img/sharp-win32-x64@0.35.4 npm 包
```

## 二、组件许可证矩阵（web 变体）

依据 build-win64-mxe v8.18.6 `README.md` 的 “libvips-web dependencies” 表（与 `versions.json`
一致；注意 sharp-libvips v1.3.3 的 `versions.properties` 中 `VERSION_AOM=3.15.0` 与实际安装
`aom 3.14.1` 不一致，以安装包 `versions.json` 与 build-win64-mxe README 为准，aom 为 BSD+专利声明，
无 LGPL 影响）：

| 组件 | 版本 | 上游口径 | 随附许可文本 |
| --- | --- | --- | --- |
| libvips | 8.18.6 | LGPL（npm 声明 LGPL-3.0-or-later） | 源码 `LICENSE` = LGPL-2.1 文本 |
| glib | 2.89.4 | LGPL | 源码 `COPYING` |
| pango | 1.58.2 | LGPL | 源码 `COPYING`（Library GPL v2, June 1991） |
| fribidi | 1.0.16 | LGPL | 源码 `COPYING` = LGPL-2.1 文本 |
| libexif | 0.6.26 | LGPL | 源码 `COPYING` = LGPL-2.1 文本 |
| librsvg | 2.62.91 | LGPL | 源码 `COPYING.LIB` = LGPL-2.1 文本 |
| libheif | 1.23.2 | LGPLv3 | 源码 `COPYING` + 头部声明 |
| proxy-libintl | 0.5 | LGPL | 源码 `COPYING` |
| cairo | 1.18.4 | MPL 2.0（上游选择该路径） | 源码 `COPYING` |
| 其余 20 项 | — | BSD/MIT/zlib 类 | 随各自源码 |

## 三、四项检查结论

### 1. 安装介质和已安装产品随附 GPL/LGPL 全文与上游第三方声明 —— 证据成立（以新构建为准）

- Runtime Bundle 构建脚本 `scripts/Build-RuntimeBundle.ps1`（113–128 行）在受控构建时从
  gnu.org 下载 `GPL-3.0.txt`、`LGPL-3.0.txt` 写入 `app/licenses/`，并把
  `docs/RUNTIME_LICENSES.md` 实例化为 `app/RUNTIME_LICENSES.md`（替换 sharp 与
  sharp-libvips 精确版本）；139–145 行生成 `app/THIRD_PARTY_NOTICES.md`；
- 安装器资源已映射 MIT `LICENSE`、`PRIVACY.md`、`RELEASE_REVIEW.md`、`THIRD_PARTY_NOTICES.md`
  （见 HANDOFF 二十一.8），但 LGPL 义务属于 Runtime 一侧，由 Bundle 承担；
- **观察 1**：上游 npm 包内的 `LICENSE` 只有 Apache-2.0 全文，没有 LGPL 文本；
- **观察 2（必须处理）**：当前活动 Runtime `signed-0.1.2-alpha.2` 的 `app/` 下没有
  `licenses/`、`RUNTIME_LICENSES.md`、`THIRD_PARTY_NOTICES.md`——它早于上述机制。下一个
  发布 Bundle 必须按上述机制重建并用 `phase4-check.ps1` 门禁重新证明；
- 各组件自身的 COPYING/LICENSE 随源码归档（第四节），构成深色矩阵的原文依据。

### 2. 用户替换接口兼容 DLL 不会被完整性校验、更新器或签名策略阻止 —— 证据成立

- PE 导入表（可重复证据：`scripts/runtime-license-evidence.ps1`，报告
  `phase4-results/runtime-license-evidence.json`）：`sharp-win32-x64-0.35.4.node` 仅导入
  `libvips-42.dll` 与 `libvips-cpp-8.18.6.dll`，后者仅导入 `libvips-42.dll`；
- Rust 单元测试 `installed_runtime_record_does_not_pin_replaceable_native_library_hashes`
  （`src-tauri/src/runtime_manager.rs:1235`）验证替换 DLL 后 Runtime 记录仍然有效；
- 更新器只对整个 Bundle ZIP 做签名/摘要与 manifest 校验，安装后不锚定原生 DLL 哈希；
  Authenticode 只覆盖启动器 EXE/安装器，不触及 Runtime DLL。

### 3. 静态并入组件的源码、构建脚本及必要的重链接材料 —— 证据成立（含 1 项残余）

- **源码精确匹配**：8 个 LGPL 类组件源码 tarball 的 SHA-256 与 build-win64-mxe v8.18.6
  配方记录完全一致（本机逐一计算比对）：

| 组件 | 记录来源 | SHA-256（前 16 位） |
| --- | --- | --- |
| vips 8.18.6 | build/vips.mk | `3C41E1D5458081BF…` |
| glib 2.89.4 | build/overrides.mk | `1CDBB799F558832E…` |
| pango 1.58.2 | build/overrides.mk | `342385B6CA3B7C73…` |
| fribidi 1.0.16 | build/overrides.mk | `1B1CDE5B235D4047…` |
| libexif 0.6.26 | build/overrides.mk | `4A055ED6575E61CA…` |
| librsvg 2.62.91 | build/overrides.mk | `6CAEAE129D40DD88…` |
| libheif 1.23.2 | build/libheif.mk | `8BD5D41D19DC8453…` |
| proxy-libintl 0.5 | plugins/proxy-libintl/proxy-libintl.mk | `F7A1CBD7579BAAF5…` |

  （完整哈希见 `scripts/data/lgpl-source-manifest.json`。cairo 1.18.4 的校验和来自 MXE 配方
  `src/cairo.mk`；2026-09-02 已按其下载并核验通过，现随本节项 4 的归档一并内置。）
- **构建脚本**：build-win64-mxe v8.18.6（build.sh、container、各个 `.mk` 与 `patches/`）、
  MXE 快照（含全部组件配方 URL/校验和/补丁）、sharp-libvips v1.3.3 `build/win.sh`；
- **重链接材料**：`vips-dev-x64-web-*-static.zip` 随附 `include/` 头文件、`lib/*.lib` 导入库
  与 DLL；`libvips-42.dll` 是可由用户替换的独立共享库，且 header/import lib 允许下游重建
  绑定（替换路径见项 2）；
- **残余**：build-win64-mxe `container/base.Dockerfile` 第 14 行以**分支**（
  `llvm-mingw-20260605`）而非 commit 固定 MXE，且默认使用 `ghcr.io/libvips/build-win64-mxe:latest`
  预构建镜像；因此“发布时”的 MXE 工具链 commit 与 2026-09-02 快照可能不同。LXGPL 组件的
  源码身份由配方校验和完全锁定（不受 MXE 影响），工具链快照的差异属于构建可复现性风险，
  已通过归档快照并记录 commit 缓解。

### 4. Release 页面给出对应版本源码/构建材料的长期可用地址或随包归档 —— 已采纳随包归档

- **推荐方案（随包归档）**：`scripts/prepare-lgpl-source-materials.ps1`
  + `scripts/data/lgpl-source-manifest.json` 生成归档（2026-09-02 已产出并含全部 9 个组件源码）：

  | 文件 | 值 |
  | --- | --- |
  | `release-assets/lgpl-source-materials-vips-8.18.6.tar.gz` | 79,683,793 字节 |
  | SHA-256 | `5A6B85A33DA69292A08401C14279DDC3863EF678FE04FBA3E391F331B6981B1C` |
  | 内容 | 9 个已核验源码（含 cairo）+ 3 个构建链快照 + `PROVENANCE.md` + `SHA256SUMS.txt`（报告 `pendingSources: []`） |

  接线（2026-09-02）：release.yml 在“Enforce Runtime license review gate”前新增
  “Prepare LGPL source-materials archive” 步骤调用本脚本，并把归档与其报告 JSON 加入 draft
  Release 上传列表；`docs/RELEASE_TEMPLATE.md` 已加引用；
- 次选（仅不可变链接 + 校验和）：较弱，LGPL 3.0 对“网络获取”的长期可用性要求建议不要
  作为唯一依据；若采用，也必须与归档脚本产出的 SHA-256 清单联动。

## 四、结论与建议处置

1. 第 1–3 项证据成立；保留两项必要动作：新 Bundle 必须以新机制重建后由门禁再验证（项 1）；
   MXE 工具链快照以 commit 记录（项 3）；
2. 第 4 项**已采纳“随包归档”**并接线 release.yml；cairo 已于 2026-09-02 补取核验（#4 完成）。
   **#5 已于 2026-09-02 落地**：`license-audit.mjs` 的处置改为机械判定——存在完整归档报告
   （`pendingSources: []` 且归档文件 SHA-256 与报告一致）时才记为
   `documented-source-availability`，否则回到 `manual-review-required`。验证：本机审计
   `review=6, unresolved=0`；硬闸门 `phase4-check.ps1 -RequireAuthenticode -RequireInstaller
   -RequireLicenseReview` 中 `license-obligations-review` 已 PASS，仅剩未签名 FAIL（报告
   `phase4-results/release-gate-lgpl-review.json`）；
3. 建议把“归档清单 SHA-256 与 Release 已上传资产一致”作为发布后抽检项（当前门禁在生成时校验，
   上传由 release.yml 固定引用同一文件）；
4. `docs/RUNTIME_LICENSES.md` 免责条款已按本次决定更新。

## 五、证据保留

- `.tmp-lgpl-review/`（约 77 MiB，已 gitignore）：任务已收尾，可删除；若保留，注意归档与
  报告 JSON 在 `release-assets/`；
- 可重复脚本：`scripts/runtime-license-evidence.ps1`（PE 导入表）、
  `scripts/prepare-lgpl-source-materials.ps1`（归档与校验和）；
- 复核结论的机器核对记录：`phase4-results/`（已 gitignore）。
