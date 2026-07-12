# Codex 与 CC Switch 一键安装器 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 构建一个面向普通用户的 Windows 10/11 与 macOS 一键安装向导，自动检测环境、安装 Codex 桌面应用和 CC Switch、创建入口、验证结果并支持失败恢复。

**Architecture:** 使用 Tauri 2 提供跨平台桌面壳，前端使用 Vite + 原生 TypeScript，后端使用 Rust。安装流程由 `InstallCoordinator` 驱动，下载、平台能力、应用适配器、验证和诊断分别隔离，通过事件流向 UI 报告进度。

**Tech Stack:** Tauri 2、Rust、Tokio、Reqwest、Serde、SHA-2、Vite、TypeScript、Vitest、Windows NSIS、macOS DMG/PKG。

## Global Constraints

- 首版支持 Windows 10/11 和 macOS。
- 目标产品是 Codex 桌面应用和 CC Switch。
- 自动检测环境、安装两个程序、创建快捷方式并启动；账号登录由用户自己完成。
- 不保存账号、密码、Token 或聊天内容。
- 已安装较新版本时不降级、不无提示覆盖。
- 下载完成后必须校验 SHA-256，校验失败不得执行安装。
- 下载失败最多自动重试 3 次，并支持中断恢复。
- Windows 需要管理员权限时使用 UAC；macOS 优先安装到当前用户的 `~/Applications`。
- 失败提示必须包含软件、步骤、原因和下一步建议。
- 第一版不实现自动登录、Token 管理、复杂配置迁移、后台静默升级、代理/VPN、软件商店和远程诊断采集。
- 当前目录不是 Git 仓库；开始实现前应先由维护者决定是否初始化 Git，未初始化前不执行提交命令。

---

## 文件结构

```text
package.json                       前端脚本和依赖
vite.config.ts                     Vite 配置
index.html                         Tauri 前端入口
src/main.ts                        前端启动入口
src/app.ts                         页面状态机和渲染协调
src/api.ts                         Tauri command 调用封装
src/styles.css                     安装向导样式
src/ui/types.ts                    前端事件和页面类型
src/ui/copy.ts                     面向小白的固定文案
src/ui/render.ts                   页面渲染函数
src/ui/render.test.ts              页面状态渲染测试
src-tauri/Cargo.toml               Rust 依赖
src-tauri/tauri.conf.json          Tauri 打包配置
src-tauri/src/main.rs              Tauri 启动入口
src-tauri/src/lib.rs                command 注册和服务组装
src-tauri/src/commands.rs          前端可调用 command
src-tauri/src/domain.rs             核心类型和错误类型
src-tauri/src/manifest.rs           版本清单解析和架构选择
src-tauri/src/download.rs           下载、续传、重试和 SHA-256
src-tauri/src/coordinator.rs        安装流程状态机
src-tauri/src/diagnostics.rs        日志、脱敏和诊断导出
src-tauri/src/platform/mod.rs       平台能力接口
src-tauri/src/platform/windows.rs   Windows 检测、UAC 和快捷方式
src-tauri/src/platform/macos.rs     macOS 检测、应用复制和入口创建
src-tauri/src/adapters/mod.rs       应用适配器接口
src-tauri/src/adapters/codex.rs     Codex 安装适配器
src-tauri/src/adapters/cc_switch.rs CC Switch 安装适配器
src-tauri/tests/coordinator.rs      流程集成测试
assets/manifest.json                内置备用版本清单
docs/testing-matrix.md              Windows/macOS 手工验收矩阵
```

## Task 1: 建立 Tauri 项目骨架和测试入口

