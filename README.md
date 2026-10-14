# ShellDock

使用 Rust 实现的桌面 SSH、SFTP 与服务器监控工作台。界面按 FinalShell 4.6.5 的工作区截图调整：全高监控侧栏、右侧会话标签、纯白快速连接列表、深色终端、下方文件树和文件表格，以及悬浮命令输入框。

这是独立实现，保留当前仓库的 Apache-2.0 许可证。没有提交 FinalShell 安装包、反编译代码、原图标或任何真实主机凭据。

## 运行

需要 Rust、Node.js 22+ 和 Tauri 2 的平台依赖。Windows 需要 Microsoft C++ Build Tools 与 WebView2；Linux 需要 GTK3、WebKitGTK 4.1 开发包。

```bash
npm ci
npm run tauri dev
```

```bash
npm run build
cargo test -p shelldock-core
cargo fmt --all --check
npm test
npm run test:e2e
```

`npm run dev` 仅是浏览器界面预览，不会连接 SSH 服务器，不提供虚构的在线监控数据。真实功能必须在 Tauri 桌面环境运行。

界面截图及校准范围见 [工作区界面](docs/ui.md)。

## 已实现

- 主机配置的新建、编辑、删除、分组与搜索。
- SSH 密码、私钥和私钥口令认证；首次主机指纹确认与变更拦截。
- 多标签终端、xterm.js 控制序列显示、PTY 行列缩放、搜索、字号与背景切换。
- Rust/Tokio 会话任务，有界输入队列与终端消费 ACK 背压。
- SFTP 目录树、路径历史、文件表格排序、上传、下载、取消、队列并发限制、新建目录、重命名、删除空目录/文件、权限编辑。文件表格显示 Unix 权限及服务器返回的 UID/GID。
- UTF-8 远端文本编辑，语法高亮、代码折叠、搜索与保存前版本检查。
- Linux CPU、内存、交换、网络速率、磁盘容量与进程采集。CPU / 内存和网卡曲线保存最近 120 次实际采样，磁盘列表包括虚拟挂载点。错误不会显示为数值 0。
- 快捷命令与历史命令；悬浮输入框支持 Enter 发送、Alt 打开历史、Esc 隐藏、双击 Ctrl 显示 / 隐藏。
- 接收 OSC 7 目录事件，驱动文件区切换；也支持手动输入远端路径。
- FinalShell 单连接 JSON / `conn` 目录导入，分组恢复及旧版密码格式兼容。
- ShellDock 无密码 JSON 导入导出，以及可选包含兼容密码的 FinalShell 单连接导出。
- 元数据原子保存，密码使用操作系统凭据服务；服务不可用时支持仅本次运行。

## 兼容范围

详细字段与限制见 [连接兼容](docs/compatibility.md)。导入保留未知非敏感字段，不会把未迁移的代理静默当成直接连接。

工作区主要布局依据用户提供的两张截图；尚未逐像素校准全部对话框。延迟测量未实现，显示“— ms”；所有者显示数字 UID/GID，未解析用户名。监控侧栏和终端 / 文件区可拖动分隔条调整。快速连接的“清空”只清除当前运行中的列表，不删除已保存的连接。

尚未使用用户真实导出文件或真实服务器完成验收；公开非敏感密码向量、格式测试和隔离协议测试不等于所有 FinalShell 版本都兼容。

私钥 ID 引用需要在目标电脑重新选择私钥文件。代理、端口转发、RDP、云同步、ZMODEM、打包传输和加速网络尚未执行，见 [路线图](docs/roadmap.md)。

远端编辑保存的 size/mtime 检查不是原子 compare-and-swap；检查与写入之间仍可能出现竞争。目前仅支持 UTF-8 文本。上传采用新文件独占创建，不会静默覆盖已有远端文件。取消可能留下部分文件。

## 技术结构

```text
src/                         Vue/TypeScript 界面、xterm.js 与 CodeMirror
src-tauri/                   Rust 桌面入口、IPC、凭据与持久化
crates/shelldock-core/        russh、SFTP、指标解析和连接兼容
```

前端负责显示和操作意图；认证、网络、文件和监控逻辑在 Rust。当前使用 Tauri 的 WebView，因此界面并非纯 Rust。

## 自动构建

GitHub Actions 在拉取请求、`main` 更新、`v*` 标签及手动触发时验证 Rust 核心、前端及桌面构建，同时生成 Windows x64 的 NSIS `.exe` 安装包、免安装 `.zip` 与 Linux x64 的 `.deb` 安装包。

在仓库 [Releases](https://github.com/super-potato-lab/shelldock/releases) 直接下载安装包或免安装 ZIP。每个 Release 同时提供 `SHA256SUMS` 校验文件。

Windows 免安装版下载 `ShellDock_<版本>_windows_x64_portable.zip`，完整解压后双击 `ShellDock.exe` 即可。ZIP 包含实际应用程序、使用说明和许可证，构建时会验证解压后的程序能打开窗口。

免安装版需要系统已安装 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)，不捆绑该运行时。Visual C++ 运行时静态链接到程序；配置仍保存在当前用户 AppData，密码使用 Windows 凭据管理器，与安装版共享。

- `main` 构建与测试全部成功后，自动发布 `v<应用版本>-preview.<运行编号>` 预发布版。
- 推送与应用版本一致的标签（例如 `v0.1.0`）后，发布对应版本；版本包含预发布后缀时标记为预发布。
- 拉取请求只验证和构建，不发布 Release。手动运行支持 `main` 或匹配版本的标签。
- 三处版本号（`Cargo.toml`、`package.json`、`src-tauri/tauri.conf.json`）必须一致。
- 安装包全部上传后才公开 Release；重新运行可继续未完成的草稿，不覆盖已发布安装包。

Actions 的 `ShellDock-Windows-x64` / `ShellDock-Linux-x64` 产物仍保留 14 天。Release 附件不受该期限影响。目前未配置代码签名或应用内自动更新。

参考与第三方声明见 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
