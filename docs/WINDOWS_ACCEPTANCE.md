# Windows 干净机验收

第三步的目标是验证普通用户无需 Node.js、npm、Git、Rust 或终端操作即可完成安装和使用。GitHub 托管的 Windows runner 是 Windows Server，只能证明构建、安装器和 Runtime 自动化路径，不能替代 Windows 10/11 桌面验收。

## 本轮收尾状态（2026-09-04）

项目所有者确认暂时没有一次性桌面虚拟机，本轮按“完成 CI、保留桌面验收待办”收尾，不将第三步完整桌面验收标记为通过。

| 验收范围 | 状态 | 证据 |
| --- | --- | --- |
| 一次性 Windows CI | PASS | [运行 33835360786](https://github.com/1424801913dd-cmd/dsh-launcher/actions/runs/33835360786)，全部步骤成功 |
| Windows 10 x64 桌面 | NOT_RUN | 等待干净实体机或虚拟机的人工观察报告 |
| Windows 11 x64 桌面 | NOT_RUN | 等待干净实体机或虚拟机的人工观察报告 |

CI 对应分支 `codex/product-step3-acceptance`，受测提交 `f6b6a0b4148f23ba8d6f325cb262748aaa627e1b`；
runner 为 Windows Server 2025，作业耗时 12 分 39 秒。后续仅归档文档的提交不属于此次受测提交。

- 前端类型检查/构建、Rust 格式检查通过；Rust 库测试 30 通过、0 失败、3 按设计忽略，另有 1 项进程崩溃集成测试按设计忽略。
- 从零 Runtime 测试独立执行并通过，报告耗时 104.531 秒，覆盖中文空格工作区、下载校验、依赖安装和隔离启停；残留受管进程为 0。
- 已安装 EXE 初始化与存活检查通过；静默安装/卸载退出码均为 0，安装资源/版本核对通过，程序及开始菜单目录移除，DSH_HOME sentinel 保留。
- CI 上生产活动指针前后均不存在（报告中为 `null`），不能据此声称验证了已有生产数据的兼容性；本机既有数据未变的证据见 `HANDOFF.md`。

已下载 artifact `phase4-unsigned-installer-smoke` 到本机 `phase4-results/ci-33835360786/`，核对三份 JSON 报告及启动日志。
安装器 `DSH Launcher_0.4.0_x64-setup.exe` 为 2,167,548 字节，下载后 SHA-256 与 CI 安装报告一致：
`949744407DABBCA21CD0AA3D7CE4F8FC93FDC2C935FCCD03E6081DF1ADA6E0DF`。
已安装内部 EXE 的报告 SHA-256 为 `6ED51495F5D020AD564E618CC498E375EA6DD6AD9CC223D7BD13DE8ACB0D2981`。
安装器实际检查为 `NotSigned`，仅供受控验收；没有创建正式 Release 或宣称通过 SmartScreen。

## 自动化层

手动工作流 `.github/workflows/phase4-installer.yml` 在一次性 Windows runner 中执行：

1. 前端、Rust 和格式检查；
2. 使用全新 Runtime、缓存、DSH_HOME 与中文空格工作区，从官方源下载并安装推荐版；
3. 验证 npm integrity、CLI、原生模块、隔离启动/停止和活动指针；
4. 构建未签名 NSIS，静默安装后通过专用启动器目录环境变量真正启动已安装程序，等待独立日志确认初始化；
5. 静默卸载，确认程序、卸载器和开始菜单目录移除，独立 DSH_HOME sentinel 保留；
6. 上传安装器、从零安装报告和安装器 smoke 报告。

本机可运行 `scripts/clean-runtime-install.ps1 -IsolatedEnvironment`。它只在名称为 `clean-runtime-*` 的全新目录工作，成功后清理该目录，并比较生产 `active.json` 前后摘要。`scripts/test-installer.ps1` 会修改同一应用标识的安装记录，因此只允许在一次性 runner 或无生产安装的隔离环境执行。

本机也可单独检查已构建 EXE 的初始化，不执行安装/卸载：

```powershell
./scripts/test-launcher-startup.ps1 `
  -ExecutablePath ./src-tauri/target/release/dsh-launcher.exe `
  -TestRoot ./phase4-results/launcher-startup-local `
  -ReportPath ./phase4-results/launcher-startup.json
```

测试前须关闭启动器；每次使用全新的 Runtime、缓存和设置路径。脚本保留 Windows 的真实用户环境，只覆盖启动器自己的目录，等待初始化日志并确认进程继续存活 3 秒，最后结束本次启动的进程。报告与隔离目录保留供排查。此检查不证明界面交互、纯净机首次向导或安装/卸载通过。

## Windows 10/11 桌面层

分别准备 Windows 10 x64 和 Windows 11 x64 的干净桌面环境，记录实际补丁和 build。可以使用另一台实体电脑，也可以使用一次性虚拟机；不要求购买或安装虚拟机软件。
开始前不得预装本项目、Node.js、DSH、Git 或 Rust，WebView2 可保留为系统组件。若测试电脑已有开发环境或生产数据，不要为验收删除它们：先记作兼容性试用，不能标记干净基线通过。
虚拟机可保存干净快照；实体机应记录安装前状态，使用无重要 DSH 数据的测试账户和独立工作区。新建账户不等于清除了机器级预装软件。
一台 Windows 11 电脑不能代替 Windows 10 验收。项目所有者现计划在另一台实体电脑补测，但尚未产生实测结果。每台机器执行以下检查：

| 检查 | 通过标准 |
| --- | --- |
| 安装 | 当前用户安装完成，版本和安装资源正确 |
| 首次向导 | 明确经历 1/3、2/3、3/3，普通用户无需命令行 |
| 默认路径 | 全部落在当前用户目录，不创建早期固定 D 盘目录 |
| 自定义路径 | Runtime、缓存、DSH_HOME、工作区支持中文和空格，重启后生效 |
| 断网重试 | 给出可操作提示；联网后重试成功，无损坏活动指针 |
| 端口冲突 | 3080 外部服务被识别，不接管、不结束外部进程 |
| 启动/打开/停止 | 健康检查通过后打开页面；停止后页面不可达且无残留受管进程 |
| 托盘 | 关闭窗口后托盘仍可恢复；退出语义明确 |
| 卸载与数据 | 程序和快捷方式移除，DSH_HOME、工作区和 sentinel 保留 |
| SmartScreen | 如实记录实际界面；未签名制品出现警告属于已知发布阻断项 |

完成每台测试机后使用 `scripts/collect-windows-acceptance.ps1` 记录系统 build、WebView2、制品 SHA-256、Authenticode、SmartScreen 和所有观察项。
schema v2 新增 `-EnvironmentKind physical|virtual|unknown` 和 `-BaselineClean YES|NO|UNKNOWN`；两者默认未知，不会自动宣称电脑干净。
脚本会拒绝 Windows Server、错误的 Windows 版本以及缺失的安装器；任何 `FAIL` 返回失败，任何 `NOT_RUN`、未知环境或非干净基线均返回未完成。

## 完成门槛

第三步只有在以下三份证据同时存在时才能标记完成：

- 一次性 runner 的从零 Runtime 安装报告、安装器启动与卸载报告均为通过；
- Windows 10 桌面报告全部为 `PASS`；
- Windows 11 桌面报告全部为 `PASS`。

代码签名不属于第三步功能验收的通过条件，但未签名和 SmartScreen 风险必须保留为正式发布阻断项。

所有桌面检查项均为 `PASS`、SmartScreen 已实际观察、机器类型已记录且安装前干净基线确认后，记录脚本才标记完成；不得将本机自动化或模拟测试结果填作桌面人工观察结果。
