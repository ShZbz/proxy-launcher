# 更新日志 (CHANGELOG)

所有重要的架构调整、新功能特性与体验缺陷修复均记录于此。

---

## [v0.2.2] - 2026-11-15

### 🐛 代理兼容修复与工程文档同步
* **修复 Antigravity `plugin-cli`（`gdrive` / `gmail` 等插件 CLI）回环代理被剥离问题**：
  * `language_server.exe` 在 `plugincli.ignoreCommandProxy` 中会识别 `HTTPS_PROXY` 为回环 IP（`127.0.0.1` / `::1`）或精确小写 `"localhost"` 的情况，误判为内部命令沙箱代理并将其清除，导致外呼请求直连超时；
  * 在 [launcher.rs](crates/launcher-core/src/launcher.rs) 的 `build_proxy_env` 中，当配置代理 `host` 为回环地址（`127.0.0.1`、`::1`、`[::1]` 或不区分大小写的 `localhost`）时，将 `HTTP_PROXY`、`HTTPS_PROXY`、`http_proxy`、`https_proxy` 统一生成为 `http://LOCALHOST:<port>`（非回环主机、`ALL_PROXY`/`all_proxy` 及 `--proxy-server` 参数保持原样），在全平台可正常解析至 `127.0.0.1` 的同时绕过宿主侧剥离逻辑。
* **构建、测试与分发规范完善**：
  * 明确独立桌面版构建须启用 `--features custom-protocol`（当前 `proxy-launcher.exe` 体积为 `5,260,800` bytes），确保 `dist/` 前端静态资源正确内嵌；
  * 更新 `launcher-core` 与 `e2e-tests` 校验（含 Windows 大小写环境变量键行为适配），`cargo test -p launcher-core -p launcher-cli -p e2e-tests` 全部 `110` 项测试通过；
  * 同步脱敏脚本与 `.gitignore` 规则，清理编译期临时中间产物。

---

## [v0.2.1] - 2026-11-15

### 🚀 Phase 2：Tauri v2 + Vue 3 + Tailwind CSS 独立桌面版发布与旧版归档
* **双模式统一架构落地**：
  * 桌面可执行文件 `proxy-launcher.exe`（5,259,776 bytes，~5.02 MB）集成了完整 WebView2 图形界面与前端静态资源（通过 `custom-protocol` 特性内嵌 `dist/index.html`、`dist/assets/*` 与 `app_icon_64.png`）；
  * 当携带 `--launch-rule` 或 `--create-shortcut` 参数启动时，在 WebView 初始化前直接进入 Rust Headless 极速直连通道，兼顾图形管理与零冷启动延迟。
* **Vue 3 + TypeScript + Tailwind CSS v4 现代化桌面界面**：
  * **左侧端口面板 (`PortSidebar` / `PortModal` / `TerminalModal`)**：品牌顶栏、深浅色主题无缝切换、端口卡片筛选高亮、呼吸脉冲探活指示、状态/延迟/出口 IP 展示、一键打开或复制 PowerShell / CMD / Bash 代理环境变量注入终端。
  * **右侧规则工作区 (`RuleWorkspace` / `RuleModal`)**：原生文件拖放解析（`.exe` / `.lnk` / `.bat` / `.cmd`）、实时搜索 (`Ctrl+F`)、端口筛选芯片、行内 `PortBadge` 交互式快切端口与闪烁脉冲反馈、右键上下文菜单、批量改绑/启动/生成桌面直连快捷方式、规则编辑模态窗（含 UAC 提权提示与一键建快捷方式）。
* **工程整理与归档**：
  * 原 Python 重构前项目文件（`core/`、`ui/`、`tests/`、`.venv/` 等）整体归档至仓库上层 `old project/` 目录，并保留 `config.json` 配置同步至 `proxy-launcher-rust/` 与 `target/release/`；
  * 清理 Rust 编译临时中间件（`debug/`、增量编译与 `.rlib`/`.d` 等），`target/release/` 仅保留最终可执行交付件。
* **质量验证**：
  * `npm run build` (`vue-tsc --noEmit && vite build`) 零错误通过；
  * `cargo test -p launcher-core -p launcher-cli -p e2e-tests` 全部 110 项测试通过（`launcher-core` 14 + `launcher-cli` 22 + `e2e-tests` 74）。

---

## [v0.2.0] - 2026-11-14

### ⚡ Phase 1：Rust 极速 CLI 内核先行发布
* **`launcher-core` 纯逻辑核心库**：
  * 实现与旧版 `config.json` 100% 兼容的强类型配置读写与 `.tmp` 原子覆写 / `.bad` 损坏回退；
  * 落地两阶段探活优化（本地 TCP 端口 `< 1ms` 极速放行 + 后台异步上游代理健康检查）；
  * 基于 Win32 COM `IShellLinkW` 生成桌面 `.lnk` 快捷方式（自动提取目标图标并置 `SW_SHOWMINNOACTIVE` 防黑框）；
  * 支持 `DETACHED_PROCESS` 脱钩派生进程与代理环境变量（`HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY` / `NO_PROXY`）精准注入。
* **`launcher-cli` 独立命令行入口**：
  * 编译生成单一无依赖二进制 `launcher-cli.exe`（~528 KB），支持 `--launch-rule <id>` 与 `--create-shortcut <id>`，实现毫秒级直连启动；
  * 核心库与 CLI 自动化测试共 36 项全部通过。

---

## [v0.1.2] - 2026-11-14

