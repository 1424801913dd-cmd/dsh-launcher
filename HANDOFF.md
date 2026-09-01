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

           [官方 DSH 鲸鱼标志]
           ● 正在运行
           DSH 0.1.1-rc.2 · Node 24.19.0

       [停止 DSH]      [打开 DSH 界面]

更新
推荐版本 0.1.1-rc.2                    [检查更新]

工作区  D:\Bian_CHENG\dsmax
日志                                                ▾
```

界面采用白色主背景、官方蓝色强调和黑色正文，优先保证高对比度与可读性，并保留键盘导航、
清晰的进度状态与错误详情。
Logo 应使用官方仓库 `apps/web/public/favicon.svg` 中的 DSH 鲸鱼标志，不自行临摹。

若项目公开发布，产品名建议使用 `DSH Launcher`，并在关于页注明“非官方社区启动器，兼容
DeepSeek Harness”。官方品牌规范允许准确描述兼容关系，但建议生态项目使用 `DSH` 缩写，避免
把完整的 `DeepSeek Harness` 商标直接作为第三方产品名，也不得造成官方背书、合作或授权的误解。

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
- DSH 品牌规范：<https://github.com/deepseek-ai/deepseek-harness/blob/master/BRAND_GUIDELINES.zh.md>
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
- [ ] 启动器强制退出后，受管 DSH 的 Job 进程树能全部终止；
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
