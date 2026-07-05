# Codex Atlas Rust 重写交接文档

## 使用目的

本文档用于下一次全新项目会话接手 `codex-atlas-rust`。下一会话不要继续修补 Go + Walk 旧界面，而是以 Rust + Tauri 重新开发 Codex Atlas，同时复用当前项目已经验证过的需求、数据口径、测试思路和第二版前端设计。

当前新副本路径：

```text
F:\workspace\small-projects\codex-atlas-rust
```

Git 基线：

```text
origin: git@github.com:chenquanda/codex-atlas.git
branch: dev
base commit: 9a98c6b 实现第二版 Skill 详情与翻译缓存
```

## 总目标

开发一个 Windows 个人自用的轻量 Codex 能力字典侧边栏，用来扫描、展示、整理和调用本机 Codex 的 `skills`、`plugins`、`tools` 和 `apps`。

核心体验：

- 启动快，常驻内存和 CPU 占用低。
- 浅色调，界面比第一版更宽、更清楚，适合侧边栏但不压缩信息。
- 主视图以 Skills 为核心，支持搜索、筛选、排序、收藏、隐藏、备注、标签、复制调用模板。
- 点击 Skill 后打开独立详情窗口，浏览该 Skill 文件夹内文件，并显示文件内容。
- 英文 Skill 内容可手动翻译，翻译结果缓存；已有缓存时默认显示中文译文。
- 中文 Skill 不显示翻译按钮，也不重复翻译。
- 扫描、统计、翻译、AI 整理都由用户手动触发，不做后台持续任务。

## 必须遵守的开发约束

- 只能在当前项目目录 `F:\workspace\small-projects\codex-atlas-rust` 内增删改代码、文档、缓存和构建产物。
- 可以只读访问 Codex 数据目录，例如 `C:\Users\Administrator\.codex` 和 `C:\Users\Administrator\.agents\skills`，但不能写入、删除或移动这些目录里的文件。
- Rust、Cargo、Tauri 工具链已经安装到 `F:\workspace\dev-tools`，不要下载到 C 盘。
- 构建缓存优先放到当前项目内，例如 `.cache/cargo-target`、`.cache/npm`、`.tmp`。
- 代码注释使用中文，重点给扫描、统计、缓存、路径边界、外部命令调用和并发状态写注释。
- 每个小功能按 TDD 推进：先写测试或可验证用例，再实现，再验证。
- 每个阶段完成后需要独立代码审查；修复 Critical / Important 后再进入下一阶段。
- 不要把当前源项目里未提交的 Go UI 实验改动作为新版本基础。

## 已安装工具链

Rust / Tauri 已准备好：

```text
RUSTUP_HOME=F:\workspace\dev-tools\rust\rustup
CARGO_HOME=F:\workspace\dev-tools\rust\cargo
PATH += F:\workspace\dev-tools\rust\cargo\bin
```

已验证版本：

```text
rustc 1.96.1
cargo 1.96.1
tauri-cli 2.11.4
node v24.17.0
npm 11.13.0
bun 1.3.14
```

MSVC Build Tools：

```text
F:\workspace\dev-tools\vs-buildtools
```

建议下一会话先设置项目级构建缓存：

```powershell
$env:RUSTUP_HOME = "F:\workspace\dev-tools\rust\rustup"
$env:CARGO_HOME = "F:\workspace\dev-tools\rust\cargo"
$env:PATH = "F:\workspace\dev-tools\rust\cargo\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "$PWD\.cache\cargo-target"
$env:TEMP = "$PWD\.tmp"
$env:TMP = "$PWD\.tmp"
```

## 重要资料入口

请下一会话优先阅读这些文件：

- `docs/Codex能力字典桌面侧边栏工具箱.md`：产品背景、核心功能、明确不做的功能。
- `docs/方案设计.md`：第一版到第二版的架构、数据模型、统计口径、翻译缓存设计。
- `docs/实施计划.md`：旧版阶段拆分和测试审查流程，可改造成 Rust/Tauri 版计划。
- `docs/开发记录.md`：已踩过的问题、验证结果、性能数据和审查结论。
- `docs/使用说明.md`：当前功能说明，可作为新版 README/帮助文档素材。
- `docs/superpowers/plans/2026-07-05-Codex-Atlas-第二版.md`：第二版实施计划。
- `design-demos/Codex Atlas 第二版界面设计.html`：新版前端界面的视觉基准，Rust/Tauri 前端应尽量以它为准。

