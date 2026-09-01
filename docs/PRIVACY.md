# DSH Launcher 隐私说明

> 更新日期：2026-09-01

DSH Launcher 是非官方的本地 Windows 启动器。启动器本身不提供云端账户、遥测或行为分析服务，
也不会读取、上传或代管 DeepSeek Harness（DSH）的 API Key、凭据和完整会话内容。

## 本地处理的数据

启动器仅为安装、启动、更新和停止 DSH 而处理以下本机信息：

- Node.js、DSH、Workspace 与 `DSH_HOME` 的路径和版本；
- 启动器创建的进程 ID、随机 loopback 端口、运行状态与健康检查结果；
- Runtime 更新通道、签名 manifest sequence、下载进度和活动/上一版本指针；
- DSH 子进程的 stdout/stderr，以及启动器自身的状态和错误信息。

生产 `DSH_HOME` 默认位于 `D:\Caches\deepseek-harness\home`，其中可能包含设置、会话、附件、
Workspace 信息和凭据引用。启动器只在用户明确确认应用 Runtime 更新时，将它复制到本机备份目录；
不会读取文件正文来生成日志，也不会把备份上传到网络。

## 本地日志与脱敏

启动器日志默认位于 `D:\Caches\dsh-launcher\logs`；D 盘不可用时回退到当前用户的
`LOCALAPPDATA`。单个日志达到 5 MiB 后轮转，只保留当前日志和一份上一日志。

写入内存日志或持久日志前，启动器会移除 URL 查询参数和片段，并遮盖常见的 token、API Key、
Authorization、Cookie、密码和 secret 字段。启动时也会原子脱敏旧版留下的日志。脱敏是纵深防御，
用户仍不应主动把凭据或完整会话内容粘贴到问题报告中。

## 网络连接

启动器可能进行以下网络连接：

- 访问当前受管 DSH 的随机 `http://127.0.0.1:<port>` 地址进行健康检查和打开 Web UI；
- 从项目配置的 GitHub HTTPS 地址检查、下载并验证签名 Runtime 和启动器更新；
- 首次安装或兼容性安装时，从 `nodejs.org` 获取固定 Node.js ZIP，并从 npm 官方 registry 获取
  精确 DSH 包和完整性元数据。

启动器不会把 loopback token、Cookie 或 DSH 用户数据发送到其他主机或端口。第三方下载服务可能按其
自身政策记录常规的 IP 地址、User-Agent 和请求时间；这不属于启动器自行收集的遥测。

## 卸载与数据保留

卸载启动器不应自动删除独立的 `DSH_HOME`、Runtime 版本或生产备份，避免丢失会话和回滚能力。
如需彻底清理，应在确认不再需要回滚和用户数据后，手动删除对应目录。不要删除仍由 DSH 或启动器进程
使用的 Runtime。

DSH Web UI、模型供应商和用户安装的 DSH 插件是独立组件，可能有各自的数据处理行为和隐私政策；
安装第三方插件前应单独审查。
