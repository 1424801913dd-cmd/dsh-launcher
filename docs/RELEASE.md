# 签名发布说明

## 当前状态

截至 2026-09-04，SignPath Foundation 申请因项目公开采用度和独立信任信号不足而未获批准。因此本项目当前
没有可用的 SignPath Foundation Authenticode 签名资格，`.github/workflows/release.yml` 仍是预备流程：缺少
SignPath 的真实配置时必须失败关闭，不得填写伪造值、借用其他项目身份或声称制品已由 SignPath Foundation
签名。

仓库中的普通本地构建默认关闭所有远程更新入口。只有发布流水线同时注入真实 HTTPS 地址与公钥时，签名 Runtime 更新和 Tauri 启动器自更新才会启用。不要把生产私钥提交到仓库。

## 两套独立签名密钥

1. Runtime Release Ed25519 密钥用于签名 Runtime Bundle 的 SHA-256 摘要和 Runtime manifest 原始 payload。客户端只内嵌 32 字节公钥。
2. Tauri updater 密钥用于签名启动器更新制品，由 Tauri 官方 updater 验证。
3. Windows Authenticode 计划在未来获批后使用 SignPath.io 托管的 SignPath Foundation 证书。当前该路径尚未
   启用；启用后私钥仅存在于 SignPath HSM，不下载到本机或 GitHub Actions runner。

在受控位置生成 Runtime Release 密钥，例如 D 盘的离线密钥目录：

```powershell
node .\scripts\sign-runtime-release.mjs generate `
  --private-output 'D:\Secrets\dsh-launcher\runtime.update-private-key' `
  --public-output 'D:\Secrets\dsh-launcher\runtime.update-public-key'
```

使用 Tauri 官方 signer 生成启动器更新密钥：

```powershell
npm run tauri -- signer generate -w 'D:\Secrets\dsh-launcher\tauri-updater.key'
```

## GitHub Actions 配置

`.github/workflows/release.yml` 是人工触发的 Windows 发布流水线。需要配置：

当前不得运行该流程发布正式版本，因为 SignPath Foundation 尚未批准项目。下列 SignPath 配置只能使用服务商
实际签发的值。

- `DSH_RUNTIME_SIGNING_PRIVATE_KEY`：Base64 PKCS#8 Runtime 私钥；
- `DSH_RUNTIME_PUBLIC_KEY`：Base64 原始 32 字节 Runtime 公钥；
- `TAURI_SIGNING_PRIVATE_KEY`：Tauri updater 私钥；
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：对应口令；
- `TAURI_UPDATER_PUBLIC_KEY`：Tauri updater 公钥。

SignPath Foundation 批准项目后，还需要配置一个 GitHub Actions Secret：

- `SIGNPATH_API_TOKEN`：仅有本项目提交签名请求权限的 SignPath API token。

以及五个 GitHub Actions Variables：

- `SIGNPATH_ORGANIZATION_ID`；
- `SIGNPATH_PROJECT_SLUG`；
- `SIGNPATH_SIGNING_POLICY_SLUG`；
- `SIGNPATH_EXECUTABLE_ARTIFACT_CONFIGURATION_SLUG`：只接受根目录 `dsh-launcher.exe` 的配置；
- `SIGNPATH_INSTALLER_ARTIFACT_CONFIGURATION_SLUG`：只接受根目录 `DSH.Launcher_<version>_x64-setup.exe` 的配置。

SignPath 项目必须绑定 <https://github.com/1424801913dd-cmd/dsh-launcher> 作为 GitHub trusted build system。
签名策略必须使用 SignPath Foundation 证书、可信时间戳和人工审批。仓库与 SignPath 账户均启用 MFA。
公开签名政策见 [CODE_SIGNING_POLICY.md](CODE_SIGNING_POLICY.md)。
发布 job 只允许从 `main` 触发且同一时间只运行一个实例。Runtime/Tauri 私钥只注入各自签名步骤，不会暴露给
依赖安装、编译、SignPath action 或安装器测试步骤。

触发时必须提供精确 DSH 版本、`recommended` 或 `alpha` 通道，以及严格递增的 manifest sequence。流水线将：

