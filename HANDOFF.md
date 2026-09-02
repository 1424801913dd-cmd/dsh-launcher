# DeepSeek Harness 启动器交接文档

> 更新日期：2026-09-01
> 适用系统：Windows 10/11 x64
> 维护范围：本机 DeepSeek Harness（DSH）运行环境、桌面启动器、用户数据与后续升级

## 一、用途与边界

这套文件为本机 DeepSeek Harness 提供无需打开终端的桌面启动入口。用户双击桌面上的
`DeepSeek Harness.lnk` 后，启动器会检查本机 Web UI；未运行时静默启动 DSH，等待服务
就绪，然后使用默认浏览器打开页面。

本目录只保存启动器脚本与维护文档。DSH 运行环境、用户数据和依赖缓存体积较大，分别放在
`D:\Tools` 与 `D:\Caches`，不复制到本目录，也不纳入 `dsmax` Git 仓库。

## 二、文件清单

| 文件 | 作用 |
| --- | --- |
| `DeepSeek-Harness-Launcher.vbs` | 桌面快捷方式调用的实际启动脚本 |
| `DSH-ROLLBACK.md` | 旧版 rc.6 的精确重装与回退提示 |
| `HANDOFF.md` | 当前完整环境、维护、升级与排错说明 |
| `src/` | React/TypeScript 启动器界面 |
| `src-tauri/` | Rust 生命周期管理器、Tauri 配置与 bridge |
| `scripts/` | 开发、检查和发布构建脚本 |

桌面快捷方式位于：

```text
%USERPROFILE%\Desktop\DeepSeek Harness.lnk
```

## 三、当前环境

| 项目 | 当前值 |
| --- | --- |
| DSH Launcher | `0.3.3`，安装于 `D:\Bian_CHENG\dsh-launcher\DSH Launcher` |
| 活动 DSH 版本 | `0.1.2-alpha.2`（签名、受管 Runtime） |
| 上一 DSH 版本 | `0.1.1-rc.2`（`previous.json`，可回滚） |
| Node.js | `D:\Tools\dsh-launcher\versions\dsh-0.1.2-alpha.2\node\node.exe`（`24.20.0`） |
| DSH 运行环境 | `D:\Tools\dsh-launcher\versions\dsh-0.1.2-alpha.2` |
| DSH CLI 入口 | `D:\Tools\dsh-launcher\versions\dsh-0.1.2-alpha.2\app\node_modules\@deepseek-ai\dsh\lib\bin.js` |
| 用户数据目录 | `D:\Caches\deepseek-harness\home` |
| npm 缓存 | `D:\Caches\npm` |
| 默认工作目录 | `D:\Bian_CHENG\dsmax` |
| Web 地址 | 每次启动使用随机 `127.0.0.1` 端口，并带进程私有访问令牌 |
| Web profile | `D:\Caches\deepseek-harness\home\profiles\web` |

当前 Web profile 只组合官方的 `@deepseek-ai/dsh-base` 与
`@deepseek-ai/dsh-web-app`，没有安装第三方插件。

## 四、启动链路

```text
桌面快捷方式
  -> C:\Windows\System32\wscript.exe
  -> DeepSeek-Harness-Launcher.vbs
  -> 设置进程级 DSH_HOME=D:\Caches\deepseek-harness\home
  -> 检查 http://127.0.0.1:3080/
  -> 未就绪时使用 --no-open 隐藏启动 node.exe + DSH web profile
  -> 最长等待约 90 秒
  -> 服务返回 HTTP 200 后打开默认浏览器
```

`--no-open` 用于关闭 DSH 自带的浏览器打开动作，由 VBS 在服务就绪后统一打开一次页面，避免
冷启动时出现两个相同标签页。启动器不会创建开机启动项，也不会常驻托盘。它只在双击时启动
或打开 DSH。

## 五、日常使用与状态检查

日常使用只需双击桌面上的 `DeepSeek Harness`。

PowerShell 中可用以下只读命令检查服务：

```powershell
Invoke-WebRequest 'http://127.0.0.1:3080/' -UseBasicParsing
```

检查实际运行版本：

```powershell
& 'D:\Tools\node-v24.19.0-win-x64\node.exe' `
  'D:\Tools\dsh-runtime-0.1.1-rc.2\node_modules\@deepseek-ai\dsh\lib\bin.js' `
  --version
```

关闭浏览器标签不会自动结束 DSH。需要停止时，应先通过进程命令行确认目标确实是当前 DSH，
再结束对应的 `node.exe`，不要按名称批量结束所有 Node 进程。

## 六、用户数据与备份

用户数据位于：

```text
D:\Caches\deepseek-harness\home
```

其中可能包含会话、Workspace 信息、模型设置和凭据引用，不得提交 Git、上传公开网盘或复制到
公开问题报告中。

升级前应使用不跟随 Junction 的方式备份，并排除依赖目录：

```powershell
robocopy `
  'D:\Caches\deepseek-harness\home' `
  'D:\Caches\deepseek-harness\backups\dsh-home-userdata-pre-upgrade-YYYYMMDD-HHmmss' `
  /E /XJ /XD node_modules
```

当前升级前备份的指针文件：

```text
D:\Caches\deepseek-harness\latest-pre-upgrade-backup.txt
```

不要用会跟随 Junction 的递归复制方式备份整个目录，否则可能把运行时依赖重复复制数万份。

## 七、升级流程

### 1. 查询版本

```powershell
$node = 'D:\Tools\node-v24.19.0-win-x64\node.exe'
$npm = 'D:\Tools\node-v24.19.0-win-x64\node_modules\npm\bin\npm-cli.js'
& $node $npm view '@deepseek-ai/dsh' dist-tags version versions --json `
  --cache 'D:\Caches\npm'
```

优先使用 npm 的 `latest` 或 `next`。GitHub 上标记为 Alpha 的预发行版不应直接覆盖当前可用版；
应安装到新的版本化目录，验证后再切换。

### 2. 新建版本化运行环境

示例目标：

```text
D:\Tools\dsh-runtime-<新版本>
```

安装时必须保留旧目录，不能覆盖当前运行环境：

```powershell
& $node $npm install `
  --prefix 'D:\Tools\dsh-runtime-<新版本>' `
  --cache 'D:\Caches\npm' `
  '@deepseek-ai/dsh@<新版本>' `
  --no-audit `
  --no-fund
```

`0.1.1-rc.2` 发布包曾因 peer 依赖解析异常而需要 `--legacy-peer-deps`，并手工补齐官方
运行依赖。未来版本不要机械复制这套补丁；应先尝试标准安装，再根据实际 npm 清单和启动错误
处理。当前 rc.2 的完整恢复依据是其运行环境根目录的 `package.json`，其中同时记录了补充依赖
和允许执行安装脚本的包。

### 3. 验证新环境

至少完成以下检查：

- `dsh --version` 输出预期版本；
- `sharp` 可以加载；
- `node-pty` 可以加载并提供 `spawn`；
- 使用现有 `DSH_HOME` 启动后，`http://127.0.0.1:3080/` 返回 HTTP 200；
- 页面标题为 `DeepSeek Harness`；
- stderr 没有模块缺失、数据库迁移或原生组件错误。

### 4. 切换启动器

只修改 `DeepSeek-Harness-Launcher.vbs` 中的 `dshEntry`，让它指向新版本目录。停止验证
服务后，必须通过桌面快捷方式重新启动一次，并核对实际 `node.exe` 命令行包含新版本路径。

### 5. 清理旧环境

新版本稳定运行后再删除旧运行环境。用户数据备份至少保留一份。删除前必须核对旧目录没有被
任何 `node.exe` 进程使用。

## 八、插件安装注意事项

DSH 插件按 profile 安装。当前目标 profile 为 `web`，真实目录在 D 盘：

```text
D:\Caches\deepseek-harness\home\profiles\web
```

运行插件命令前必须先设置：

```powershell
$env:DSH_HOME = 'D:\Caches\deepseek-harness\home'
```

否则 DSH 会使用默认的 `%USERPROFILE%\.dsh`，可能在 C 盘生成另一套 profile 和大量依赖。

`dsh plugin` 会把参数转交给 pnpm。当前 D 盘 Node 环境带有 `corepack.cmd`，但 pnpm 尚未作为
可用命令配置。安装插件前应先把 pnpm 准备在 D 盘，并备份 Web profile；不要为了省事把大型
依赖缓存改到 C 盘。

社区插件会在 DSH 进程内运行，可能访问 DSH 能访问的文件、网络和会话数据。安装前至少检查：

- 仓库与 npm 包归属是否一致；
- `package.json` 是否包含 `preinstall`、`install` 或 `postinstall`；
- peer dependency 是否明确兼容当前 DSH；
- 是否要求远程脚本、管理员权限或关闭安全机制；
- 是否提供卸载、回退和版本锁定说明。

## 九、故障排查

### Windows Script Host：找不到脚本文件

原因通常是启动器文件被移动，但桌面快捷方式仍保存旧参数。检查快捷方式的：

- 目标：`C:\Windows\System32\wscript.exe`
- 参数：启动器 VBS 的绝对路径，并用双引号包围
- 起始位置：启动器所在目录

### Windows Script Host：未结束的字符串常量

说明 VBS 的引号或续行被破坏。不要用包含智能引号的编辑器替换英文 `"`，也不要删除命令
拼接行末的 `_`。

### 页面无法访问

按顺序检查：

1. `dshEntry` 指向的文件是否存在；
2. 3080 端口是否被其他程序占用；
3. `DSH_HOME` 是否为 D 盘真实数据目录；
4. 使用相同参数前台启动一次以查看 stderr；
5. 是否存在 `ERR_MODULE_NOT_FOUND` 或原生模块加载错误。

### 首次启动出现两个相同标签页

DSH Web 默认会自行打开浏览器，而 VBS 也会在服务就绪后打开页面。启动命令必须保留
`--no-open`，确保浏览器只由 VBS 打开一次。若升级或调整启动参数后问题复现，先检查
`DeepSeek-Harness-Launcher.vbs` 中的 Web 命令是否仍包含该参数。