## 设计稿要求

第二版前端界面直接保留，以这个文件作为主视觉基准：

```text
design-demos/Codex Atlas 第二版界面设计.html
```

界面方向：

- 不是简单放大旧界面，而是重新布局控件。
- 主窗口可以比第一版更宽，避免文字和按钮拥挤。
- 作为侧边栏工具，不要做成长页面；高度可以让用户手动拖长。
- 顶部要清楚显示产品身份、搜索入口和主要视图切换。
- Skills 列表要能看清名称、说明、标签和使用次数。
- 低频选项可通过一个按钮展开，例如“更多”或设置按钮。
- Skill 详情使用独立窗口，不挤在主窗口内。
- 有中文翻译缓存时，摘要和内容默认优先显示中文。

## 可复用的 Go 模块思路

旧版 Go 代码不要求原样迁移，但其中的领域模型和边界处理值得复用。

### `internal/domain`

可复用内容：

- `Ability`、`AbilityKind`、`Stats`、`UserData`、`AIData` 等核心概念。
- 原始数据、AI 数据、用户数据分层合并的规则。
- 用户数据优先：备注、收藏、隐藏、手动标签、自定义模板不能被 AI 覆盖。

Rust 建议：

- 放到 `src-tauri/src/domain.rs` 或 `src-tauri/src/domain/`。
- 使用 `serde::{Serialize, Deserialize}`。
- 给关键合并逻辑保留单元测试。

### `internal/config`

可复用内容：

- Codex 数据目录解析。
- 项目内 `data/`、`.tmp/`、`dist/` 路径约束。
- `dist` 启动和 portable 启动时的数据路径判断。

Rust 建议：

- 所有写入路径必须经过项目根或应用数据根校验。
- 外部 Codex 目录只读。
- 对 symlink / junction 要做真实路径校验，避免写出项目目录。

### `internal/scanner`

可复用内容：

- 扫描 `skills/**/SKILL.md`。
- 扫描插件缓存里的 `.codex-plugin/plugin.json`。
- 插件 skill 使用 `pluginName:skillName`，避免短名冲突。
- 解析 YAML front matter 的 `name` 和 `description`，没有时用目录名和 Markdown fallback。

Rust 建议：

- 使用 `walkdir` 控制遍历深度和跳过目录。
- 使用 `serde_yaml` 或 `gray_matter` 解析 front matter。
- 扫描器要返回可展示错误说明，但不能因为单个坏文件中断全部扫描。

### `internal/stats`

可复用内容：

- 主指标是 `usageCount`，不是会话数。
- 用户消息中每个显式 `$skill` / `$plugin:skill` 计 1 次使用。
- 助手实际读取对应 `SKILL.md` 的 function call 计 1 次使用。
- 同一会话内重复出现要重复计数。
- 不统计 developer/system 消息、function output、工具输出、skills 列表说明文本。
- 短名只有唯一时才匹配，插件重名必须优先匹配完整名。
- 没有新版统计缓存时 UI 显示“未统计”，不要显示误导性的 `0 使用`。

Rust 建议：

- 流式读取 `sessions/**/*.jsonl`，避免一次性加载历史会话。
- 用索引表匹配 token，不要每行对每个 skill 跑正则。
- 缓存写入 `data/stats-cache.json`。

### `internal/skilldoc`

可复用内容：

- Skill 详情窗口只读取当前 Skill 文件夹内可预览文件。
- 支持 Markdown、文本、代码和配置类文件。
- 跳过依赖目录、构建目录、过大文件。
- 限制最大文件数、最大深度和单文件大小。
- 禁止路径逃逸 Skill 根目录。

Rust 建议：