### ✨ 新特性与交互增强 (Features & UX)
* **按代理端口分类筛选规则**：
  * 默认显示全部规则；单击左侧任意端口卡片可高亮选中该端口（2px 强调边框与高亮底色），右侧规则列表随即仅展示绑定该端口的规则并显示数量统计（如 `2/3`）；
  * 再次单击同一端口卡片、点击左侧 `Ports` 标题栏的「全部」按钮或右侧规则栏的筛选芯片（`● 端口: ... ✕`）即可恢复显示全部规则；删除当前筛选端口时自动复位。
* **规则快捷切换代理端口**：
  * 规则行的端口展示升级为可交互胶囊 `PortBadge`（如 `● clash ▾`）：**左键单击**弹出屏幕边缘自适应浮层菜单 `open_port_quick_menu()` 直接改绑端口（附带实时探活状态圆点），**右键单击**直接轮切至下一个可用端口；
  * 右键单击规则行新增快捷上下文菜单，支持一键启动、切换代理端口、切换注入方式、生成桌面直连图标、编辑与删除；
  * 底部批量操作栏新增「`⇄ 切换端口 ▾`」按钮，支持对已勾选的多条规则一键批量改绑代理端口。

### 🎨 界面审美与动画优化 (UI & Animations)
* 统一扩充 `ui/theme.py` 深浅双色 Token（卡片选中态、端口胶囊悬停态、浮层菜单、筛选芯片）并同步支持主题切换实时重绘；
* 规则切换端口时新增边框脉冲提示动画（`PORT_SWITCH_FLASH_FRAMES`），并将规则行原先不易辨识的 `✎` 编辑按钮替换为清晰的「编辑」按钮、端口卡片「测试」按钮升级为胶囊风格、`RuleDialog` 注入方式选项升级为带说明的宽按钮。

### 🐛 Bug 修复 (Bug Fixes)
* **彻底修复 `RuleDialog` 中 `PortCombo` 下拉选框点击穿透问题**：
  * 为下拉弹窗补充模态 grab 保存/恢复（`_prev_grab` / `top.grab_set()`）、`<ButtonPress-1>` 与 `<ButtonRelease-1>` 分离拦截及 45ms 延迟关闭，并将 `<FocusOut>` 改为延迟跨窗体焦点校验；
  * 结合 `is_open()` 与 `is_recently_closed(0.22)` 防护窗口，彻底阻止选择端口时穿透触发下方「注入方式」按钮。

---

## [v0.1.1] - 2026-11-09

### ⚡ 性能与流畅度优化 (Performance)
* **冷启动加速 45%**：移除了初始化流程中先于数据装载调用的空排版 `update()`，合并渲染时机，启动构建耗时从 2.24 秒缩减至 1.23 秒。
* **窗口拖动跟手流畅**：优化 `DropZone` 虚线框的 `<Configure>` 监听机制，增加尺寸比对与防抖节流，拖动/位移窗口时跳过耗时的 Canvas 重绘；单步耗时降至 5.0ms（相当于 200 FPS），彻底根除拖拽卡滞。
* **统一几何管理**：根窗口子面板全面规范为 `grid(row=..., column=...)` 统一管理，消除了 `pack` 与 `grid` 混用引发的反复几何震颤。

### 🎨 界面与用户体验修复 (Bug Fixes & UX)
* **修复深色/浅色切换底色脱节与卡片缝隙横条**：
  * 解决 `CTkScrollableFrame` 双层容器底色同步缺陷：不仅更新外层视口画布 `_parent_canvas`，同时在主题切换时显式更新内层容器 `tkinter.Frame` 背景色；
  * 彻底消除了模式切换后卡片间隙与列表下方的黑白孤岛横条，实现 100% 平滑贴合。
* **解决左上角标题文字截断遮挡**：
  * 侧边栏宽度 `SIDEBAR_W` 从 320px 拓宽至 340px，赋予更多横向展示空间；
  * 标题字号微调至 14pt bold 并精简 `>_ 终端` 与主题切换按钮尺寸，留给标题 176px 以上可用宽度，彻底杜绝文字被按钮挤压遮挡。
* **消除全屏与窗口缩放时的黑块闪烁**：
  * 将 `PortPanel`、`RulePanel` 内部的多个原生 `tkinter.Frame` 统一替换为 CustomTkinter 的 `w.Frame`，消除子窗口 HWND 与 Windows DWM 异步擦除的机制冲突；
  * 窗口根层统一注入主题底色保底，并规范了 Per-Monitor DPI 设置。

---

## [v0.1.0] - 2026-11-09

### ✨ 新特性与功能增强
* **自定义圆角应用图标**：提取透明圆角应用图标（`picture/app_icon.ico` / `app_icon_64.png`），自动绑定任务栏与窗口标题栏。
* **桌面直连代理快捷方式 (`↗`)**：
  * 允许为任何规则在桌面一键生成 `<软件名> (代理).lnk` 快捷方式；
  * 双击该桌面图标可直接调用 `--launch-rule` 在后台极速完成探活与代理注入启动，无需启动主启动器图形界面。
* **一键代理终端 (`>_`)**：
  * 顶部与端口卡片均支持一键打开预注入代理环境变量（PowerShell / CMD / Bash）的终端窗口；
  * 支持右键弹出并一键复制注入脚本代码。
* **快速探活与延迟检测**：
  * 优化端口探测探针超时与回退重试策略；
  * 增加呼吸脉冲动画与状态胶囊提示。

---

## [v0.0.0] - 2026-10-04

### 🚀 初始版本
* 基础端口管理与连通性探活；
* 启动规则管理（支持 `env` / `flag` / `both` 三种注入模式）；
* 快捷方式（`.lnk`）与 `.exe` 拖拽自动解析；
* 本地纯净运行，零注册表与系统残留。