**Files:**
- Create: `package.json`
- Create: `vite.config.ts`
- Create: `index.html`
- Create: `src/main.ts`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/lib.rs`

**Interfaces:**
- Produces `cargo test`、`npm run test` 和 `npm run tauri dev` 三个可执行入口。
- Produces a Tauri application with an empty window and no installation side effects.

- [ ] **Step 1: 创建 package scripts 和最小前端入口**

在 `package.json` 中提供 `dev`、`build`、`test` 和 `tauri` scripts；`src/main.ts` 只挂载一个根节点并显示“安装器准备就绪”。

- [ ] **Step 2: 创建 Tauri Rust crate 和健康检查测试**

在 `src-tauri/src/lib.rs` 中导出 `pub fn app_name() -> &'static str { "Codex 与 CC Switch 一键安装器" }`，并添加断言该函数返回固定名称的单元测试。

- [ ] **Step 3: 运行骨架测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: 1 个单元测试 PASS。

Run: `npm run build`

Expected: Vite 构建成功并生成 `dist`。

- [ ] **Step 4: 确认 Tauri 空窗口启动**

Run: `npm run tauri dev`

Expected: 显示安装器窗口，不下载、不请求管理员权限、不创建快捷方式。

- [ ] **Step 5: Commit**

```text
git add package.json vite.config.ts index.html src src-tauri
git commit -m "chore: scaffold cross-platform installer"
```

## Task 2: 定义版本清单、目标平台和领域错误

**Files:**
- Create: `src-tauri/src/domain.rs`
- Create: `src-tauri/src/manifest.rs`
- Create: `assets/manifest.json`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/manifest.rs`

**Interfaces:**
- `AppId::{Codex, CcSwitch}`
- `PlatformTarget { os: Os, arch: Arch }`
- `AppRelease { version, url, size_bytes, sha256, installer_kind }`
- `Manifest::select(app_id: AppId, target: PlatformTarget) -> Result<AppRelease, InstallError>`
- `Manifest::from_json(input: &str) -> Result<Manifest, InstallError>`

- [ ] **Step 1: 写清单解析失败测试**

测试缺少 `sha256`、不支持的架构和空下载地址时分别返回结构化错误，而不是 panic。

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml manifest`

Expected: 测试因 `Manifest` 和 `InstallError` 尚未实现而 FAIL。

- [ ] **Step 3: 实现领域类型和清单解析**

使用 Serde 反序列化 JSON；`sha256` 必须为 64 位十六进制字符串；`size_bytes` 必须大于 0；URL 必须是 HTTPS。

- [ ] **Step 4: 添加 Windows x64、macOS x64 和 macOS arm64 的清单结构**

`assets/manifest.json` 使用真实发行包信息前可以先放入测试专用的本地 HTTPS 地址和固定 SHA-256；正式开发前必须替换为经过核验的官方包地址。

- [ ] **Step 5: 运行领域测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml manifest`

Expected: 所有清单解析、平台选择和校验字段测试 PASS。

- [ ] **Step 6: Commit**

```text
git add src-tauri/src/domain.rs src-tauri/src/manifest.rs assets/manifest.json src-tauri/src/lib.rs
git commit -m "feat: define release manifest and installer domain types"
```

## Task 3: 实现环境检测和平台能力接口

**Files:**
- Create: `src-tauri/src/platform/mod.rs`
- Create: `src-tauri/src/platform/windows.rs`
- Create: `src-tauri/src/platform/macos.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/platform/mod.rs`

**Interfaces:**
- `trait Platform: Send + Sync`
- `Platform::target() -> PlatformTarget`
- `Platform::free_space(path: &Path) -> Result<u64, InstallError>`
- `Platform::has_install_permission() -> Result<bool, InstallError>`
- `Platform::find_app(app_id: AppId) -> Result<Option<InstalledApp>, InstallError>`
- `Platform::create_entry(app_id: AppId, executable: &Path) -> Result<(), InstallError>`
- `Platform::launch(executable: &Path) -> Result<(), InstallError>`

- [ ] **Step 1: 添加平台检测测试**

使用 `FakePlatform` 测试 x64、arm64 和未知架构映射；验证低于最低磁盘空间时返回 `InsufficientDiskSpace`。

- [ ] **Step 2: 运行平台测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml platform`

