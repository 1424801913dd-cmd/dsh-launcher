# DSH Launcher

一个面向 Windows 的非官方 DeepSeek Harness 本地启动器，使用 Tauri 2、React、TypeScript 与 Rust 构建。
本项目独立开发和维护，未经 DeepSeek 审核、赞助或背书；上游目前仍处于 developer preview，兼容性可能发生破坏性变化。
In English: this is an unofficial, independently developed project; it is not reviewed, sponsored, or endorsed by DeepSeek.

## 当前能力

- 单实例启动、系统托盘与受管 DSH 生命周期；
- Windows Job Object 进程树清理与真实 HTTP 健康检查；
- 私有 Node.js Runtime、推荐版/Alpha 通道、旁路安装与 smoke test；
- 原子版本指针、手动切换和回滚；
- Ed25519 签名 Runtime manifest/Bundle、后台下载、DSH_HOME 备份和失败自动回滚；
- Tauri 签名启动器更新及 GitHub Actions 发布流水线。

普通本地构建默认关闭远程更新。只有发布流水线注入真实 HTTPS 地址与公钥后才会启用，客户端不会在
Runtime Bundle 更新期间执行 `npm install` 或网络生命周期脚本。

## 本地数据目录

全新安装默认把 Runtime 与缓存分别放在当前用户的
`%LOCALAPPDATA%\DSH Launcher\runtime` 和 `%LOCALAPPDATA%\DSH Launcher\cache`，DSH 用户数据默认位于
`%LOCALAPPDATA%\DeepSeek Harness\home`。存在 `D:\Tools\dsh-launcher` 活动指针或已安装 Runtime 记录时，
启动器会继续使用原 D 盘目录以兼容早期版本，但不会仅因电脑存在 D 盘就为新用户创建这些目录。

## 开发

项目当前优先使用 D 盘的 Node、Rust 与缓存环境，具体初始化逻辑见
`scripts/Initialize-DevEnvironment.ps1`。

```powershell
& '.\scripts\check.ps1'
& '.\scripts\build.ps1'
```

构建产物位于 `src-tauri\target\release\dsh-launcher.exe`。

签名密钥、GitHub Actions Secrets 和发布步骤见 [docs/RELEASE.md](docs/RELEASE.md)。
普通用户的首次安装、目录说明和故障排查见 [docs/USER_GUIDE.md](docs/USER_GUIDE.md)。
干净 Windows 自动化与桌面验收矩阵见 [docs/WINDOWS_ACCEPTANCE.md](docs/WINDOWS_ACCEPTANCE.md)。
2026-09-04：一次性 Windows CI 的从零 Runtime 安装及安装器启停/卸载验收已通过；Windows 10/11 桌面验收仍待执行，当前制品仍未签名。
后续 Windows 10 实体机发现 0.4.0 版本漂移，旧候选停止使用；当前 0.4.1 修复和复测范围见 [版本漂移复测说明](docs/VERSION_DRIFT_RETEST.md)。旧 CI 通过不等于该桌面缺陷已关闭。
测试端随后报告 0.4.1 在“已安装/无法卸载”页面被阻断，尚未完成版本漂移复测。当前先使用[专用只读安装器诊断流程](docs/INSTALLER_DIAGNOSTIC_README.md)采证；该诊断包不是可安装修复版，不可将其计为桌面验收通过。

## 受控试用准备

另一台实体电脑也可用于桌面验收，不要求安装虚拟机。先读 [试用指南](docs/TRIAL_GUIDE.md)，候选版本与限制见
[试用说明](docs/TRIAL_RELEASE_NOTES.md)，结果填写 [反馈表](docs/TRIAL_FEEDBACK.md)。不要在已有重要 DSH 数据的账户上直接试装。
第二台电脑已试过旧版安装：接手助手先读 [旧安装盘点与安全隔离说明](docs/TEST_MACHINE_HANDOFF.md)（ZIP 中为 `CODEX-HANDOFF.md`），不要直接清理目录或把兼容性测试当作全新安装。

维护者可从已核验 CI artifact 生成可拷贝试用 ZIP，不重新构建 EXE、不发布 Release：

```powershell
./scripts/prepare-trial-package.ps1 `
  -ArtifactRoot ./phase4-results/ci-33857469193 `
  -OutputRoot ./release-assets/trial-step4-0.4.1
```

脚本按 `scripts/data/trial-candidate.json` 固定安装器身份，并核对四份 CI 报告（含两种安装模式和精确版本一致性）；拒绝哈希不符、失败报告和覆盖已有输出。
包内包含安装器、校验值、说明、反馈表及可选的桌面记录脚本，不包含个人数据或预装 Runtime。
回归验证入口为 `scripts/test-trial-package.ps1 -ArtifactRoot ./phase4-results/ci-33857469193`；其模拟报告不是实机验收证据。

## 安全边界

- 不读取、复制、记录或上传 DSH 凭据与完整会话内容；
- 不按进程名或端口终止非本启动器创建的进程；
- 不接受未签名、被篡改、过期、降级或重放的 Runtime 更新；
- Windows Authenticode、SmartScreen 与干净虚拟机安装器验收属于后续发布质量工作。

## 许可

MIT。详见 [LICENSE](LICENSE)。DeepSeek Harness 及相关标识归其各自权利人所有；本项目为非官方项目。
第三方依赖清单见 [docs/THIRD_PARTY_NOTICES.md](docs/THIRD_PARTY_NOTICES.md)，发布身份与许可证复核见
[docs/RELEASE_REVIEW.md](docs/RELEASE_REVIEW.md)。

## Code signing policy

当前状态（2026-09-04）：SignPath Foundation 申请因项目尚缺少足够的公开采用度、独立引用和持续参与信号而
未获批准。当前没有任何公开制品由 SignPath Foundation 签名，签名发布流水线保持失败关闭，也不会退回发布
未签名正式版本。

仓库保留了未来获批后使用的受信构建与双阶段签名设计。届时适用的归属文案为：Free code signing provided by
[SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/)。当前状态、团队角色、
审批边界和隐私要求见 [docs/CODE_SIGNING_POLICY.md](docs/CODE_SIGNING_POLICY.md)，申请记录见
[docs/SIGNPATH_APPLICATION.md](docs/SIGNPATH_APPLICATION.md)。
