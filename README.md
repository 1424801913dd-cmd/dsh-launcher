# DSH Launcher

一个面向 Windows 的非官方 DeepSeek Harness 本地启动器，使用 Tauri 2、React、TypeScript 与 Rust 构建。

## 当前能力

- 单实例启动、系统托盘与受管 DSH 生命周期；
- Windows Job Object 进程树清理与真实 HTTP 健康检查；
- 私有 Node.js Runtime、推荐版/Alpha 通道、旁路安装与 smoke test；
- 原子版本指针、手动切换和回滚；
- Ed25519 签名 Runtime manifest/Bundle、后台下载、DSH_HOME 备份和失败自动回滚；
- Tauri 签名启动器更新及 GitHub Actions 发布流水线。

普通本地构建默认关闭远程更新。只有发布流水线注入真实 HTTPS 地址与公钥后才会启用，客户端不会在
Runtime Bundle 更新期间执行 `npm install` 或网络生命周期脚本。

## 开发

项目当前优先使用 D 盘的 Node、Rust 与缓存环境，具体初始化逻辑见
`scripts/Initialize-DevEnvironment.ps1`。

```powershell
& '.\scripts\check.ps1'
& '.\scripts\build.ps1'
```

构建产物位于 `src-tauri\target\release\dsh-launcher.exe`。

签名密钥、GitHub Actions Secrets 和发布步骤见 [docs/RELEASE.md](docs/RELEASE.md)。

## 安全边界

- 不读取、复制、记录或上传 DSH 凭据与完整会话内容；
- 不按进程名或端口终止非本启动器创建的进程；
- 不接受未签名、被篡改、过期、降级或重放的 Runtime 更新；
- Windows Authenticode、SmartScreen 与干净虚拟机安装器验收属于后续发布质量工作。

## 许可

MIT。详见 [LICENSE](LICENSE)。DeepSeek Harness 及相关标识归其各自权利人所有；本项目为非官方项目。