### `dsh plugin` 报找不到 pnpm

这是插件管理器缺少 pnpm 前置环境，不影响普通桌面启动。配置 D 盘 pnpm 后重试，并确保命令
继承正确的 `DSH_HOME`。

## 十、安全与维护边界

- 不读取、提交或分享 DSH 用户目录中的凭据、令牌和完整会话；
- 不把第三方插件的“一键 PowerShell 下载并执行”当作可信安装方式；
- 不直接覆盖当前运行环境；所有升级使用新版本化目录；
- 不通过放宽整个磁盘、用户目录或仓库 ACL 来解决单一安装问题；
- 启动器本身不提供安全沙箱，DSH 与插件的权限等同于启动它的 Windows 用户；
- DSH 仍处于预发布阶段，升级和插件变更都应先备份、再验证、最后清理。

## 十一、与 dsmax 仓库的关系

启动器最初保存在：

```text
D:\Bian_CHENG\dsmax\tools\dsh-launcher
```

2026-08-28 起迁移到：

```text
D:\Bian_CHENG\dsh-launcher
```

迁移后它不再属于 `DeepSeek-Max-Fix` 项目，也不会自动随该仓库提交。若需要长期版本管理，建议
以后为本目录单独初始化 Git 仓库，而不是重新放回 `dsmax`。

## 十二、旧版 VBS 交接验收清单

- [ ] VBS、回滚说明与本文档都位于 `D:\Bian_CHENG\dsh-launcher`；
- [ ] 桌面快捷方式参数指向新 VBS；
- [ ] 快捷方式起始位置为新目录；
- [ ] VBS 中 Node、DSH、Workspace 与 DSH_HOME 路径均存在；
- [ ] 双击快捷方式后页面返回 HTTP 200；
- [ ] 实际后台进程使用 `dsh-runtime-0.1.1-rc.2`；
- [ ] `dsmax` 仓库交接文档不再把启动器描述为仓库内文件；
- [ ] `dsmax` Git 记录已移除原 `tools/dsh-launcher` 文件。

## 十三、桌面启动器升级方案

### 1. 目标与可行性

计划将当前 VBS 启动入口升级为类似游戏启动器的独立桌面应用，负责管理 DSH 的完整生命周期，
而不是替换 DSH 自带的 Web UI。该方案在 Windows 10/11 上可行，主要能力包括：

- 检测 DSH 是否安装以及当前运行状态；
- 启动 DSH、等待健康检查通过并打开 Web UI；
- 真正停止 DSH 及其完整子进程树，而不是只关闭浏览器窗口；
- 检查推荐版和 Alpha 预览版更新；
- 从零安装私有 Node.js 与 DSH，不污染系统 PATH 或全局 npm；
- 在旁路目录安装新版本，验证后原子切换，并支持失败回滚；
- 对启动器自身提供签名更新；
- 保留当前 VBS 作为新启动器开发和过渡期间的回退入口。

截至 2026-08-30，npm 的 `latest` 为 `0.1.1-rc.2`，`alpha` 为
`0.1.2-alpha.2`。DSH 仍处于快速迭代的开发预览阶段，因此更新逻辑必须区分推荐版和预览版，
不能把语义化版本号最大的版本直接视为推荐安装版本。

### 2. 推荐技术栈

```text
Tauri 2 + React/TypeScript + Rust + Windows Job Object
```

选择理由：

- React/TypeScript 适合实现简洁、美观的游戏启动器式界面；
- Tauri 复用 Windows WebView2，安装体积和内存通常小于 Electron；
- Rust 后端可直接持有 Windows 进程句柄并调用 Job Object；
- Tauri Updater 支持强制签名的启动器自更新；
- Tauri capability 可以只开放少量固定命令，避免前端获得任意 Shell 或文件系统权限。

Electron 可作为团队拒绝引入 Rust 时的备选，但包体、内存和攻击面更大，并且仍需原生 helper
才能可靠管理完整进程树。若确定只支持 Windows 且团队更熟悉 C#，`.NET 8 + WPF` 也是可靠备选。

### 3. 总体架构

```text
React 启动器 UI
       │ 仅调用固定命令
       ▼
Rust 后端
  ├─ Process Supervisor
  │    └─ Windows Job Object
  │         └─ dsh-bridge.mjs -> Node.js -> DSH Web
  ├─ Runtime Manager
  │    ├─ Node.js 版本
  │    ├─ DSH 版本
  │    ├─ 安装、校验、切换
  │    └─ 回滚
  └─ Launcher Updater
       └─ 签名更新包 / GitHub Releases

DSH_HOME 用户数据独立保存，不随程序版本安装、切换或清理
```

前端只允许调用以下窄接口，不开放通用命令执行能力：

```text
get_status
start_dsh
stop_dsh
open_web_ui
check_dsh_update
download_dsh_update
apply_dsh_update
rollback_dsh
get_logs
```

### 4. DSH 进程生命周期

#### 启动

1. 使用单实例 Mutex 防止双击或多个启动器实例并发启动；
2. 由 Rust 使用固定的可执行文件和参数数组启动，不经过 `cmd.exe`、PowerShell 或 `npx`；
3. 建议使用 `--host 127.0.0.1 --port 0 --no-open`，由系统选择空闲端口；
4. 在主线程恢复前先把进程加入 Windows Job Object；
5. 保存实例 UUID、PID、创建时间、规范化可执行路径、Runtime ID、端口和进程句柄；
6. 持续读取 stdout/stderr，解析实际 Web URL 并写入轮转日志；
7. 只有目标进程仍存活且 HTTP 健康检查成功时，状态才进入 `Running`；
8. 用户点击“打开 DSH”后才调用系统默认浏览器打开 Web UI。

不能仅因为 3080 端口返回 HTTP 200 就认定它属于当前 DSH，也不能按 `node.exe` 名称或模糊
命令行扫描后结束进程。

#### 真正停止

本机 DSH `0.1.1-rc.2` 已注册 `SIGINT` 和 `SIGTERM`，收到信号后会执行完整应用插件树的
`dispose()`，并提供约 5 秒的有界清理时间。但 Windows 上普通 `child.kill()` 可能直接终止
Node.js，不能保证触发上述清理，也可能遗留工具、PTY 或其他子进程。

推荐停止流程：

1. 启动器运行一个很小的 `dsh-bridge.mjs`，由它在同一 Node.js 进程中引导 DSH；
2. Rust 通过 bridge 的私有 stdin 管道发送 `shutdown`；
3. bridge 调用 `process.emit("SIGTERM")`，触发 DSH 已注册的正常清理；
4. 等待 5～8 秒；
5. 如果 DSH 未自然退出，调用 `TerminateJobObject` 终止整个进程树；
6. 确认 Job 中没有活动进程、端口已释放、HTTP 健康检查已失效；
7. 三项全部满足后，UI 才显示“已停止”。

窗口右上角 `X` 默认应隐藏到托盘。用户选择“退出启动器”时，应提示“停止 DSH 并退出”。
启动器持有带 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 的 Job handle，避免自身崩溃后留下孤儿 DSH。
若未来要求启动器完全退出而 DSH 继续运行，则需另建独立常驻 supervisor，不纳入第一版。

### 5. 状态机

```text
NotInstalled
Installing
Stopped
Starting
Running
StoppingGracefully
ForceStopping
Updating
Crashed
RollbackRequired
```

所有按钮必须由状态机驱动。例如 `Starting` 和 `StoppingGracefully` 期间不能重复启动或再次更新，
`Updating` 期间不能切换 Runtime，活动任务存在时停止或应用更新必须二次确认。

### 6. 从零安装与目录布局

首次启动时按以下顺序执行：

1. 检查 Windows 版本、CPU 架构、WebView2 和磁盘空间；
2. 默认把大型 Runtime、下载与依赖缓存放在 D 盘；
3. 下载经过启动器兼容性清单验证的便携 Node.js，不修改系统 PATH；
4. 从 npm 官方 registry 获取 DSH dist-tags、精确版本和 integrity；
5. 安装到同一磁盘的 staging 目录，绝不覆盖活动版本；
6. 验证 Node.js、DSH、`sharp`、`node-pty` 和配置 dump；
7. 使用隔离的临时 `DSH_HOME`、随机端口和 `--no-open` 做启动与真正停止 smoke test；
8. 验证成功后将其移动到不可变版本目录并原子更新活动版本指针；
9. 打开官方 DSH Web UI，让 DSH 自己完成模型与凭据配置。启动器不读取或保存 API Key。

建议目录：

```text
D:\Tools\dsh-launcher\
  runtime\node\<node-version>\
  versions\<dsh-version>\
  staging\
  active.json
  previous.json

D:\Caches\dsh-launcher\
  downloads\
  npm\
  logs\

D:\Caches\deepseek-harness\home\
  # 继续作为独立 DSH_HOME
```

现有 `D:\Tools\node-v24.19.0-win-x64`、`D:\Tools\dsh-runtime-0.1.1-rc.2` 和
`D:\Caches\deepseek-harness\home` 应由新启动器检测并导入，不要求重新安装或移动用户数据。

### 7. DSH 检查更新、直接更新与回滚

用户可选择以下通道：

- 推荐版：跟随 npm `latest`；
- Alpha 预览版：跟随 npm `alpha`，必须由用户主动开启；
- 不把 GitHub 最新 Release 或语义化版本最大值直接当作推荐版；
- 离线或更新服务器异常时，不得阻塞已安装版本的正常启动。

更新流程：

```text
检查版本
  -> 后台下载或安装到 staging
  -> 校验来源、长度、integrity/hash 和签名
  -> 隔离 smoke test
  -> 提示结束活动任务
  -> 备份生产 DSH_HOME
  -> 真正停止旧 DSH
  -> 原子切换 active.json
  -> 启动新版本并健康检查
  -> 成功：保留旧版供回滚
  -> 失败：停止新版本并恢复 previous.json
```