- 文件列表命令由 Tauri command 暴露给前端。
- 读取文件时再次校验真实路径在 Skill 根目录下。
- 返回相对路径、文件名、扩展名、大小、内容哈希和是否可翻译。

### `internal/translate`

可复用内容：

- 翻译缓存文件：`data/translation-cache.json`。
- 缓存 key 使用：

```text
skillID + relativePath + contentHash
```

- 缓存命中时默认显示中文译文。
- 内容变更后旧缓存失效。
- 中文内容不显示翻译按钮。

Rust 建议：

- 语言判断先用轻量启发式，不需要引入重型模型。
- 翻译缓存读写要原子化，避免中途失败损坏 JSON。

### `internal/ai`

可复用内容：

- 翻译和 AI 整理都通过本机 `codex exec --ephemeral`。
- 输入输出写在项目内 `.tmp`。
- 调用失败、超时、输出为空或格式错误时，UI 显示错误，不破坏本地数据。
- 翻译只处理当前文件，不自动批量翻译整个 skills 仓库。

Rust 建议：

- 使用 `std::process::Command` 或 `tauri::async_runtime::spawn_blocking`。
- 固定 `TEMP/TMP` 到项目 `.tmp`。
- 对输出目录做真实路径校验。
- 命令执行要有超时和取消/关闭窗口后的状态保护。

### `internal/app`

可复用内容：

- 应用服务层负责聚合扫描结果、用户数据、AI 数据、统计、Skill 详情和翻译缓存。
- UI 不直接操作存储文件和 Codex 源目录。
- 刷新、统计、AI、保存等任务需要串行化，避免并发写 JSON。

Rust 建议：

- 用 `AppState` + `Mutex/RwLock` 管理共享状态。
- 每个 Tauri command 只做一件事，返回前端可展示的数据结构。
- 重要命令都写单元测试或集成测试。

## 新版建议技术结构

建议使用 Tauri v2 + Rust 后端 + Web 前端。

可选前端栈：

- 推荐：Vite + React + TypeScript。生态成熟，适合把现有 HTML 设计拆成组件。
- 也可选：Vite + Svelte。运行时更轻，但团队/工具熟悉度可能低一点。

建议目录：

```text
src/
  components/
  pages/
  styles/
  api/
src-tauri/
  src/
    main.rs
    domain/
    config/
    scanner/
    store/
    stats/
    skilldoc/
    translate/
    ai/
    commands/
  tauri.conf.json
data/
docs/
design-demos/
.cache/
.tmp/
dist/
```

前端原则：

- 以 `design-demos/Codex Atlas 第二版界面设计.html` 为视觉基准。
- 不做营销首页，启动即主工具界面。
- 主窗口信息密度要适中，不能像第一版那样太挤。
- 按钮、开关、筛选、展开面板要有清楚状态。
- 详情窗口独立，文件列表和内容阅读区要能长期阅读。

后端原则：

- 先移植核心数据和测试，再接 UI。
- 所有外部目录只读。
- 所有写入使用项目内 `data/` 和 `.tmp/`。
- 扫描和统计要流式、可取消、可错误降级。

## 下一会话建议开发节奏

### 阶段 0：项目初始化

- 确认 `dev` 分支和工作区干净。
- 设置 Rust/Cargo/Node 缓存到项目内。
- 初始化 Tauri v2 项目。
- 把第二版 HTML 设计拆成静态前端页面，先做到无后端数据也能展示。
- 验证：`npm run build`、`cargo test`、`cargo tauri build` 或等价命令能跑通。

### 阶段 1：领域模型与存储

- 移植 `Ability`、`Stats`、用户数据、AI 数据和合并规则。
- 实现 `data/atlas-store.json`、`data/last-scan.json`、`data/stats-cache.json`、`data/translation-cache.json` 的读写。
- TDD 覆盖用户数据优先、坏 JSON 降级、写入路径边界。

### 阶段 2：扫描器

- 实现 skills / plugins 扫描。
- 先不急着做 Tools / Apps 的复杂运行时识别。
- TDD 覆盖 front matter、fallback、插件 skill、重名短名。

### 阶段 3：主界面数据接入