Expected: 测试因平台接口尚未定义而 FAIL。

- [ ] **Step 3: 实现 Windows 平台模块**

读取 Windows 版本和架构；通过注册表、已知安装目录和开始菜单入口查找应用；创建开始菜单/桌面入口；启动时使用 `std::process::Command`，需要提权时交给 NSIS/UAC。

- [ ] **Step 4: 实现 macOS 平台模块**

检查 `~/Applications`、`/Applications` 和应用包内的 `Contents/Info.plist`；将应用复制到 `~/Applications`；使用 `open` 启动；入口创建遵循 macOS 应用目录习惯，不生成多余桌面文件。

- [ ] **Step 5: 运行平台单元测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml platform`

Expected: FakePlatform 测试全部 PASS；真实平台测试只在对应系统执行。

- [ ] **Step 6: Commit**

```text
git add src-tauri/src/platform src-tauri/src/lib.rs
git commit -m "feat: add cross-platform environment detection"
```

## Task 4: 实现下载、断点续传、重试和 SHA-256 校验

**Files:**
- Create: `src-tauri/src/download.rs`
- Modify: `src-tauri/src/domain.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/download.rs`

**Interfaces:**
- `DownloadManager::fetch(release: &AppRelease, destination: &Path, sink: &dyn ProgressSink) -> Result<PathBuf, InstallError>`
- `ProgressSink::on_progress(app_id: AppId, downloaded: u64, total: u64)`
- `sha256_file(path: &Path) -> Result<String, InstallError>`

- [ ] **Step 1: 写下载测试**

覆盖完整下载、已有同名临时文件时续传、HTTP 断开后最多重试 3 次、SHA-256 不匹配删除文件四种情况；测试使用本地 mock HTTP 服务，不访问真实软件地址。

- [ ] **Step 2: 运行下载测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml download`

Expected: 测试因 `DownloadManager` 尚未实现而 FAIL。

- [ ] **Step 3: 实现临时文件和续传逻辑**

下载到 `<destination>.part`；服务器支持 Range 时从文件长度处继续；服务器不支持 Range 时删除 `.part` 后重新下载；每次重试记录原因。

- [ ] **Step 4: 实现大小和哈希校验**

下载完成后先校验 `size_bytes`，再使用 SHA-256 校验；任何校验失败都删除 `.part` 和最终文件，并返回 `ChecksumMismatch` 或 `SizeMismatch`。

- [ ] **Step 5: 运行测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml download`

Expected: 所有下载、续传、重试和哈希测试 PASS。

- [ ] **Step 6: Commit**

```text
git add src-tauri/src/download.rs src-tauri/src/domain.rs src-tauri/Cargo.toml
git commit -m "feat: add resumable verified downloads"
```

## Task 5: 实现 Codex 和 CC Switch 安装适配器

**Files:**
- Create: `src-tauri/src/adapters/mod.rs`
- Create: `src-tauri/src/adapters/codex.rs`
- Create: `src-tauri/src/adapters/cc_switch.rs`
- Modify: `src-tauri/src/platform/mod.rs`
- Test: `src-tauri/src/adapters/mod.rs`

**Interfaces:**
- `trait AppAdapter`
- `AppAdapter::id() -> AppId`
- `AppAdapter::detect() -> Result<Option<InstalledApp>, InstallError>`
- `AppAdapter::install(package: &Path) -> Result<InstallResult, InstallError>`
- `AppAdapter::verify() -> Result<VerificationResult, InstallError>`
- `AppAdapter::launch() -> Result<(), InstallError>`

- [ ] **Step 1: 写 FakeAdapter 流程测试**

验证适配器能区分“未安装”“已安装可用”“已安装但版本过旧”“已安装版本更新”四种状态，并验证新版本不会覆盖更高版本。

- [ ] **Step 2: 运行适配器测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml adapters`

Expected: 测试因 `AppAdapter` 尚未实现而 FAIL。