禁止使用 `npm update -g`、`npx latest` 或原地覆盖活动 `node_modules`。至少保留当前版本和上一版本。
staging 与 versions 必须位于同一卷，才能使用同卷 rename 和原子指针切换抵抗中断或断电。

当前 rc.2 的 npm 发布包曾因 peer dependency 解析异常需要 `--legacy-peer-deps`，并手工补齐
官方依赖；其原生依赖也包含必要的 install/postinstall。第一版必须为 rc.2 保留精确的
compatibility recipe、lockfile、Node.js 版本和 smoke test，不能简单使用 `--ignore-scripts`，也不能
把 rc.2 的补丁机械应用到未来版本。

面向公开发行时，更安全的方式是在受保护 CI 中制作“精确 Node.js + DSH + 完整生产依赖”的
签名 Runtime Bundle；客户端只下载、验证和解压，不直接运行来自网络的 npm 生命周期脚本。

### 8. 启动器自身更新与供应链安全

启动器自身更新与 DSH Runtime 更新是两条独立通道：

- 启动器使用 Tauri Updater、GitHub Releases 或静态 HTTPS manifest；
- 启动器更新包必须通过 Tauri Ed25519 签名校验；
- 公开发布时再增加 Windows Authenticode 和可信时间戳，降低 SmartScreen 警告；
- DSH Runtime 使用独立签名 manifest，记录通道、版本、Node.js 版本、架构、URL、长度、
  SHA-256、签名、最低启动器版本和兼容/迁移信息；
- manifest 与 Runtime 使用内嵌公钥验证，拒绝降级、重放和被篡改的文件；
- 解压时拒绝绝对路径、`..`、符号链接、重解析点和异常尺寸文件；
- 日志不得记录 API Key、令牌、完整环境变量或用户会话内容；
- 启动器与 DSH 均按当前用户权限运行，不创建管理员服务，不为了方便而请求 UAC。

### 9. UI 与品牌方案

第一版采用单窗口或少量分页，核心界面保持简洁：

```text
DSH Launcher                              设置

           [DSH Launcher 自有终端图标]
           ● 正在运行
           DSH 0.1.1-rc.2 · Node 24.19.0

       [停止 DSH]      [打开 DSH 界面]

更新
推荐版本 0.1.1-rc.2                    [检查更新]

工作区  D:\Bian_CHENG\dsmax
日志                                                ▾
```

界面采用白色主背景、蓝色强调和黑色正文，优先保证高对比度与可读性，并保留键盘导航、
清晰的进度状态与错误详情。应用 Logo 使用本项目自有的“终端 + 启动”几何图形，不复用 DeepSeek
鲸形标识。

若项目公开发布，产品名建议使用 `DSH Launcher`，并在关于页注明“非官方社区启动器，兼容
DeepSeek Harness”，并明确独立维护、未经审核、赞助或背书。上游官方仓库确认命令名缩写为 `dsh`；
截至 2026-09-01 的复核没有找到官方商标或 Logo 再使用授权，因此不得造成官方背书、合作或授权的误解。

### 10. 可借鉴项目与参考资料

- Tauri Updater / Sidecar：借鉴小体积桌面壳、签名更新、权限隔离与 stdout 事件；普通 sidecar
  kill 不能替代 Windows Job Object；
- Stability Matrix：借鉴私有运行时、隔离安装、更新、回滚和实时控制台；其 AGPL/EULA 限制较强，
  只借鉴产品与架构模式，不复制代码或素材；
- Prism Launcher：借鉴运行时探测、版本隔离、启动参数和日志管理；其 GPL-3.0 代码不直接复制；
- Jan：借鉴本地后台服务在应用退出、更新重启和下次启动时的生命周期 reconciliation；
- 所有第三方代码、设计素材和图标在复用前必须单独核对许可证、NOTICE 与品牌要求。

参考资料：

- DeepSeek Harness：<https://github.com/deepseek-ai/deepseek-harness>
- DSH npm 包：<https://www.npmjs.com/package/@deepseek-ai/dsh>
- DSH CLI 关闭行为：<https://github.com/deepseek-ai/deepseek-harness/blob/master/apps/cli/reference/README.md>
- 发布身份与许可证复核：`docs/RELEASE_REVIEW.md`
- Tauri Updater：<https://v2.tauri.app/plugin/updater/>
- Tauri Sidecar：<https://v2.tauri.app/develop/sidecar/>
- Windows Job Object：<https://learn.microsoft.com/windows/win32/procthread/job-objects>

### 11. 实施阶段

#### 第一阶段：生命周期 MVP

- 建立 Tauri + React + Rust 项目；
- 导入并识别现有 Node.js、DSH、DSH_HOME 与工作区；
- 实现单实例、状态机、启动、健康检查、打开 Web UI；
- 实现 bridge 正常清理、Job Object 强制兜底和真正停止；
- 实现托盘、日志、错误详情；
- 保留现有 VBS 快捷方式作为回退。

#### 第二阶段：安装与版本管理

- 私有 Node.js Runtime；
- 干净机器首次安装向导；
- 推荐版/Alpha 通道；
- 版本目录、compatibility recipe、smoke test；
- 手动切换与回滚。

#### 第三阶段：自动更新

- 后台下载与进度；
- 生产 DSH_HOME 备份；
- 原子切换和失败自动回滚；
- 启动器签名自更新；
- 签名 manifest、Runtime Bundle 与发布流水线。

#### 第四阶段：发布质量

- 干净 Windows 虚拟机安装测试；
- 断网、下载中断、断电和启动器崩溃恢复测试；
- Windows Authenticode、SmartScreen 与安装器测试；
- 品牌、许可证、NOTICE、隐私与日志审查。

### 12. 验收标准

- [ ] 连续启动和停止 100 次，不残留 DSH、Node.js、PTY、工具子进程或监听端口；
- [x] 启动器强制退出后，受管 DSH 的 Job 进程树能全部终止；
- [ ] 停止成功前必须同时验证进程树为空、端口释放和健康检查失效；
- [ ] 端口被其他程序占用时只报告冲突，绝不结束该程序；
- [ ] 双击或多个启动器实例不会并发启动两个 DSH；
- [ ] 没有安装 Node.js 或 DSH 的干净 Windows 可以完成首次安装；
- [ ] 更新任一阶段失败或被中断后，旧版本仍可启动；
- [ ] 新版本健康检查失败时可以自动切回旧 Runtime；
- [ ] 被篡改的 manifest、Runtime、更新包或签名必须被拒绝；
- [ ] 推荐版和 Alpha 通道互不混淆，预览版不会静默自动安装；
- [ ] 更新、停止和退出不会静默打断活动中的 DSH 任务；
- [ ] 启动器不读取、复制、记录或上传 DSH 凭据和完整会话数据。

## 十四、第一阶段实施记录（2026-08-31）

第一阶段生命周期 MVP 已完成。当前实现是 `Tauri 2 + React/TypeScript + Rust`，现有 VBS 保留为
回退入口，尚未改动桌面快捷方式。

### 1. 已实现能力

- 单实例启动器；第二次打开会聚焦已有窗口，不会创建第二个受管 DSH；
- 自动识别本机 Node.js、DSH `0.1.1-rc.2`、`DSH_HOME` 和工作区；
- 使用随机本地端口启动 DSH，解析真实 URL，HTTP 200 后才进入 `Running`；
- 使用官方 DSH 鲸鱼 SVG，并提供白色主背景、官方蓝色与黑色搭配的高对比度单页 UI；
- “启动 DSH”“打开界面”“停止 DSH”三个核心操作；
- 关闭窗口时隐藏到托盘；托盘可显示窗口、打开 DSH、停止并退出；
- Rust 在 DSH 启动前创建 Windows Job Object，并设置
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`；
- `dsh-bridge.mjs` 通过私有 stdin 接收 `shutdown`，触发 DSH 的 `SIGTERM` 清理与
  `dispose()`；8 秒仍未退出时使用 `TerminateJobObject` 结束完整进程树；
- 只停止当前启动器持有句柄的实例，绝不按 `node.exe` 名称或端口误杀其他进程；
- 如果固定端口 3080 已有 HTTP 服务，只标记为外部服务，不接管、不结束；
- 内存日志与 UTF-8 轮转日志，日志目录为 `D:\Caches\dsh-launcher\logs`；
- Tauri capability 只开放启动器自身的窄命令，没有通用 Shell 执行权限。

### 2. 关键文件

```text
src/App.tsx                         # UI、轮询、按钮与停止确认
src/styles.css                      # 启动器视觉样式
public/dsh-logo.svg                 # 官方 DSH 鲸鱼标志
src-tauri/src/lib.rs                # 状态机、进程监督、Job、健康检查、托盘
src-tauri/resources/dsh-bridge.mjs  # DSH 引导与正常关闭桥接
src-tauri/capabilities/default.json # 最小 Tauri 权限
scripts/dev.ps1                     # 开发运行
scripts/check.ps1                   # 前端、格式与 Rust 测试
scripts/build.ps1                   # Release 构建
```

### 3. 开发环境与命令

Rust 工具链也安装在 D 盘，避免持续占用 C 盘：

```text
RUSTUP_HOME = D:\Tools\rust\rustup
CARGO_HOME  = D:\Tools\rust\cargo
rustc       = 1.98.0
cargo       = 1.98.0
```

从项目根目录运行：

```powershell
& '.\scripts\dev.ps1'
& '.\scripts\check.ps1'
& '.\scripts\build.ps1'
```

脚本会设置 D 盘 Node、Rust、npm 缓存和 Cargo target 路径，无需修改系统 PATH。

### 4. 已完成验证

- `npm run check`：通过；
- `npm run build`：通过；
- `cargo check`：通过；
- `cargo fmt --check`：通过；
- 普通 Rust 测试：1 项通过，1 项真实生命周期测试默认忽略；
- 真实 DSH 生命周期集成测试：通过。测试使用独立临时 `DSH_HOME`，验证启动、动态端口、
  HTTP 200、bridge 正常关闭以及健康端点失效；
- Release 构建：通过，产物为
  `D:\Bian_CHENG\dsh-launcher\src-tauri\target\release\dsh-launcher.exe`；
- 集成测试结束后没有残留启动器或 DSH Node 进程。

真实生命周期测试可按需单独运行：

```powershell
. '.\scripts\Initialize-DevEnvironment.ps1'
cargo test --manifest-path '.\src-tauri\Cargo.toml' real_dsh_lifecycle -- --ignored --nocapture
```

### 5. 当前限制

- 第一阶段只导入本机现有 Node/DSH 路径，尚未实现从零安装、版本通道、更新与回滚；
- 尚未制作安装器，`tauri.conf.json` 的 bundle 当前关闭；Release EXE 应从本项目构建目录运行，
  不能把单个 EXE 当作便携发行包复制到其他电脑；
- Release EXE 当前未做 Authenticode 签名，公开分发前必须进入第三、四阶段；
- 自动化环境因本机 Windows `CreateProcess` 访问被拒，未能完成可见 GUI 的自动冒烟；核心后端
  已通过真实 DSH 生命周期集成测试，首次人工验收应运行 `scripts/dev.ps1` 检查窗口、托盘和浏览器；
- 第一阶段没有把桌面快捷方式切到新程序，旧 VBS 仍是日常入口与可靠回退；
- “停止 DSH”会中断正在执行的任务，UI 已提供二次确认，但无法判断 Web UI 内是否存在活动任务。

### 6. 第二阶段入口

下一阶段按本方案第十三节继续：建立版本化 Runtime 目录与 `active.json`，实现干净机器首次安装、
推荐版/Alpha 通道、compatibility recipe、隔离 smoke test、手动切换和回滚。完成安装器与人工 GUI
验收前，不应替换现有桌面快捷方式。

## 十五、浅色高对比度 UI 调整（2026-08-31）

根据第一阶段人工预览反馈，原深色主题的次要文字、路径与禁用按钮对比度不足，现已统一调整：

- 页面和卡片以白色为主，应用外围使用非常浅的蓝灰色区分层级；
- 正文、标题和路径使用近黑色与深灰色，避免低亮度蓝灰字；
- 官方品牌蓝 `#4D6BFE` 用于视觉强调，主按钮使用更深蓝色保证白字可读；
- 运行、处理中、异常状态使用高对比度绿、蓝、橙红色组合；
- 禁用按钮不再通过低透明度处理，改用实色灰底、灰字和清晰边框；
- 日志区改为浅灰背景，时间、级别、正文和错误分别使用可辨识颜色；
- 增加键盘焦点轮廓；
- 启动器内的官方 Logo 固定使用黑色路径，避免 Windows 处于深色模式时 SVG 自动变白，导致
  Logo 在启动器白色背景上不可见。

