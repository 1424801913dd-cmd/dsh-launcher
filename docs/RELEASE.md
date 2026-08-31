# 第三阶段签名发布说明

仓库中的普通本地构建默认关闭所有远程更新入口。只有发布流水线同时注入真实 HTTPS 地址与公钥时，签名 Runtime 更新和 Tauri 启动器自更新才会启用。不要把生产私钥提交到仓库。

## 两套独立签名密钥

1. Runtime Release Ed25519 密钥用于签名 Runtime Bundle 的 SHA-256 摘要和 Runtime manifest 原始 payload。客户端只内嵌 32 字节公钥。
2. Tauri updater 密钥用于签名启动器更新制品，由 Tauri 官方 updater 验证。

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

- `DSH_RUNTIME_SIGNING_PRIVATE_KEY`：Base64 PKCS#8 Runtime 私钥；
- `DSH_RUNTIME_PUBLIC_KEY`：Base64 原始 32 字节 Runtime 公钥；
- `TAURI_SIGNING_PRIVATE_KEY`：Tauri updater 私钥；
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`：对应口令；
- `TAURI_UPDATER_PUBLIC_KEY`：Tauri updater 公钥。

触发时必须提供精确 DSH 版本、`recommended` 或 `alpha` 通道，以及严格递增的 manifest sequence。流水线将：

1. 读取 `compatibility-recipes.json`，下载并校验固定 Node ZIP；
2. 在发布机执行一次精确 npm 生产安装和原生依赖验证；
3. 生成包含完整 Node 与 DSH 依赖树的 Runtime Bundle；
4. 对 Bundle 摘要和 manifest payload 分别执行 Ed25519 签名；
5. 合并另一个通道上一份仍有效的已签名 release，避免通道混淆；
6. 注入 Runtime 公钥、manifest 地址及 Tauri updater 配置；
7. 运行完整测试，构建 Tauri updater 制品，并上传 Runtime 通道资产。

客户端更新时不会运行 `npm install`、preinstall、install 或 postinstall。所有网络生命周期脚本只允许出现在受控发布流水线的 Bundle 构建步骤。

## 本地验证

```powershell
& '.\scripts\check.ps1'
& '.\scripts\build.ps1'
```

普通 Release EXE 位于 `src-tauri\target\release\dsh-launcher.exe`。默认配置中的“签名自动更新”会显示为未配置，这是预期行为；不要用占位 URL 或测试公钥伪装生产更新。

Windows Authenticode 证书、SmartScreen 声誉和干净虚拟机安装器验证属于第四阶段发布质量工作；Tauri updater 制品签名不能替代 Authenticode。
