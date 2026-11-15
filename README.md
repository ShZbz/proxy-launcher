# 代理启动器 (Proxy Launcher) - Rust 版

小巧极速、零注入、无残留、随删随用的 Windows 桌面网络代理启动辅助工具（Rust + Tauri v2 + Vue 3 高性能重构版本）。

---

## 快速下载与使用（普通用户）

无需安装 Rust、Node.js 或任何编译环境，直接下载便携压缩包解压即可运行：

1. 前往本仓库的 **[GitHub Releases](https://github.com/ShZbz/proxy-launcher/releases)** 页面，在最新版本中下载 `proxy-launcher-<version>-windows-x64.zip`（例如 `proxy-launcher-v0.2.2-windows-x64.zip`）。
2. 将压缩包完整解压到任意目录（建议固定文件夹，例如 `D:\Tools\proxy-launcher\`），解压后目录包含以下文件：
   - `proxy-launcher.exe`：独立桌面主程序（双击即可打开图形界面；携带 `--launch-rule` 时走无 WebView 极速直连通道）
   - `WebView2Loader.dll`：WebView2 运行时加载器（须与 `proxy-launcher.exe` 位于同一目录）
   - `launcher-cli.exe`：纯命令行极速启动入口
   - `config.json`：初始空白配置文件（由 `config.example.json` 模板生成，后续在界面中的修改会自动保存至此文件）
   - `使用说明.md`：功能与快捷键详细说明
3. 双击 `proxy-launcher.exe` 打开主界面：
   - **配置代理端口**：在左侧栏添加本地代理端口（如 Clash / Mihomo / v2rayN 的 `127.0.0.1:7890`），点击「测试」验证连通性；
   - **添加启动规则**：将桌面快捷方式（`.lnk`）或 `.exe` 直接拖入右侧虚线框，或点击手动添加；
   - **一键带代理启动 / 生成桌面直连图标**：点击「▶ 启动」立即带代理启动目标程序，或点击「↗」在桌面生成 `<软件名> (代理).lnk` 直连快捷方式（以后双击桌面图标无需打开主界面即可秒开）。

> **运行环境要求**：Windows 10 / Windows 11 (x64)，依赖系统内置的 Microsoft Edge WebView2 Runtime（如极精简版系统缺失，可前往微软官网下载安装 Evergreen WebView2 Runtime）。

---

## 核心特性

- **极速瞬开 (< 5ms)**：双模式架构——桌面快捷方式通过 `--launch-rule` 走无 WebView 极速直连通道（< 5ms 核心启动，零 GUI 冷启动开销），落地两阶段探活机制（本地 TCP 端口极速放行 + 后台异步代理验证），双击快捷方式真正实现「跟手即开」。
- **轻量独立桌面应用 (~5 MB)**：基于 Tauri v2 + Vue 3 + TypeScript + Tailwind CSS v4 构建，前端产物（`dist/`）全量内嵌于 `proxy-launcher.exe`（同目录需附带 `WebView2Loader.dll`）；纯 CLI 入口 `launcher-cli.exe` 仅约 ~500 KB，零 Python 运行时依赖。
- **Antigravity 插件 CLI 代理兼容**：当配置端口主机为回环地址（`127.0.0.1` / `localhost` / `::1`）时，`HTTP_PROXY` / `HTTPS_PROXY` 等环境变量自动生成 `http://LOCALHOST:<port>`，避免宿主 CLI（如 `gdrive`、`gmail` 插件后端）将用户代理误判为内部沙箱代理并清除。
- **原生 Win32 快捷方式与拖拽解析**：底层调用 Windows COM `IShellLinkW` 生成桌面 `.lnk`（置 `SW_SHOWMINNOACTIVE` 防闪框），并支持 `.exe` / `.lnk` / `.bat` / `.cmd` 原生拖放与 `GetPath` / PIDL 解析。
- **配置完全兼容**：无缝读取并写入 `config.json`，支持同名 `.tmp` 原子覆写与损坏自动恢复。

---

## 工程结构

```text
proxy-launcher/
├── .github/workflows/release.yml  # GitHub Actions Windows 自动测试、编译与 Release 打包工作流
├── benchmark.ps1                  # 30 轮启动耗时实测基准脚本
├── CHANGELOG.md                   # 详细更新日志与演进记录
├── 使用说明.md                    # 业务逻辑与界面功能使用指南
├── config.example.json            # 空配置模板（本地运行可复制为 config.json）
├── picture/                       # 应用程序图标与素材 (app_icon.ico / png)
├── src/                           # Vue 3 + TypeScript + Tailwind CSS v4 桌面前端
│   ├── App.vue                    # 主工作区、全局快捷键与模态窗调度
│   ├── api.ts                     # Tauri v2 IPC 与原生文件拖放封装
│   ├── types.ts                   # 数据模型与深浅色主题 Token
│   └── components/                # PortSidebar / PortModal / TerminalModal / RuleWorkspace / RuleModal
├── src-tauri/                     # Tauri v2 桌面宿主 + IPC 命令 + Headless 极速直连通道
├── crates/
│   ├── launcher-core/             # 纯逻辑核心库 (Config, Lnk, Launcher, Probe)
│   └── launcher-cli/              # 命令行二进制入口 (launcher-cli.exe)
└── tests/e2e/                     # 端到端集成测试套件 (74 项 E2E 测试)
```

---

## 从源码构建（开发者）

> 前置依赖：Windows 10/11 (x64)、Node.js (v20+)、Rust 稳定版工具链（MSVC 或 GNU）。

```powershell
# 1. 安装前端依赖并构建生产资源 (Vue 3 + TypeScript + Tailwind CSS -> dist/)
npm ci
npm run build

# 2. 运行核心库、CLI 与 E2E 自动化测试套件
cargo test -p launcher-core -p launcher-cli -p e2e-tests

# 3. 编译独立桌面版 Release 二进制 (target/release/proxy-launcher.exe，内嵌 dist/ 生产资源)
cargo build --release -p proxy-launcher --features custom-protocol

# 4. 编译纯命令行入口 (target/release/launcher-cli.exe)
cargo build --release -p launcher-cli

# 5. （可选）复制空配置模板用于本地运行测试
Copy-Item config.example.json target/release/config.json

# 6. 运行 30 轮启动耗时性能实测
.\benchmark.ps1

# 7. CLI 直接启动指定规则应用 (无 GUI 极速带代理启动)
.\target\release\proxy-launcher.exe --launch-rule <rule_id>
# 或使用纯 CLI 入口:
.\target\release\launcher-cli.exe --launch-rule <rule_id>

# 8. 生成原生桌面快捷方式
.\target\release\proxy-launcher.exe --create-shortcut <rule_id>
```

> **便携运行提示**：构建完成后，如需将 `target/release/proxy-launcher.exe` 复制到其他目录独立使用，请一并携带同目录下的 `WebView2Loader.dll` 与 `config.json`（可通过 `config.example.json` 复制生成）。

---

## 详细文档索引

- [使用说明.md](使用说明.md)：启动器设计理念、端口管理、终端注入与操作指南；
- [CHANGELOG.md](CHANGELOG.md)：版本历史与架构演进记录。