- [ ] **Step 3: 定义统一适配器接口和退出码映射**

将 Windows 安装程序退出码、macOS 复制失败、用户取消权限分别映射到 `InstallError::InstallerFailed`、`PermissionDenied` 和 `UserCancelled`。

- [ ] **Step 4: 实现 Codex 适配器**

按清单中的 `installer_kind` 执行对应安装包；安装完成后通过平台模块查找应用版本，验证应用路径和启动行为。

- [ ] **Step 5: 实现 CC Switch 适配器**

复用相同接口，但独立维护 CC Switch 的安装路径、版本读取规则和启动入口，不在 Codex 适配器中加入分支。

- [ ] **Step 6: 运行适配器测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml adapters`

Expected: FakeAdapter、版本比较和错误映射测试 PASS。

- [ ] **Step 7: Commit**

```text
git add src-tauri/src/adapters src-tauri/src/platform/mod.rs
git commit -m "feat: add Codex and CC Switch adapters"
```

## Task 6: 实现安装流程状态机和恢复机制

**Files:**
- Create: `src-tauri/src/coordinator.rs`
- Modify: `src-tauri/src/domain.rs`
- Test: `src-tauri/src/coordinator.rs`
- Test: `src-tauri/tests/coordinator.rs`

**Interfaces:**
- `InstallPhase::{Checking, Planning, Downloading, VerifyingPackage, Installing, VerifyingInstall, Completed, Failed, Cancelled}`
- `InstallEvent { app_id: Option<AppId>, phase: InstallPhase, progress: Option<Progress>, message_key: String }`
- `InstallCoordinator::plan() -> Result<InstallPlan, InstallError>`
- `InstallCoordinator::run(plan: InstallPlan, sink: &dyn EventSink) -> Result<InstallSummary, InstallError>`
- `InstallCoordinator::resume() -> Option<InstallCheckpoint>`

- [ ] **Step 1: 写状态转移测试**

验证流程顺序为检测、规划、逐个下载/校验/安装/验证、创建入口、完成；验证取消、失败重试和跳过时不会执行后续不允许的阶段。

- [ ] **Step 2: 写恢复测试**

在 Codex 安装失败后保存 checkpoint；重新创建 coordinator 时读取 checkpoint，并从 Codex 失败阶段重试，不重复执行已经验证成功的步骤。

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml coordinator`

Expected: 测试因状态机和 checkpoint 尚未实现而 FAIL。

- [ ] **Step 4: 实现状态机**

每个阶段开始前写入 checkpoint；成功后写入完成状态；失败时只保存必要的应用 ID、阶段、版本和临时文件路径，不保存任何账号信息。

- [ ] **Step 5: 实现重试、跳过和取消**

重试只重新执行当前应用的失败阶段；跳过后将该应用标记为 `Skipped` 并继续下一个应用；取消立即停止新进程、保留可恢复状态并清理无效临时文件。

- [ ] **Step 6: 运行状态机和集成测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml coordinator -- --nocapture`

Expected: 状态顺序、恢复、取消、跳过和失败映射测试 PASS。

- [ ] **Step 7: Commit**

```text
git add src-tauri/src/coordinator.rs src-tauri/src/domain.rs src-tauri/tests/coordinator.rs
git commit -m "feat: orchestrate resumable installation flow"
```

## Task 7: 实现日志、脱敏和 Tauri commands

**Files:**
- Create: `src-tauri/src/diagnostics.rs`
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/diagnostics.rs`

**Interfaces:**
- `redact_sensitive(input: &str) -> String`
- `write_event(event: &InstallEvent) -> Result<(), InstallError>`
- `export_diagnostics(destination: &Path) -> Result<PathBuf, InstallError>`
- Tauri commands: `get_preflight()`, `start_install()`, `retry_install()`, `skip_app()`, `cancel_install()`, `get_resume_checkpoint()`, `export_diagnostics()`