## 十六、Release 路径与托盘修复（2026-08-31）

人工验收发现 Release 构建通过 Tauri 解析资源时会得到 `\\?\D:\...` 形式的 Windows verbatim
路径。Node.js 24 不能把该形式作为主模块路径使用，会将其错误解析为 `D:` 并以 `EISDIR`、
`exit code: 1` 退出。启动器现在会在创建 Node 子进程前把本地磁盘和 UNC verbatim 路径转换为
等价的普通 Windows 路径，并增加纯函数回归测试。真实 DSH 生命周期测试也会主动使用 verbatim
形式的 bridge 路径，端到端覆盖 Release 中的实际失败分支。

托盘行为同时加强：

- 托盘使用固定 ID `dsh-launcher-tray` 并保留官方鲸鱼图标；
- 左键单击托盘图标直接恢复并聚焦主窗口；
- 点击窗口 `X` 前先确认托盘图标仍已注册；
- 如果托盘图标不可用，拒绝隐藏窗口并记录错误，避免启动器失联；
- 成功隐藏窗口时写入“主窗口已隐藏到系统托盘”日志。

Windows 可能默认把鲸鱼图标放在任务栏 `^` 的隐藏图标区，这仍属于正常托盘运行状态。

## 十七、第一阶段当前验收状态（2026-08-31）

第一阶段现已完成开发、修复与当前一轮人工界面验收，状态如下：

- 浅色高对比度 UI 已由用户实际打开确认，白色主背景、黑色正文、官方蓝色强调均正常显示；
- 首次 Release 人工启动暴露的 `EISDIR: lstat 'D:'` 已定位为 Tauri verbatim 资源路径与
  Node.js 24 主模块路径不兼容，并已修复；
- 路径转换单元测试通过；使用同款 verbatim bridge 路径的真实 DSH 启动、HTTP 200 健康检查、
  bridge 正常清理和完整停止测试通过；
- 普通前端检查、构建、Rust 测试和 Release 构建均通过；
- 最新 Release 产物为
  `D:\Bian_CHENG\dsh-launcher\src-tauri\target\release\dsh-launcher.exe`；
- 测试结束后没有残留 `dsh-launcher.exe` 或受管 DSH `node.exe` 进程；
- 用户曾观察到“关闭窗口后托盘图标消失”，后来确认当时同时关闭了 `scripts/dev.ps1` 弹出的
  终端窗口。开发模式下终端是启动器开发进程的宿主，关闭终端会结束启动器，因此托盘图标随之
  消失，这是预期行为，不是托盘隐藏失败；
- 日常使用应直接运行 Release EXE。Release 使用 Windows GUI 子系统，不依赖外部终端；点击主窗口
  `X` 只隐藏到系统托盘，任务栏 `^` 隐藏图标区中的黑色鲸鱼即为 DSH Launcher，左键可恢复窗口；
- `scripts/dev.ps1` 仅供开发调试，使用期间不要关闭其终端；需要结束开发实例时可在终端中停止，
  或使用托盘菜单“停止 DSH 并退出”；
- 旧 VBS 与原桌面快捷方式继续保留为回退入口，尚未自动切换；签名自动更新、生产数据备份与
  更新失败自动回滚仍属于第三阶段。

截至本记录，第一阶段生命周期 MVP 可以收尾。后续若桌面快捷方式切换到 Release EXE，应再完成
一次快捷方式启动、DSH 启动、窗口隐藏/恢复、真正停止和托盘退出的最终桌面入口验收。

## 十八、第二阶段实施记录（2026-08-31）

第二阶段“安装与版本管理”已完成，启动器版本提升为 `0.2.0`。实现继续保留第一阶段的进程监督、
Job Object 和托盘行为，并新增以下能力。

### 1. Runtime Manager 与目录契约

- 默认将大型文件放在 `D:\Tools\dsh-launcher` 与 `D:\Caches\dsh-launcher`；如果 D 盘目录不可写，
  自动回退到当前用户的 `LOCALAPPDATA`；
- 私有 Node 位于 `runtime\node\<node-version>`，DSH 位于 `versions\dsh-<dsh-version>`，安装过程只
  写入同卷 `staging`，通过全部验证后才 rename 到不可变版本目录；
- `active.json` 与 `previous.json` 使用 Windows `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` 原子
  替换；切换或回滚只修改指针，不原地覆盖 Runtime，也不删除 DSH_HOME；
- 首次启动会识别并登记原有 `D:\Tools\node-v24.19.0-win-x64` 与
  `D:\Tools\dsh-runtime-0.1.1-rc.2`，不要求重新下载或移动用户数据；
- `DSH_LAUNCHER_RUNTIME_ROOT` 与 `DSH_LAUNCHER_CACHE_ROOT` 可在自动测试中覆盖默认目录。

### 2. 首次安装与兼容性验证

- 首次安装向导显示 Windows、x64、WebView2、至少 3 GB 可用空间、Runtime 根目录和缓存目录；
- 私有 Node 固定从 `nodejs.org` 下载 Windows x64 ZIP，限制下载与解压大小，校验官方 SHA-256，
  解压时拒绝绝对路径、`..` 与符号链接；当前 recipe 使用 Node `24.20.0`，ZIP SHA-256 为
  `6cac9ffbca8f6a47091e4b5c772e0606049c3871cb67d900c0cedde630e545ba`；
- npm 只使用官方 registry，安装精确 DSH 版本，正常执行必要的 install/postinstall，不使用
  `--ignore-scripts`；安装后核对 registry manifest 与 `package-lock.json` 中的 SHA-512 integrity；
- `src-tauri/resources/compatibility-recipes.json` 固化 rc.2 的 `--legacy-peer-deps`、完整补充依赖、
  Node 版本和 integrity；Alpha 0.1.2-alpha.2 使用独立 recipe，不能混用 rc.2 补丁；
- 未内置的未来版本先走标准精确安装，仍必须通过完整 smoke test 才会进入版本目录；
- smoke test 使用隔离临时 DSH_HOME 和随机端口，依次验证 `dsh --version`、`sharp`、`node-pty.spawn`、
  Web profile 配置 dump、HTTP 200、页面标题 `DeepSeek Harness`、模块缺失错误扫描，以及 bridge 正常
  dispose/Windows Job 真正停止；失败会删除 staging，活动 Runtime 不受影响。

### 3. 推荐版、Alpha、切换与回滚

- “推荐版”严格跟随 npm `latest`，“Alpha”严格跟随 npm `alpha`；截至 2026-08-31，分别为
  `0.1.1-rc.2` 与 `0.1.2-alpha.2`；断网或 registry 异常只让检查/安装失败，不阻塞已安装版本启动；
- Alpha 必须由用户主动选择并再次确认，不会静默安装；
- 新版本完成旁路安装与 smoke test 后不会覆盖当前活动版本，用户可在列表中手动切换；
- 切换前必须停止受管 DSH，原活动版本写入 `previous.json`；用户可手动回滚，失败 Runtime 启动或
  意外退出时状态进入 `RollbackRequired`；
- 首次安装是唯一会在验证成功后立即激活新 Runtime 的流程，因为此时没有旧活动版本。

### 4. UI 与后端接口

