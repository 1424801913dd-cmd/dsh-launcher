# DSH Launcher 发布身份与许可证复核

更新时间：2026-09-01。本文是发布工程记录，不是法律意见；版本、Runtime 或品牌素材变化后必须重新复核。

## 产品身份

- 产品名为 **DSH Launcher**，用于兼容 DeepSeek Harness（上游命令名为 `dsh`）。
- 本项目独立开发和维护，不是 DeepSeek 官方产品，也未声称获得 DeepSeek 审核、赞助或背书。
- Release copy must state: "Unofficial and independently maintained; not reviewed, sponsored, or endorsed by DeepSeek."
- README、应用导航和页脚均显示“非官方”；发布页、安装器说明和下载页也必须保留这一说明。
- 应用图标和界面 Logo 使用本仓库自有的“终端 + 启动”几何图形，不再使用 DeepSeek 鲸形标识。
- “DeepSeek Harness”仅用于说明兼容对象；不得把 DeepSeek 名称或标识作为发行者、签名主体或下载域名身份。

核对依据：上游官方仓库将产品称为 “DeepSeek Harness (`dsh`)”，并声明 MIT 许可：
<https://github.com/deepseek-ai/deepseek-harness>。截至本次复核，未找到官方发布的商标或 Logo 再使用授权，
因此按更保守的非官方兼容产品边界处理。

## Windows x64 实际携带依赖

`scripts/license-audit.mjs` 分开统计锁文件与本机实际安装目录。当前 Runtime 锁文件记录 580 个跨平台包，
Windows x64 实际安装 521 个；许可证门禁只对实际携带的包判定发布义务，同时在 JSON 中保留锁定总数。
推荐通道 RC2 和 Alpha.2 分别有 509、580 个包的完整版本锁；构建使用 `npm ci`，不再根据顶层包的
传递依赖范围重新解析。`phase4-check.ps1` 会逐个验证锁文件与 compatibility recipe、顶层 integrity 和
内部 DSH 包版本一致。

### MPL-2.0 Rust 依赖

当前有 5 个未经修改、由 crates.io 源码构建的 MPL-2.0 包：`cssparser`、`cssparser-macros`、
`dtoa-short`、`option-ext`、`selectors`。生成的 `THIRD_PARTY_NOTICES.md` 记录精确版本及对应 crates.io
源码获取地址。Mozilla 的 MPL 2.0 FAQ 对“由他人未修改源码编译并分发可执行文件”的要求，是告知接收者
如何取得 MPL 部分源码；本项目以精确版本源码链接落实该记录：
<https://www.mozilla.org/en-US/MPL/2.0/FAQ/>。

处置状态：`documented-source-availability`。若以后修改这些包、打补丁或改为私有源码来源，此状态立即失效。

### sharp Windows x64 二进制包

实际携带的 `@img/sharp-win32-x64@0.35.4` 声明 `Apache-2.0 AND LGPL-3.0-or-later`，目录中包含
独立的 `libvips-42.dll`、`libvips-cpp-8.18.6.dll`、原包 `LICENSE`、README 和版本清单。其上游构建仓库说明
Windows 预编译共享库来自 libvips 构建产物，并列出打包源码与第三方许可：
<https://github.com/lovell/sharp-libvips/tree/v1.3.3>。

LGPLv3 第 4 节要求显著许可说明、随附 GPL/LGPL 文本，并允许在满足条件时采用适合的共享库机制：
<https://www.gnu.org/licenses/lgpl-3.0.html>。

处置状态：`manual-review-required`。发布负责人仍需确认以下事项后才能解除硬门禁：

1. 安装介质和已安装产品随附 GPLv3、LGPLv3 全文以及上游第三方声明；
2. 用户替换接口兼容 DLL 的能力不会被完整性校验、更新器或签名策略阻止；
3. 对预编译 Windows 包中静态并入的 LGPL 组件，源码、构建脚本及必要的重链接材料满足发行要求；
4. Release 页面给出对应版本源码/构建材料的长期可用地址或随包归档。

在上述事项获得发行负责人或法律复核前，不将这一项自动标记为已解决。

本机技术证据进一步确认：`sharp-win32-x64-0.35.4.node` 的 PE 导入表直接引用
`libvips-42.dll` 和 `libvips-cpp-8.18.6.dll`，后者也直接引用 `libvips-42.dll`。启动器只在下载/安装时
验证签名和完整性，活动 Runtime 的有效性检查不固定原生 DLL 哈希；单元测试
`installed_runtime_record_does_not_pin_replaceable_native_library_hashes` 验证替换 DLL 后记录仍有效。
因此 libvips 本体具备接口兼容 DLL 替换路径。

但 `libvips-42.dll` 的导入表没有显示 glib、fribidi、libexif、libheif、librsvg 等 README 所列 LGPL
组件的独立 DLL。结合上游对 Windows “static web releases” 的说明，可以合理推断其中至少部分组件静态
并入该 DLL；这正是仍需源码、构建脚本和重链接材料人工复核的边界。可重复证据脚本为
`scripts/runtime-license-evidence.ps1`，本次 JSON 位于已忽略的
`phase4-results/runtime-license-evidence.json`。

## 发布文案检查表

- 发行者、证书 Subject 和下载页面使用本项目主体，不使用 DeepSeek 官方身份；
- 首屏、README、Release 正文和安装器说明保留“非官方、独立维护、未获背书”的表述；
- 不使用 DeepSeek 鲸形 Logo、官方截图或容易混淆的图标；
- 明确上游仍处于 developer preview，兼容性可能发生破坏性变化；
- 隐私说明、第三方通知和源码获取信息可从安装目录或发布页直接访问。
- Authenticode 发布者为 `SignPath Foundation`；README、Release 和下载页保留 SignPath.io/SignPath Foundation
  归属文案并链接 `docs/CODE_SIGNING_POLICY.md`，不得暗示该签名构成 DeepSeek 背书；
- 只接受 GitHub trusted build 生成的 artifact ID，内部 EXE 和最终 NSIS 必须分两阶段签名并具有可信时间戳。