- [ ] **Step 1: 写脱敏测试**

输入包含 `token=abc123`、`Authorization: Bearer secret`、带用户名的 Windows 路径和普通日志；断言敏感值被替换，普通安装阶段文本保留。

- [ ] **Step 2: 运行诊断测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml diagnostics`

Expected: 测试因脱敏函数尚未实现而 FAIL。

- [ ] **Step 3: 实现滚动日志和诊断导出**

日志写入用户数据目录，单文件达到 5 MB 时轮换，最多保留 3 个文件；诊断导出只包含脱敏日志、系统版本、架构和安装阶段。

- [ ] **Step 4: 实现 Tauri commands**

commands 只负责参数校验、调用服务和序列化结果，不把平台安装逻辑放入 command 函数；长任务通过事件发送 `InstallEvent`。

- [ ] **Step 5: 运行诊断和 Rust 全量测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: 所有 Rust 单元测试和集成测试 PASS，日志中没有敏感值。

- [ ] **Step 6: Commit**

```text
git add src-tauri/src/diagnostics.rs src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: expose install commands and redacted diagnostics"
```

## Task 8: 实现小白用户安装向导界面

**Files:**
- Create: `src/ui/types.ts`
- Create: `src/ui/copy.ts`
- Create: `src/ui/render.ts`
- Create: `src/api.ts`
- Create: `src/app.ts`
- Create: `src/styles.css`
- Modify: `src/main.ts`
- Test: `src/ui/render.test.ts`

**Interfaces:**
- `Screen = "welcome" | "preflight" | "confirm" | "progress" | "complete" | "failure"`
- `UiState { screen, preflight, apps, activeApp, events, error }`
- `render(state: UiState, root: HTMLElement): void`
- `startInstall(): Promise<void>`

- [ ] **Step 1: 写页面状态渲染测试**

使用 Vitest + JSDOM 验证欢迎页只有“开始安装/退出”、检测页显示三种状态、进度页显示两个应用、失败页显示重试/跳过/日志入口、完成页显示两个启动按钮。

- [ ] **Step 2: 运行前端测试确认失败**

Run: `npm run test -- src/ui/render.test.ts`

Expected: 测试因渲染函数尚未实现而 FAIL。

- [ ] **Step 3: 实现固定中文文案和状态类型**

将所有面向用户的文案集中在 `src/ui/copy.ts`；禁止在渲染函数中直接展示 Rust 错误名、HTTP 状态码或 shell 命令。

- [ ] **Step 4: 实现页面渲染和事件订阅**

前端只消费 Tauri commands 和 `InstallEvent`，按 `UiState` 渲染；主按钮在任一时刻只有一个主要动作；取消授权显示“你取消了授权，可稍后重试”。

- [ ] **Step 5: 实现无障碍和错误状态样式**

为按钮提供键盘焦点、禁用态和明确的 `aria-label`；错误使用图标加文字，不只依靠颜色；进度显示百分比和当前软件名称。

- [ ] **Step 6: 运行前端测试和构建**

Run: `npm run test`

Expected: 所有前端测试 PASS。

Run: `npm run build`

Expected: Vite 构建成功。

- [ ] **Step 7: Commit**

```text
git add src/main.ts src/app.ts src/api.ts src/styles.css src/ui
git commit -m "feat: add beginner-friendly installer wizard"
```

## Task 9: 接入真实安装包清单并配置打包

**Files:**
- Modify: `assets/manifest.json`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/Cargo.toml`
- Create: `.github/workflows/build.yml`
- Create: `scripts/validate-manifest.mjs`
- Test: `scripts/validate-manifest.test.mjs`

**Interfaces:**
- `validate-manifest.mjs` 必须验证每个应用包含 Windows x64、macOS x64、macOS arm64 三个平台条目。
- CI 必须执行 `npm ci`、`npm run test`、`cargo test` 和 `npm run tauri build`。

- [ ] **Step 1: 写清单验证测试**