- 前端新增首次安装向导、通道选择、安装进度、版本检查、已安装 Runtime 列表、手动切换、回滚和
  活动路径/私有 Runtime 标识；
- 后端新增 `check_runtime_versions`、`set_runtime_channel`、`install_runtime_channel`、
  `switch_runtime` 与 `rollback_runtime` 窄命令；仍未开放通用 shell 权限；
- DSH 运行、启动、停止或 Runtime 操作进行期间，安装/切换/回滚按钮由状态机锁定。

### 5. 已完成验证

- `npm run check`：通过；
- `npm run build`：通过；
- Rust 默认测试：5 项通过，真实生命周期与完整安装测试默认忽略；
- 真实“干净 Runtime 根目录”安装测试：通过。测试实际从 `nodejs.org` 下载并校验 Node ZIP，使用
  npm 官方 registry 完整安装 rc.2 recipe，验证 integrity、CLI、原生模块、隔离 Web 启停、页面标题、
  Job Object 停止和活动指针，最后删除一次性 Runtime 与测试缓存；
- 完整 Release 检查与构建仍应使用 `scripts/check.ps1` 和 `scripts/build.ps1`。

### 6. 第三阶段边界

第二阶段不会后台静默下载或切换，也不包含签名 Runtime Bundle、签名 manifest、生产 DSH_HOME
备份、失败自动切回、启动器签名自更新或发布流水线。这些供应链和无人值守更新能力继续留在第三
阶段。旧 VBS 与桌面快捷方式仍保留，不自动替换。

## 十九、第三阶段开发记录（2026-08-31）

第三阶段“签名自动更新”的本地实现已开工并完成主体链路，启动器版本提升为 `0.3.0`。仓库没有配置
真实 Git 远端、发布地址或生产签名私钥，因此普通本地构建坚持安全关闭远程更新；发布流水线注入真实
HTTPS 端点与公钥后才会启用。

### 1. 签名 Runtime 供应链

- 新增 Ed25519 签名 envelope；签名覆盖 manifest 原始 payload，Bundle 签名覆盖带域分隔符的
  SHA-256 摘要；客户端只内嵌公钥；
- manifest 固定包含 sequence、签发/过期时间、通道、DSH/Node 版本、架构、HTTPS URL、长度、
  SHA-256、Bundle 签名、package integrity、recipe、最低启动器版本和迁移声明；
- 本地持久化最高 sequence 与 payload digest，拒绝降级、旧 sequence 和同 sequence 不同 payload；
- 推荐版与 Alpha 按签名通道和 Windows x64 架构严格筛选，低版本或等版本不会作为更新返回；
- Bundle 流式下载到缓存临时文件，逐块更新 UI 进度；长度、SHA-256 和 Ed25519 全部通过后才提交；
- 解包拒绝绝对路径、`..`、符号链接、超限条目/体积、NTFS ADS、保留设备名、尾随点/空格和异常路径；
- Bundle 在发布机提前包含完整 Node 与 DSH 生产依赖。客户端更新不会运行 npm 或任何网络生命周期脚本。

### 2. 备份、原子切换与自动回滚

- 已验证 Bundle 先进入不可变旁路版本目录并使用隔离 DSH_HOME 完成完整 smoke test；下载不打断正在
  运行的生产 DSH；
- 用户点击“备份并切换”且明确确认后，启动器才停止受管 DSH；
- 生产 DSH_HOME 复制到缓存 `backups` 的 staging，拒绝跟随符号链接、junction 和重解析点，完整复制后
  以目录 rename 提交；日志只记录备份位置，不读取或输出凭据与会话内容；
- `active.json` 继续使用写穿原子替换。新 Runtime 激活后必须在生产 DSH_HOME 上启动并通过 HTTP 健康
  检查；失败会自动恢复 `previous.json`、重新启动旧 Runtime，并把结果显示为 rolled-back；
- manifest 声明未知的必需数据迁移时会在切换前拒绝执行，避免未经实现的破坏性迁移。

### 3. 启动器自更新与发布流水线

- 接入 Tauri 官方 updater/process 插件，能力只开放 check、download-and-install 和 restart；
- 发布配置启用 `createUpdaterArtifacts`、被动安装和签名 updater；普通配置不含端点与公钥；
- 新增 `Build-RuntimeBundle.ps1`、`sign-runtime-release.mjs` 和 GitHub Actions Windows 发布流水线；
- 流水线从精确 compatibility recipe 构建 Bundle、验证 Node SHA-256/npm integrity/CLI/原生模块，合并另一
  通道上一份签名 release，再签名并发布 Runtime manifest 与启动器 updater；
- 私钥只通过 Actions secrets 提供。密钥生成、secret 名称与发布步骤见 `docs/RELEASE.md`；
- Windows Authenticode、SmartScreen 和干净虚拟机安装器测试仍属于第四阶段，不能用 updater 签名替代。

### 4. 当前验证

- `scripts/check.ps1`：通过；前端类型检查与构建通过；
- Rust 默认测试共 11 项：9 项通过，2 项真实联网/本机生命周期测试按设计忽略；
- 新增测试覆盖 manifest 篡改、旧 sequence/同 sequence 篡改、Bundle 摘要篡改、非 HTTPS URL、ZIP 路径
  逃逸、NTFS ADS 与 Windows 设备名；
- `scripts/build.ps1` 已修复原生命令失败传播和陈旧产物误判，并成功构建 `0.3.0` Release EXE；
- 当前 Release 产物：`src-tauri\target\release\dsh-launcher.exe`。

### 5. 外部发布闸门

在没有用户提供或配置真实仓库、HTTPS 发布端点与两套生产签名密钥之前，不能声称已经完成一次真实签名
发布或真实在线升级。代码和流水线已就绪，本地构建明确显示“未配置”并拒绝检查，不会回退到 npm 在线
安装或不签名下载。

## 二十、第三阶段最终验收与第四阶段入口（2026-09-01）

第三阶段已经完成真实公开发布、在线自更新、生产数据备份、Runtime 切换和生产健康检查，不再受第十九节
“外部发布闸门”限制。第十九节保留为开发过程记录，本节状态为后续维护的当前基线。

### 1. 公开仓库与签名发布

- 公开仓库：`https://github.com/1424801913dd-cmd/dsh-launcher`；
- 最新公开启动器：`app-v0.3.3`，Release 地址为
  `https://github.com/1424801913dd-cmd/dsh-launcher/releases/tag/app-v0.3.3`；
- `0.3.3` 安装包、Tauri updater 签名和 `latest.json` 已由 GitHub Actions 生成并公开；
- Runtime 资产位于公开但面向机器读取的 `runtime-channels` Release；当前签名 manifest sequence 为 `4`；
- 第三阶段最终成功流水线：`https://github.com/1424801913dd-cmd/dsh-launcher/actions/runs/33413289797`；
- 发布使用的 Runtime Ed25519 与 Tauri updater 私钥只保存在 GitHub Actions Secrets 和本机受保护的
  `D:\Secrets\dsh-launcher`；不得读取、提交、打印或复制私钥内容；
- 当前收尾提交为 `4426f4f`（`fix: exclude generated module fallback from backups`）。

### 2. 真实在线升级中发现并修复的问题

- `0.3.1` 修复健康检查 URL 规范化时在 token 查询参数后追加 `/`、导致访问令牌失效的问题；
- `0.3.2` 支持 DSH 返回的本地 `303 + Set-Cookie` 健康检查流程，只允许最多五次且始终限制在同一
  `http://127.0.0.1:<port>`，不会把 Cookie 或 token 发送到其他主机或端口；
- `0.3.3` 修复生产备份被 `$DSH_HOME/profiles/node_modules` 内 Runtime 生成的 Junction 阻止的问题；
- 备份现在只排除精确的共享模块映射目录 `profiles/node_modules`。该目录由 DSH 在启动时按当前安装自动
  重建和重新指向；配置、会话、存储、附件、profile 文件和 profile 自己的依赖目录仍会备份；
- 除上述精确目录外，备份继续拒绝任何符号链接、Junction、重解析点和特殊文件，不能为了兼容而放宽。

### 3. 当前生产验收状态

- `active.json`：`signed-0.1.2-alpha.2`，Node `24.20.0`，`managed=true`，`smokeTested=true`；
- `previous.json`：`legacy-0.1.1-rc.2`，可由启动器执行回滚；
- 当前生产 DSH 由受管 Node
  `D:\Tools\dsh-launcher\versions\dsh-0.1.2-alpha.2\node\node.exe` 运行；
- 生产切换后随机 loopback 端口健康检查通过，日志记录“签名 Runtime 0.1.2-alpha.2 已生效”；
- 本次生产备份：`D:\Caches\dsh-launcher\backups\dsh-home-1788230783283`；
- 已核验备份包含 `settings.yaml`、`sessions`、`storages`、`attachments` 与 `profiles/web`，不含生成的
  `profiles/node_modules`，且备份目录内没有重解析点；
- `scripts/check.ps1` 在 `0.3.3` 上通过：Rust 测试 12 项通过、0 项失败、2 项真实环境测试按设计忽略；
- 本机安装目录 `D:\Bian_CHENG\dsh-launcher\DSH Launcher` 是用户选择的程序安装目录，不属于源码，
  Git 中保持未跟踪，不能误提交；
- 本轮 `.tmp-e2e` 下载和审计临时目录已删除。

### 4. 第四阶段开工边界

第四阶段按第十三节“发布质量”推进，不再重复第三阶段功能。建议顺序如下：

1. 盘点可用的 Windows Authenticode 代码签名证书、密钥托管方式和 RFC 3161 可信时间戳服务；在证书
   未确定前先完成可自动化的安装器与故障注入测试，不得用 Tauri updater Ed25519 签名冒充 Authenticode；
2. 建立干净 Windows 10/11 x64 虚拟机验收矩阵，覆盖无 Node、无 DSH、无开发工具的首次安装、启动、
   更新、卸载和数据保留；
3. 增加断网、下载中断、损坏资产、进程崩溃以及 staging/指针切换中断恢复测试，并证明旧 Runtime
   始终可启动；
