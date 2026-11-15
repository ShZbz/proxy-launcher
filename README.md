# 代理启动器 (Proxy Launcher) - Rust 版

小巧极速、零注入、无残留、随删随用的 Windows 桌面网络代理启动辅助工具（Rust + Tauri v2 + Vue 3 高性能重构版本）。

---

## 核心特性

- **极速瞬开 (< 5ms)**：双模式架构——桌面快捷方式通过 `--launch-rule` 走无 WebView 极速直连通道（< 5ms 核心启动，零 GUI 冷启动开销），落地两阶段探活机制（本地 TCP 端口极速放行 + 后台异步代理验证），双击快捷方式真正实现「跟手即开」。
- **轻量独立桌面应用 (~5.02 MB)**：基于 Tauri v2 + Vue 3 + TypeScript + Tailwind CSS v4 构建，前端产物（`dist/`）全量内嵌于 `proxy-launcher.exe`（5,260,800 bytes，同目录需附带 `WebView2Loader.dll` 160,880 bytes）；纯 CLI 入口 `launcher-cli.exe` 仅约 ~528 KB，零 Python 运行时与 `.venv` 依赖。
- **Antigravity 插件 CLI 代理兼容**：当配置端口主机为回环地址（`127.0.0.1` / `localhost` / `::1`）时，`HTTP_PROXY` / `HTTPS_PROXY` 等环境变量自动生成 `http://LOCALHOST:<port>`，避免宿主 CLI（如 `gdrive`、`gmail` 插件后端）将用户代理误判为内部沙箱代理并清除。
- **原生 Win32 快捷方式与拖拽解析**：底层调用 Windows COM `IShellLinkW` 生成桌面 `.lnk`（置 `SW_SHOWMINNOACTIVE` 防闪框），并支持 `.exe` / `.lnk` / `.bat` / `.cmd` 原生拖放与 `GetPath` / PIDL 解析。
- **配置完全兼容**：无缝读取并写入 `config.json`，支持同名 `.tmp` 原子覆写与损坏自动恢复。

---

## 工程结构

```
proxy-launcher/
├── old project/                 # 已归档的原 Python + Tkinter 历史版本
└── proxy-launcher-rust/         # 当前 Rust + Tauri v2 + Vue 3 主工程
    ├── benchmark.ps1            # 30 轮启动耗时实测基准脚本
    ├── CHANGELOG.md             # 详细更新日志与演进记录
    ├── 使用说明.md               # 业务逻辑与界面功能使用指南
    ├── config.json              # 代理端口与启动规则配置文件
    ├── picture/                 # 应用程序图标与素材 (app_icon.ico / png)
    ├── src/                     # Vue 3 + TypeScript + Tailwind CSS v4 桌面前端
    │   ├── App.vue              # 主工作区、全局快捷键与模态窗调度
    │   ├── api.ts               # Tauri v2 IPC 与原生文件拖放封装
    │   ├── types.ts             # 数据模型与深浅色主题 Token
    │   └── components/          # PortSidebar / PortModal / TerminalModal / RuleWorkspace / RuleModal
    ├── src-tauri/               # Tauri v2 桌面宿主 + IPC 命令 + Headless 极速直连通道
    ├── crates/
    │   ├── launcher-core/       # 纯逻辑核心库 (Config, Lnk, Launcher, Probe)
    │   └── launcher-cli/        # 命令行二进制入口 (launcher-cli.exe)
    ├── target/release/          # Release 便携交付目录 (proxy-launcher.exe / WebView2Loader.dll / launcher-cli.exe / config.json)
    └── tests/e2e/               # 端到端集成测试套件 (74 项 E2E 测试)
```

> **便携运行提示**：直接运行 `target/release/proxy-launcher.exe` 即可打开桌面管理界面；如需复制到其他目录独立使用，请一并携带同目录下的 `WebView2Loader.dll` 与 `config.json`。

---

## 常用命令

```powershell
# 构建前端生产资源 (Vue 3 + TypeScript + Tailwind CSS -> dist/)
npm run build

# 编译独立桌面版 Release 二进制 (target/release/proxy-launcher.exe，内嵌 dist/ 生产资源)
cargo build --release -p proxy-launcher --features custom-protocol

# 运行核心库、CLI 与 E2E 自动化测试套件 (110 项全绿)
cargo test -p launcher-core -p launcher-cli -p e2e-tests

# 运行 30 轮启动耗时性能实测
.\benchmark.ps1

# 直接启动指定规则应用 (无 GUI 极速带代理启动)
.\target\release\proxy-launcher.exe --launch-rule <rule_id>
# 或使用纯 CLI 入口:
.\target\release\launcher-cli.exe --launch-rule <rule_id>

# 生成原生桌面快捷方式
.\target\release\proxy-launcher.exe --create-shortcut <rule_id>
```

---

## 详细文档索引
- [使用说明.md](使用说明.md)：原启动器设计理念、端口管理、终端注入与操作指南；
- [CHANGELOG.md](CHANGELOG.md)：版本历史与架构演进记录。