验证正式清单中每个 URL 为 HTTPS、SHA-256 为 64 位十六进制、文件大小大于 0、平台条目完整，缺失时退出码为 1。

- [ ] **Step 2: 运行清单验证测试确认失败**

Run: `node scripts/validate-manifest.test.mjs`

Expected: 在正式包信息未填入前，测试明确报告缺失的发行包字段。

- [ ] **Step 3: 核验并填入真实发行包信息**

只使用 Codex 和 CC Switch 的官方发行包，记录版本、文件大小、SHA-256、安装包类型和架构；不得把未经核验的临时 URL 发布给用户。

- [ ] **Step 4: 配置 Windows 和 macOS 打包**

配置应用名称、图标、输出目录、最低系统版本、NSIS 安装行为和 macOS DMG/PKG 输出；默认不启用静默安装。

- [ ] **Step 5: 添加 CI 构建矩阵**

Windows runner 构建 Windows 安装包，macOS runner 分别构建 Intel 和 Apple Silicon 产物；CI 只构建和测试，不上传用户诊断数据。

- [ ] **Step 6: 运行本地构建检查**

Run: `node scripts/validate-manifest.mjs assets/manifest.json`

Expected: 输出三个目标平台均通过校验。

Run: `npm run tauri build`

Expected: 生成对应平台安装产物，并且应用启动后不自动执行安装流程。

- [ ] **Step 7: Commit**

```text
git add assets/manifest.json src-tauri/tauri.conf.json src-tauri/Cargo.toml scripts .github
git commit -m "build: configure release manifest and platform packages"
```

## Task 10: 完成跨平台验收和发布检查

**Files:**
- Create: `docs/testing-matrix.md`
- Create: `docs/release-checklist.md`
- Modify: `README.md`

- [ ] **Step 1: 建立手工测试矩阵**

在 `docs/testing-matrix.md` 中列出 Windows 10 x64、Windows 11 x64、macOS Intel、macOS Apple Silicon 四个环境，以及全新安装、已安装旧版本、已安装新版本、网络中断、校验失败、取消授权、中途退出七类场景。

- [ ] **Step 2: 执行全量自动化测试**

Run: `npm run test`

Expected: 前端测试全部 PASS。

Run: `cargo test --manifest-path src-tauri/Cargo.toml`

Expected: Rust 单元测试和集成测试全部 PASS。

- [ ] **Step 3: 执行四个平台手工测试**

逐项记录检测结果、安装结果、启动结果、日志脱敏结果和恢复结果；任一平台出现“安装器提示成功但应用不存在”时，不得发布。

- [ ] **Step 4: 执行发布安全检查**

确认 Windows 签名、macOS 签名/公证状态、所有清单 URL 为 HTTPS、所有包 SHA-256 与实际文件一致，安装器不会输出账号或 Token。

- [ ] **Step 5: 编写使用说明**

`README.md` 只说明下载、安装、首次登录、失败时导出诊断信息和反馈方式，不要求用户执行命令行操作。

- [ ] **Step 6: Commit**

```text
git add docs/testing-matrix.md docs/release-checklist.md README.md
git commit -m "docs: add cross-platform release verification"
```

## 计划自检

- 规格覆盖：目标平台、安装范围、用户流程、版本清单、下载校验、权限、恢复、日志脱敏、Tauri 技术选型和验收矩阵均有对应任务。
- 占位符检查：计划不使用模糊占位词或“实现后再补”等描述。
- 接口一致性：`AppId`、`PlatformTarget`、`AppRelease`、`InstallError`、`InstallEvent`、`InstallCoordinator` 和 `AppAdapter` 在前后任务中使用同一命名。
- 范围检查：第一版只实现安装编排，不扩展到登录、Token、配置迁移、代理或软件商店。
- 当前仓库状态：目录尚未初始化 Git；计划保留提交步骤，但实现前必须先初始化仓库或明确采用其他版本管理方式。