4. 完成安装器签名、签名验证、SmartScreen 观测与安装/卸载回归；SmartScreen 声誉状态必须如实记录，
   不能把“已签名”等同于“不会警告”；
5. 审查品牌、MIT `LICENSE`、第三方 `NOTICE`、隐私说明、日志脱敏和 Release 文案；
6. 执行第十三节验收清单，包括连续启停 100 次与强制退出后的 Job 进程树清理，形成可复现报告。

第四阶段开始前无需重新生成 Runtime/Tauri 密钥，也不要删除当前 Alpha、上一 rc.2 或本次生产备份。

## 二十一、第四阶段首批实施记录（2026-09-01）

第四阶段“发布质量”已经开工，源码版本提升为 `0.4.0`。当前公开安装版仍为 `0.3.3`；在安装器、
Authenticode、干净虚拟机和发布验收完成前，不得把本地 `0.4.0` 构建描述为生产发布版。

### 1. 故障注入与缓存提交边界

- Runtime Bundle 下载、校验和缓存提交已拆成可注入输入流的确定性测试边界；
- 新测试覆盖下载提前结束和同长度内容损坏，两种情况下都必须保留上一份已验证缓存、拒绝提交新文件，
  并且不留下 `.part-*` 半文件；
- 修复了 Windows 下载失败路径仍持有输出文件句柄、可能导致半文件清理失败的问题；
- 已验证 Bundle 现在通过 `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` 原子替换缓存，不再先删除旧缓存
  再 rename，断电窗口内仍保留旧缓存或完整新缓存；
- 新增 staging/活动指针中断测试：未提交的 staging Runtime 和 `.active.json.*.tmp` 不会进入 Runtime
  catalog，现有 `active.json` 及其 Node/DSH 入口继续有效。

### 2. 日志隐私修复

- 审查发现旧实现会把 DSH stdout 中带访问 token 的 loopback URL 原样写入内存和持久日志；
- 所有日志现在先统一脱敏，再进入 UI 日志队列或磁盘日志：URL 查询参数/片段、token、API Key、
  Authorization、Cookie、password 和 secret 值会替换为 `[REDACTED]`；
- 新增幂等回归测试，避免重复启动后对已脱敏文本产生二次破坏；
- 启动器启动时会检查当前和上一份历史日志，使用同卷临时文件和原子替换完成旧日志脱敏；失败时只记录
  通用警告，不把原始错误内容再次写入日志；
- 本机现有 `D:\Caches\dsh-launcher\logs\launcher.log` 已原子脱敏。脱敏前只统计到 6 处敏感模式，
  未打印或复制其值；脱敏后 URL 查询和敏感字段未脱敏匹配均为 0；
- 新增 [隐私说明](docs/PRIVACY.md)，记录本地数据、备份、日志、网络连接和卸载保留边界。

### 3. 可重复发布质量检查

新增 `scripts/phase4-check.ps1`：

```powershell
# 本地基线；未签名和缺少安装器显示为 WARN
& '.\scripts\phase4-check.ps1' -RunProjectChecks `
  -ReportPath 'phase4-results\baseline.json'

# 正式发布硬闸门；未签名或没有安装器直接失败
& '.\scripts\phase4-check.ps1' -RunProjectChecks `
  -RequireAuthenticode -RequireInstaller `
  -ReportPath 'phase4-results\release-gate.json'