1. 读取 `compatibility-recipes.json`，下载并校验固定 Node ZIP；
2. 按 DSH 版本读取 `src-tauri/resources/runtime-locks` 中经过复核的完整 npm lockfile，只执行
   `npm ci --omit=dev`，并验证 lockfile 未被改写、顶层 integrity 正确、所有内部 DSH 包版本一致；
3. 生成包含完整 Node 与 DSH 依赖树的 Runtime Bundle，并随包写入实际安装依赖 NOTICE、GPL/LGPL 全文、
   原生包自带许可文件及精确 sharp-libvips 构建源码入口；
4. 对 Runtime 许可证报告执行硬门禁；存在 `manual-review-required` 项时，在签名和上传前停止；
5. 对 Bundle 摘要和 manifest payload 分别执行 Ed25519 签名；
6. 合并另一个通道上一份仍有效的已签名 release，避免通道混淆；
7. 注入 Runtime 公钥、manifest 地址及 Tauri updater 配置；
8. 先生成一次不会发布的未签名 NSIS 预打包，让 Tauri 固定 EXE 的 NSIS bundle 类型元数据；随后只上传
   该 EXE 的 GitHub Actions artifact ID 到 SignPath，等待人工批准并取回签名结果；
9. 验证 EXE 的 SignPath Foundation Authenticode 链和时间戳，再用完全相同的 NSIS 目标重新打包；
10. 将 NSIS artifact ID 提交给 SignPath，验证签名后执行隔离安装、卸载和数据保留测试；
11. 对最终签名安装器创建 Tauri updater ZIP，再生成 Ed25519 签名和 `latest.json`；
12. 只有全部硬门禁通过后才创建或更新 GitHub draft Release，并上传 Runtime 通道资产及许可证报告。

客户端更新时不会运行 `npm install`、preinstall、install 或 postinstall。所有网络生命周期脚本只允许出现在受控发布流水线的 Bundle 构建步骤。
缺少对应版本的完整 Runtime lockfile 时，Bundle 构建会在下载或安装依赖前失败关闭；不得退回动态解析传递依赖。

## 本地验证

```powershell
& '.\scripts\check.ps1'
& '.\scripts\build.ps1'
```

普通 Release EXE 位于 `src-tauri\target\release\dsh-launcher.exe`。默认配置中的“签名自动更新”会显示为未配置，这是预期行为；不要用占位 URL 或测试公钥伪装生产更新。

`scripts/verify-authenticode.ps1` 会拒绝签名链无效、签名主体不是 SignPath Foundation 或缺少可信时间戳的
制品。Tauri updater 制品签名不能替代 Authenticode；获得有效 Authenticode 也不保证 SmartScreen 立即建立声誉。

## SignPath Foundation 申请与首次启用

申请记录和当前状态见 [SIGNPATH_APPLICATION.md](SIGNPATH_APPLICATION.md)。下一次申请应在项目已有真实 Windows
发行历史、持续维护记录、社区参与和独立外部引用之后提交，不能原样重复上一轮材料。申请入口：
<https://signpath.org/apply>。申请时提供公开仓库、MIT 许可证、已有 Release、隐私说明、本文件和
`CODE_SIGNING_POLICY.md`。

项目获批前不要把占位 organization、project、policy 或 artifact configuration 值写入仓库；获批后按
SignPath 控制台的真实值配置 GitHub Variables 和 Secret。

首次运行发布工作流时需要在 SignPath 中分别批准 EXE 和 NSIS 两次请求。工作流最多等待每次批准一小时；
超时不会上传或发布未签名制品，可以在重新触发前从 SignPath 审计记录确认请求状态。

## 获批前的预览版本边界

如果维护者以后决定用未签名制品积累真实用户反馈，应使用独立的预发布流程，而不是削弱或绕过现有签名发布
流程。预览版本必须显著标记 `Unsigned Preview` 和 Windows“未知发布者”风险，提供源码提交号、SHA-256、
许可证材料和安装测试结果，并关闭启动器自动更新。未签名预览不得使用正式发布模板，也不得描述为已通过
SignPath、Authenticode 或 SmartScreen 验证。