- 前端调用 Tauri command 获取能力列表。
- 支持搜索、视图切换、快速筛选、排序、列表选择和底部摘要。
- 支持收藏、隐藏、备注、标签、模板复制、打开文件/目录/复制路径。

### 阶段 4：使用统计

- 实现真实使用次数统计。
- 接入 `data/stats-cache.json`。
- UI 显示 `N 使用` 或 `未统计`。
- TDD 覆盖同会话多次使用、developer/system/tool output 排除、读取 `SKILL.md` 计数。

### 阶段 5：Skill 详情窗口

- 点击 Skill 打开独立详情窗口。
- 左侧文件列表，右侧文件内容。
- 支持原文/译文切换。
- 中文文件隐藏翻译按钮。
- 缓存命中默认中文。

### 阶段 6：翻译和 AI 整理

- 翻译当前文件：调用 `codex exec --ephemeral`。
- AI 整理当前可见列表，只写 AI 数据层。
- 加超时、错误显示、关闭窗口后的状态保护。

### 阶段 7：打包、性能和审查

- 构建 Windows exe / portable 包。
- 验证启动速度、内存、CPU。
- GUI smoke：主窗口、详情窗口、翻译按钮、缓存命中、中文文件隐藏翻译。
- 代码审查：路径边界、缓存口径、并发写入、外部目录只读、中文注释。

## 推荐测试清单

Rust 后端：

- `cargo test`
- 扫描 fixture：local skill、plugin skill、坏 front matter、重名 skill。
- stats fixture：多次 `$skill`、`$plugin:skill`、developer 噪音、function output 噪音、读取 `SKILL.md`。
- skilldoc fixture：路径逃逸、过大文件、跳过目录、内容 hash。
- translate fixture：缓存命中、内容变更失效、中文内容不需要翻译。

前端：

- TypeScript 类型检查。
- 组件测试或轻量 Playwright smoke。
- 截图确认主界面接近第二版 HTML 设计。
- 截图确认详情窗口可阅读、按钮不重叠、长文本不挤压。

集成：

- `cargo tauri dev` 能启动。
- `cargo tauri build` 能产出 exe。
- 真实 Codex 数据 smoke：扫描 abilities / skills 数量合理。
- 刷新统计后列表不是全 `未统计`，有真实 `N 使用`。
- 关闭重启后统计缓存和翻译缓存能加载。

## 建议调用的 skills

下一会话建议按顺序使用：

- `superpowers:brainstorming`：确认 Rust/Tauri 版拆分边界，不要直接写代码。
- `superpowers:writing-plans`：把本文档拆成可执行计划。
- `superpowers:test-driven-development`：每个后端功能先测试再实现。
- `superpowers:subagent-driven-development`：分阶段执行时使用子 agent 做独立任务或复审。
- `superpowers:requesting-code-review`：阶段完成后请求审查。
- `redesign-existing-projects` 或 `design-taste-frontend`：把第二版 HTML 设计转成真实 Tauri 前端时使用。
- `auto-git-sync`：阶段完成后提交并推送。

## 不要继续沿用的内容

- 不要继续用 Go + Walk 作为最终 UI 技术栈。
- 不要把当前源项目中未提交的 Go UI 表格化/样式实验当作基础。
- 不要为了快速实现而回到第一版拥挤 UI。
- 不要自动批量翻译所有 Skill。
- 不要把未知统计显示成 `0 使用`。
- 不要让 AI 整理覆盖用户手动标签、备注、收藏、隐藏和模板。

## 可以保留作为参考的旧版成果

- 统计口径已经验证过，必须保留“真实使用次数优先”。
- 翻译缓存 key 和手动翻译策略已经验证过，建议原样迁移。
- 路径边界和真实路径校验已经踩过坑，Rust 版一开始就要做。
- 当前构建/运行性能基准可作为新版本参考：
  - Go 版 GUI smoke 工作集约 `27.4MB`。
  - CPU 时间约 `0.25s`。
  - 真实扫描约 `abilities=120`、`skills=87`。

Rust/Tauri 版不一定要完全达到 Go 版内存，但必须保持轻量，不要向 Electron 级别膨胀。