```

检查项包括前端/Rust 测试、必要文档、Release EXE、安装器制品、每个制品的 Authenticode 状态，以及
当前用户/本机证书库内是否存在带私钥且未过期的代码签名证书。机器相关 JSON 报告写入已忽略的
`phase4-results`，不提交仓库。

### 4. 当前验证与 Authenticode 基线

- `scripts/check.ps1` 通过：Rust 共 18 项测试，16 项通过、0 项失败、2 项真实联网/生命周期测试按设计
  忽略；前端类型检查、构建和 Rust 格式检查通过；
- `scripts/build.ps1` 成功生成本地 `0.4.0` Release EXE：
  `src-tauri\target\release\dsh-launcher.exe`；
- 当前源码 Release EXE、生产安装目录中的 `dsh-launcher.exe` 和 `uninstall.exe` 均为 `NotSigned`；
- `Cert:\CurrentUser\My` 与 `Cert:\LocalMachine\My` 当前没有带私钥的可用代码签名证书；
- 本地阶段四配置已经生成一个 NSIS 安装器，因此质量基线中的 `installer-artifacts` 已由 `WARN` 变为
  `PASS`；安装器与 Release EXE 仍为 `NotSigned`，`authenticode` 和 `code-signing-certificate` 继续是
  预期 `WARN`，不能改写为通过；
- 已用 `-RequireAuthenticode -RequireInstaller` 实际运行发布硬闸门，脚本按预期以退出码 1 拒绝当前
  未签名构建，证明警告不会在正式发布检查中被误当成通过。

### 5. 本地 NSIS 安装器与隔离测试入口

- `tauri.conf.json` 固定 NSIS 为当前用户安装、禁止降级、中英文资源、LZMA 压缩、独立开始菜单目录，
  并为安装器/卸载器使用项目图标；普通源码构建仍保持 `bundle.active=false`；
- `src-tauri/tauri.phase4.conf.json` 只为本地质量测试开启 NSIS，不生成 updater 制品，也不注入生产更新
  公钥或端点；
- `scripts/build-unsigned-installer.ps1` 显式使用 `--no-sign`，已生成：
  `src-tauri\target\release\bundle\nsis\DSH Launcher_0.4.0_x64-setup.exe`；当前大小约 2.01 MiB，
  文件与产品版本均为 `0.4.0`，Authenticode 状态为 `NotSigned`；
- Tauri bundler 在 Windows 上通过 Known Folder API 固定读取 `%LOCALAPPDATA%\tauri`，不接受进程级
  `LOCALAPPDATA` 覆盖。本机已把该精确目录迁移到 `D:\Caches\dsh-launcher\tauri-bundler-tools`，并在
  C 盘原位置建立目录联接；当前 NSIS 3.11 工具缓存约 6.84 MiB。第二次构建直接复用 D 盘缓存，未重新
  下载；构建脚本会校验联接目标，避免后续缓存静默回到 C 盘；
- `scripts/test-installer.ps1` 覆盖静默安装、版本与资源文件、静默卸载、可执行文件/快捷方式移除和独立
  数据 sentinel 保留；必须显式传入 `-IsolatedEnvironment`，并在发现本仓库内生产安装或启动器进程时
  先于任何写入拒绝运行；本机保护分支已经实测通过；
- 当前宿主机没有 Windows Sandbox 或 Hyper-V，同时存在 `DSH Launcher\` 生产安装目录，因此没有在
  宿主机执行同标识安装器，避免覆盖公开版 `0.3.3`；
- 新增手动工作流 `.github/workflows/phase4-installer.yml`，在一次性 GitHub Windows runner 上构建未签名
  NSIS、执行静默安装/卸载脚本并上传安装器与 JSON 报告。该工作流尚未提交、推送或真实运行；
  `windows-latest` runner 也不能替代 Windows 10/11 客户端虚拟机矩阵。

### 6. 当前生产 Alpha 连续启停 100 次

- 真实生命周期测试新增 `DSH_LAUNCHER_LIFECYCLE_CYCLES`，支持 1～1000 次循环；每轮依次验证启动成功、
  HTTP 健康检查成功、正常停止、原端口健康检查失效，以及受管进程/Job 句柄释放；
- `scripts/lifecycle-soak.ps1` 默认执行 100 次，拒绝在 `dsh-launcher.exe` 正在运行时开始，并在测试结束后
  通过 `Win32_Process` 检查任何命令行包含 `dsh-bridge.mjs` 的残留 Node 进程；
- soak 默认读取 `D:\Tools\dsh-launcher\active.json`，本轮实际使用
  `signed-0.1.2-alpha.2`、DSH `0.1.2-alpha.2`、Node `24.20.0`，测试数据位于一次性隔离
  `DSH_HOME`，没有读取或改写生产用户数据；
- 100/100 次全部通过，Rust 测试耗时 245.65 秒，端到端报告总耗时 246.501 秒，退出码为 0；
- 结束后 `D:\Caches\dsh-launcher\tests` 为空，受管 DSH 进程数为 0；连续运行新增日志中的未脱敏 URL
  查询和敏感字段匹配也均为 0；
- 报告位于已忽略的 `phase4-results\lifecycle-soak-100.json`。该项完成了第十三节“连续启动和停止
  100 次”的本机证据。

### 7. 启动器强制退出后的 Job 进程树清理

- 新增 Windows 集成测试 `src-tauri/tests/job_crash.rs` 和入口脚本 `scripts/job-crash-test.ps1`；测试使用
  一次性隔离 `DSH_HOME` 启动当前活动 Runtime，并拒绝在真实启动器进程运行时执行；
- 测试监督进程创建带 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 的 Job Object，把 DSH 进程加入 Job，等待
  HTTP 健康检查通过后直接执行 `process::exit(86)`，不发送正常停止指令，也不运行 Rust 析构逻辑；
- 本轮实际使用 `signed-0.1.2-alpha.2`、DSH `0.1.2-alpha.2`、Node `24.20.0`。监督进程异常退出后，
  受管 DSH PID 在 15 秒闸门内终止，原回环 URL 健康检查失效，测试以退出码 0 通过；
- 端到端耗时 4.116 秒；结束后残留的 `dsh-bridge.mjs` Node 进程数为 0，一次性测试根目录已删除，
  `D:\Caches\dsh-launcher\tests` 为空；
- 报告位于已忽略的 `phase4-results\job-crash.json`。这提供了第十三节“启动器强制退出后受管 DSH
  Job 进程树全部终止”的本机证据。

### 8. 第三方许可证清单与发布复核闸门

- 新增 `scripts/license-audit.mjs`，离线读取 `package-lock.json`、Windows x64 的 `cargo metadata --locked`
  结果和当前活动 Runtime 锁文件；报告与 NOTICE 都从锁定依赖图生成，不依赖手工抄写；
- 当前清单覆盖启动器 JavaScript 生产依赖 6 个、Windows Rust 依赖 332 个，以及
  `signed-0.1.2-alpha.2` Runtime 实际安装依赖 521 个；Runtime 锁文件中的跨平台依赖为 580 个，未随
  当前 Windows x64 Runtime 安装的 59 个不会再误判为发行内容。三组依赖的 license 元数据缺失数均为
  0，Node.js 顶层 `LICENSE` 也已确认存在；
- 生成的 `docs/THIRD_PARTY_NOTICES.md` 约 39 KiB。`phase4-check.ps1` 会重新计算依赖图并验证该文件
  没有过期；审计 JSON 位于已忽略的 `phase4-results\license-audit.latest.json`；
- 实际复核项从 19 个收敛为 6 个。5 个 MPL-2.0 Rust 包已在 NOTICE 中记录精确版本 crates.io 源码地址，
  状态为 `documented-source-availability`；实际携带的 `@img/sharp-win32-x64@0.35.4` 仍为唯一
  `manual-review-required` 项；
- `docs/RELEASE_REVIEW.md` 固化非官方身份、Logo 边界、官方一手依据、MPL 处置和 sharp/libvips 的
  LGPL 待确认事项。原 DeepSeek 鲸形 SVG 及全部应用图标已替换为本项目自有的中性启动器图形；
- `phase4-check.ps1 -RequireLicenseReview` 现在只对没有处置记录的真实发行项失败，因此仍会由上述 1 个
  LGPL 项拒绝发布；
- Tauri 配置将项目 MIT `LICENSE` 设为安装器许可证文件，并把项目许可证、隐私说明、发布复核和第三方
  NOTICE 映射到安装目录 `resources`；四个源文件与 Release 资源副本的 SHA-256 已逐一确认一致；
- `phase4-results\phase4-check-runtime-locks-final.json` 中项目检查、品牌身份、Logo 来源、两套完整 Runtime
  锁、许可证元数据和 Runtime 清单均通过；`phase4-results\release-gate-runtime-locks-final.json` 仍按预期
  只因 1 个 LGPL 人工复核项、内部
  EXE/安装器未签名和本机无代码签名证书而失败；
- 新增 `scripts/runtime-license-evidence.ps1`，使用 PE 导入表记录 sharp 原生模块与 libvips DLL 的动态链接
  边界；报告位于 `phase4-results\runtime-license-evidence.json`。新增单元测试确认安装后替换原生 DLL
  不会被 Runtime 记录哈希拦截。libvips 本体具备替换路径，但其 DLL 不导入 README 所列多项 LGPL
  组件；结合上游 Windows static web release 说明，静态并入组件的重链接材料仍是唯一许可证硬阻塞；
- `Build-RuntimeBundle.ps1` 现在为每个实际构建的 Runtime 生成精确安装依赖 NOTICE 与 JSON 报告，并在
  `app` 内随包放入 GNU GPLv3/LGPLv3 全文、`docs/RUNTIME_LICENSES.md` 模板实例及原生包自带材料；模板
  会从本次锁文件解析 sharp 与 sharp-libvips 精确版本，不复用当前开发机版本；
- `.github/workflows/release.yml` 在任何 Runtime 签名和上传前读取该 JSON，只要仍有
  `manual-review-required` 项就立即失败；通过后还会把许可证报告与 Runtime 资产一起发布。当前 sharp
  LGPL 项会有意阻止该工作流，避免材料尚未人工确认时误发；
- 完整 Bundle 实测发现原流程只固定顶层 `@deepseek-ai/dsh@0.1.2-alpha.2` integrity，重新解析时约两百个
  内部 `@deepseek-ai/dsh-*` 依赖已漂移到 `0.1.2-alpha.3`；该测试构建没有签名或发布。现在新增
  `scripts/prepare-runtime-lock.mjs` 和版本化完整锁，Bundle 构建改为 `npm ci`，校验锁文件前后 SHA-256、
  顶层 integrity 和所有内部 DSH 包精确版本，缺锁时在联网前失败关闭；
- RC2 完整锁含 509 个跨平台包，SHA-256
  `F50E5321334C02D6A7D957B765B75C1FFF49215BF3E3CC65A51571468D3F9DB5`；真实 Windows x64 Bundle
  安装 450 个包。Alpha.2 完整锁含 580 个跨平台包，SHA-256
  `660BEE1F294CA61B1C82F37292865622131D69814575C11D682892A50C764AEC`；真实安装 521 个包；
- 两个通道均已完成完整 Bundle 构建、原生模块 smoke test、ZIP 内 lock 哈希一致性和 7 类许可证材料
  存在性验证。RC2 测试 ZIP 为 101,685,081 字节，SHA-256
  `D5DFAFF1B6913344D89DBED5151384970E30EF90FA20961417D99F068A0E0781`；Alpha.2 为 102,922,190 字节，
  SHA-256 `8CA53EC893D553FCAA30A34BD5456BF1BB56D7C149C90DC3FB66FD81450D0FE4`。两个约 100 MiB 的测试 ZIP
  已在验证后清理，只保留 `phase4-results\runtime-bundle-final-*` 下的审计 JSON 和元数据；
- 新增 `docs/RELEASE_TEMPLATE.md`，固定非官方/独立维护/未获背书、上游 developer preview、隐私、数据
  保留、许可证材料和发布前 Authenticode/虚拟机/SmartScreen 闸门文案；发布工作流会替换启动器版本、
  Runtime 通道和 DSH 版本后作为 draft Release 正文。`tauri-action` 已按官方当前文档更新到 `v1`；
- 最新未签名 NSIS 大小为 2,152,290 字节，SHA-256 为
  `845444E52DB3232011DEA932C0735671117214558A66E74C06B3996C077EE3E7`，Authenticode 仍为
  `NotSigned`。

### 9. 更新故障注入与自动恢复

- 新增 `scripts/update-health-fault-test.ps1`。候选 Runtime 的 DSH 入口在健康检查可用前直接
  `process.exit(42)`；启动器拒绝该候选并回滚活动指针，随后使用 `signed-0.1.2-alpha.2` 重新启动、
  通过健康检查并真正停止。端到端耗时 5.526 秒，残留受管进程为 0，报告位于
  `phase4-results\update-health-fault.json`；
- 新增真实回环 TCP 断流测试：服务端声明 1,024 字节，只发送 128 字节后关闭连接。下载失败不会替换
  已验证旧缓存，正常错误路径会关闭句柄并立即删除 partial；
- 新增下载进程强杀测试：辅助测试进程进入流式写入后被父进程终止。旧缓存仍保持原字节，下一次下载
  会只识别并清理同目标、严格数字时间戳命名的 `.part-*` 残留；异常类型或重解析点会被拒绝；
- 新增 `scripts/backup-crash-test.ps1`。备份完成复制、提交 staging 前，辅助进程执行
  `process::exit(87)`；活动 Runtime 指针不变，下一次备份会清理严格命名的
  `.dsh-home-<时间戳>.staging`，且绝不跟随符号链接或重解析点；
- 新增 `scripts/switch-crash-test.ps1`。辅助进程在 `previous.json` 已原子提交、`active.json` 尚未写入时
  执行 `process::exit(88)`；旧活动指针保持有效，随后正常切换和回滚均通过。报告位于
  `phase4-results\switch-crash.json`，耗时 0.701 秒；
- 新增 `scripts/staging-crash-test.ps1`。已验证 Runtime 的 `runtime.json` 同步完成后、staging 目录原子
  重命名前，辅助进程执行 `process::exit(89)`；旧活动指针不变，未提交版本不会出现在 `versions`，
  下一次同版本安装会安全清理严格命名的残留 staging。报告位于
  `phase4-results\staging-crash.json`，耗时 0.712 秒；
- 下载断流/强杀报告位于 `phase4-results\download-faults.json`，端到端耗时 1.242 秒；备份强杀报告位于
  `phase4-results\backup-crash.json`，端到端耗时 0.665 秒。上述报告目录均被 Git 忽略；
- 故障注入和 DLL 可替换性测试合入默认测试后，全量 Rust 单元测试为 22 通过、3 个需真实环境的显式忽略；Job 强杀集成测试
  仍由专用脚本显式触发。`phase4-results\phase4-check-after-fault-injection.json` 中前端、格式、Rust、
  许可证元数据与 Runtime 清单均通过，许可证义务和 Authenticode 保持预期警告。

### 10. 仍未完成的第四阶段闸门

- Authenticode 方案已确定为 SignPath Foundation；仍需仓库所有者提交申请、接受条款、启用 MFA，并在获批后
  配置 SignPath project/policy/artifact configuration 标识与最小权限 API token；
- 对 NSIS 安装器和内部 EXE 完成 Authenticode 签名，验证签名链、时间戳、升级安装、卸载和数据保留；
- 在无 Node、无 DSH、无开发工具的干净 Windows 10/11 x64 虚拟机执行矩阵测试；
- 对 `@img/sharp-win32-x64` 及其预编译 LGPL 组件完成共享库替换/重链接材料、许可证全文与源码归档复核——
  2026-09-02 已按第二十二节完成人工复核与源码归档生成；剩余“归档随发布/接线 release.yml”为
  所有者决策（见第二十二节.3）；
- 将已完成的非官方身份与自有 Logo 边界落实到最终 Release 页面文案；
- 记录真实 SmartScreen 观测。获得 Authenticode 签名不等于已经建立 SmartScreen 声誉。

### 11. SignPath Foundation 双阶段签名落地

- 2026-09-01 已由项目所有者选择 SignPath Foundation 作为 Windows Authenticode 方案。公开仓库
  `1424801913dd-cmd/dsh-launcher` 已有 MIT 许可证和多个 Windows Release，满足“公开、已有同形态发布”这一
  申请前置事实；最终资格仍由 SignPath Foundation 审核；
- 新增 `docs/CODE_SIGNING_POLICY.md`，并在 README 与 Release 模板使用 SignPath 要求的 “Code signing
  policy” 标题和归属文案；记录 authors/reviewers/approvers、MFA、隐私、trusted build、人工审批和非官方
  身份边界。`docs/SIGNPATH_APPLICATION.md` 汇总申请链接、资格声明、控制台配置和必须由所有者完成的动作；
- `.github/workflows/release.yml` 不再让 `tauri-action` 一步构建并上传。新顺序是：许可证硬门禁 → 生成并
  丢弃一次未签名 NSIS 预打包以固定 EXE bundle 元数据 → 上传 GitHub Actions artifact → SignPath 签 EXE
  → 验证 SignPath Foundation 签名与时间戳 → 用已签 EXE 重新构建 NSIS → 第二次 SignPath 签安装器 →
  安装/卸载/数据保留测试 → 创建 updater ZIP →
  Ed25519 签 updater → 生成 `latest.json` → 创建 GitHub draft Release；
- 之所以必须分两次 SignPath 请求，是因为安装器需要携带已签名内部 EXE；而 Tauri updater 的 ZIP 与签名
  必须在安装器 Authenticode 完成后生成，否则 SignPath 写入 PE 签名会使此前的 updater 摘要失效；
- 本地实测 `tauri bundle` 会报告为 EXE 写入 NSIS bundle 类型信息。流水线因此在 SignPath 前先做一次
  同目标预打包；对已经完成该元数据写入的 EXE 再次 bundle，前后 SHA-256 保持一致。最终安装测试仍会
  验证从安装器释放出的内部 EXE Authenticode，防止未来 Tauri 行为变化静默破坏签名；
- 新增 `scripts/verify-authenticode.ps1`，默认要求签名 Subject 包含 `SignPath Foundation`，可强制可信
  时间戳并输出 SHA-256/证书 JSON；`test-installer.ps1 -RequireAuthenticode` 还会验证真正安装后的内部 EXE；
- 新增 `scripts/prepare-tauri-update-manifest.mjs`，只为最终签名安装器 ZIP 生成 Windows x64 静态 updater
  manifest。`prepare-tauri-release-config.mjs` 改为不在 bundling 时提前生成 updater artifacts；
- `phase4-check.ps1` 增加 SignPath 政策文案门禁，并允许“制品已有有效外部 Authenticode、私钥位于云签名
  服务”作为证书能力通过条件，不再错误要求 GitHub runner 的本机证书库持有私钥；
- 发布 job 只接受 `main`，并以 concurrency group 串行化，避免两个签名审批/同版本 draft 竞争；原先 job
  级注入的 Runtime/Tauri 私钥已收窄到各自唯一签名步骤，npm 安装、编译、SignPath 和安装测试不再继承；
- 2026-09-01 项目所有者已通过 <https://signpath.org/apply> 成功提交 SignPath Foundation 免费订阅申请，
  页面返回 `Form submitted`；当前等待 SignPath Foundation 审核与后续邮件。项目尚未获批，GitHub 仍无
  `SIGNPATH_API_TOKEN` 与五个 SignPath Variables；不得用占位值、个人 PFX 或未签名上传绕过；
- sharp/libvips 的 LGPL 人工复核仍位于任何 SignPath 请求之前，因此即使 SignPath 配置完成，许可证问题
  未解除时流水线也会在上传待签制品前失败关闭。
- 本轮验证：release workflow 用 PyYAML 6.0.1 成功解析为 25 个 steps；三个 PowerShell 脚本与两个 Node
  脚本语法通过；`scripts/check.ps1` 通过（Rust 22 通过、3 忽略、0 失败）；SignPath 文案/文档、品牌、
  Runtime 锁和许可证元数据门禁通过。基线报告为
  `phase4-results/signpath-baseline-final.json`，仅保留 1 个 LGPL 人工复核 WARN 与预期未签名 WARN；
- 本地按发布配置从源码重建后，未签名内部 EXE 为 5,476,352 字节，SHA-256
  `1DE85D15DAD6889C64223751F8D5AB65C589025157509EEB8F425993CB764EC0`；未签名 NSIS 为 2,154,531 字节，
  SHA-256 `5E7BA2FBDBF737A413D0EF144706B8A81CDFC1CACCF77A343E7116CD36EC5130`。两者仍是 `NotSigned`，不得公开；
  `DSH Launcher\` 中的生产 `0.3.3` 安装没有运行、覆盖或删除。

## 二十二、LGPL 人工复核记录（2026-09-02）

本节完善二十一.10 中 sharp/libvips LGPL 项的处置记录。完整备忘录见
[docs/LGPL_REVIEW.md](docs/LGPL_REVIEW.md)，机器可核对清单见
`scripts/data/lgpl-source-manifest.json`。

### 1. 证据链（全部来自本机只读检查与 `.tmp-lgpl-review` 本地缓存）

- 交付链：libvips 8.18.6 + 依赖 → `build-win64-mxe` v8.18.6（build.sh/container/build 配方/
  overrides.mk + MXE `llvm-mingw` 分叉快照 `d973945bb92c7783d5afa41bb2b8d2e1a04eaba3`）→
  `vips-dev-x64-web-8.18.6-static.zip` → `sharp-libvips` v1.3.3 `build/win.sh` →
  `@img/sharp-win32-x64@0.35.4` 的 `lib/`；
- 安装包 `versions.json` 记录 29 个组件精确版本（权威记录；注意 sharp-libvips v1.3.3
  `versions.properties` 的 `VERSION_AOM=3.15.0` 与实际 `aom 3.14.1` 不一致，aom 非 copyleft，仅记录）；
- 8 个 LGPL 类组件源码 tarball 的 SHA-256 与 build-win64-mxe 配方记录**逐一完全一致**（vips/glib/
  pango/fribidi/libexif/librsvg/libheif/proxy-libintl，完整哈希见 manifest）；cairo 1.18.4 校验和
  与 MXE `src/cairo.mk` 一致，已于 2026-09-02 下载并核验通过（MPL 2.0 口径），并入下方归档；
- PE 导入表（`phase4-results/runtime-license-evidence.json`）确认 addon 只导入两个独立
  libvips DLL；单元测试 `installed_runtime_record_does_not_pin_replaceable_native_library_hashes`
  （`src-tauri/src/runtime_manager.rs:1235`）确认安装后不锚定 DLL 哈希。

### 2. 四项检查结论

1. **随附全文与声明**：机制成立（`Build-RuntimeBundle.ps1` 113–128 行放置 GPL-3.0/LGPL-3.0 全文与
   `RUNTIME_LICENSES.md`，139–145 行生成 `THIRD_PARTY_NOTICES.md`）。**注意**：当前活动 Runtime
   `signed-0.1.2-alpha.2` 的 `app/` 中没有这些文件（早于该机制），下一个发布 Bundle 必须由新
   机制重建并以门禁重新证明；上游 npm 包内 `LICENSE` 仅 Apache-2.0 全文；
2. **替换 DLL 不被阻止**：成立（三项独立证据，见上）；
3. **源码/构建脚本/重链接材料**：成立（8 个组件源码校验和匹配 + 三层构建链快照 +
   `vips-dev` 包含 include/ 与导入库使 DLL 可替换/可重链）；残余风险为
   `base.Dockerfile` 以分支而非 commit 固定 MXE（工具链可复现性，源码身份不受影响）；
4. **长期可用地址或随包归档**：**决策点**。推荐“随包归档”。

### 3. 已产出与待办

- 已产出 `scripts/prepare-lgpl-source-materials.ps1` + `scripts/data/lgpl-source-manifest.json`；
  本机已生成归档（含全部 9 个组件源码，报告 `pendingSources: []`）：
  `release-assets/lgpl-source-materials-vips-8.18.6.tar.gz`（79,683,793 字节，SHA-256
  `5A6B85A33DA69292A08401C14279DDC3863EF678FE04FBA3E391F331B6981B1C`，含 9 个已核验源码 +
  3 个构建链快照 + PROVENANCE.md + SHA256SUMS.txt，见
  `release-assets/lgpl-source-materials-vips-8.18.6.tar.gz.json`）；
- 待办（2026-09-02 全部完成）：① 所有者已采纳“归档作为 Release asset”方案，release.yml 已接线：
  在“Build verified Runtime Bundle”**前**新增 “Prepare LGPL source-materials archive” 步骤
  （审计要在运行时内看到完整归档），并把归档与其报告 JSON 加入 draft Release 上传列表
  （`docs/RELEASE_TEMPLATE.md` 也加了引用）；② cairo 已于本机补取并核验（#4 完成）；③ #5 已落地：
  `license-audit.mjs` 把 sharp 处置改为**机械判定**——仅当
  `release-assets/lgpl-source-materials-*.tar.gz.json` 存在、`pendingSources` 为空且归档文件
  SHA-256 与报告一致时记为 `documented-source-availability`，否则回到 `manual-review-required`；
- 验证（2026-09-02，#5 落地后）：审计 `review=6, unresolved=0`；
  `-RequireAuthenticode -RequireInstaller -RequireLicenseReview` 硬闸门仅剩未签名 FAIL，
  `license-obligations-review` 已 PASS（报告 `phase4-results/release-gate-lgpl-review.json`）。
  此前基线（#5 前）见下文“复核后验证”一段；
- 复核后验证（2026-09-02）：`phase4-check.ps1 -RunProjectChecks` 通过（Rust 22 通过、3 忽略、
  0 失败；license-metadata PASS；license-obligations-review 仍为 1 个 WARN），报告
  `phase4-results/lgpl-review-final.json`；`-RequireAuthenticode -RequireInstaller
  -RequireLicenseReview` 硬闸门按预期以退出码 1 拒绝（license-obligations-review FAIL 与
  未签名 FAIL），报告 `phase4-results/release-gate-lgpl-review.json`；
- `docs/LGPL_REVIEW.md` 已加入 `phase4-check.ps1` 发布文档清单。
