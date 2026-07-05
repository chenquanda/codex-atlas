# Codex Atlas Rust 重写交接文档

## 当前状态

本项目是 Codex Atlas 的 Rust + Tauri 重写起点，路径为：

```text
F:\workspace\small-projects\codex-atlas-rust
```

当前分支：

```text
dev
```

这个分支已经主动清理旧 Go + Walk 源码、旧 exe、旧构建脚本、旧缓存数据和旧备选设计稿，只保留重写所需的最小上下文：

- `docs/Rust重写交接文档.md`
- `design-demos/Codex Atlas 第二版界面设计.html`
- `README.md`

旧实现如需追溯，可去 Git 历史或 `main` 分支查看，但新开发不要把旧 Go 代码作为实现参考。

## 总目标

用 Rust + Tauri v2 重新开发 Codex Atlas：一个 Windows 个人自用的轻量 Codex 能力字典侧边栏。

它要帮助用户查看、整理和调用本机 Codex 能力：

- skills
- plugins
- tools
- apps

核心体验：

- 启动快，常驻内存和 CPU 占用低。
- 浅色调，界面比第一版更宽、更清楚。
- 第一屏就是可用工具界面，不做营销页。
- 主视图以 Skills 为核心。
- 点击 Skill 打开独立详情窗口。
- 可浏览 Skill 文件夹内可阅读文件。
- 英文内容可手动翻译，翻译结果缓存。
- 有中文翻译缓存时默认显示中文。
- 中文内容不显示翻译按钮。
- 扫描、统计、翻译、AI 整理都由用户手动触发，不做后台持续任务。

## 硬性约束

- 只能在当前项目目录内增删改文件。
- 可以只读访问 `C:\Users\Administrator\.codex` 和 `C:\Users\Administrator\.agents\skills`。
- 不能写入、删除或移动当前项目外的 Codex 数据目录。
- Rust、Cargo、Tauri 工具链使用 `F:\workspace\dev-tools`，不要下载到 C 盘。
- 构建缓存、Node 缓存、临时文件和测试产物优先放在当前项目内。
- 代码注释使用中文。
- 每个功能先测试或验证用例，再实现，再验证。
- 每个任务都要经过独立审查，修复 Critical / Important 后才能进入下一个任务。
- 不要把未知统计显示为 `0 使用`，未知应显示为 `未统计`。
- 不要让 AI 整理覆盖用户手动标签、备注、收藏、隐藏和自定义模板。
- 不要自动批量翻译所有 Skill。

## 工具链

已安装：

```text
rustc 1.96.1
cargo 1.96.1
tauri-cli 2.11.4
node v24.17.0
npm 11.13.0
bun 1.3.14
```

位置：

```text
RUSTUP_HOME=F:\workspace\dev-tools\rust\rustup
CARGO_HOME=F:\workspace\dev-tools\rust\cargo
MSVC Build Tools=F:\workspace\dev-tools\vs-buildtools
```

建议每次开发前设置：

```powershell
$env:RUSTUP_HOME = "F:\workspace\dev-tools\rust\rustup"
$env:CARGO_HOME = "F:\workspace\dev-tools\rust\cargo"
$env:PATH = "F:\workspace\dev-tools\rust\cargo\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "$PWD\.cache\cargo-target"
$env:TEMP = "$PWD\.tmp"
$env:TMP = "$PWD\.tmp"
```

## 设计基准

唯一保留的设计稿：

```text
design-demos/Codex Atlas 第二版界面设计.html
```

前端实现必须以它为基准：

- 不要回到第一版窄而挤的布局。
- 不要做成长页面，仍保持侧边栏工具形态。
- 主窗口可比第一版更宽，用户也可以手动拖高。
- 低频操作用按钮展开，例如“更多”或设置入口。
- Skill 详情用独立窗口，不挤在主窗口里。
- 文本、按钮、标签和列表项不能重叠或被截断得难以使用。

## 建议技术结构

推荐：

```text
Rust + Tauri v2 + Vite + React + TypeScript
```

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

如果选择其他轻量前端框架，必须说明原因，并保证能贴近设计稿。

## 业务规则

### 数据分层

数据分三层：

- 原始数据：从本机 Codex 目录只读扫描得到。
- AI 数据：中文摘要、AI 标签、调用模板、适用场景等。
- 用户数据：别名、手动标签、备注、收藏、隐藏、自定义模板。

合并规则：

- 用户数据优先级最高。
- 扫描只能重建原始数据。
- AI 整理只能更新 AI 数据。
- 用户数据不能被扫描或 AI 覆盖。

### 扫描

扫描对象：

- 本地 skills。
- 插件缓存中的 skills。
- 插件清单。
- tools 和 apps 的可展示信息。

要求：

- 外部 Codex 目录只读。
- 单个坏文件不能中断全局扫描。
- 插件 skill 使用 `pluginName:skillName`，避免短名冲突。
- 解析 `SKILL.md` 的 front matter；没有 front matter 时使用目录名和 Markdown fallback。

### 使用统计

主指标是 `UsageCount`。

计数规则：

- 用户消息中每个显式 `$skill` 计 1 次。
- 用户消息中每个显式 `$plugin:skill` 计 1 次。
- 助手实际读取对应 `SKILL.md` 的记录计 1 次。
- 同一会话内重复出现要重复计数。

不计数：

- developer 消息。
- system 消息。
- function output。
- tool output。
- skills 列表说明文本。

显示规则：

- 有统计时显示 `N 使用`。
- 没有新版缓存或统计未知时显示 `未统计`。
- 不要把未知状态伪装成 `0 使用`。

### Skill 详情

点击 Skill 后打开独立详情窗口。

详情窗口要求：

- 左侧显示该 Skill 文件夹内可阅读文件。
- 右侧显示当前文件内容。
- 支持 Markdown、文本、代码和配置类文件。
- 跳过依赖目录、构建目录、过大文件。
- 限制最大文件数、最大深度和单文件大小。
- 文件读取必须校验真实路径，不能逃逸 Skill 根目录。

### 翻译缓存

翻译只在用户手动触发时执行。

缓存文件：

```text
data/translation-cache.json
```

缓存 key：

```text
skillID + relativePath + contentHash
```

规则：

- 中文内容不显示翻译按钮。
- 非中文内容可手动点击翻译。
- 缓存命中时默认显示中文译文。
- 内容变更后旧缓存失效。
- 不自动批量翻译整个 skills 仓库。

### AI 调用

翻译和 AI 整理都通过本机：

```text
codex exec --ephemeral
```

要求：

- 输入输出写入项目内 `.tmp`。
- 固定 `TEMP/TMP` 到项目 `.tmp`。
- 设置超时和错误处理。
- 输出为空、格式错误或命令失败时，UI 显示错误，不破坏本地数据。

## 推荐开发节奏

### 阶段 0：计划与初始化

- 读取本文档和第二版设计稿。
- 先写完整开发计划，保存到 `docs/superpowers/plans/`。
- 初始化 Tauri v2 项目。
- 配置项目内缓存。
- 做一个贴近设计稿的静态前端壳。

### 阶段 1：核心模型与存储

- 实现能力、统计、用户数据、AI 数据模型。
- 实现 JSON 存储。
- 测试用户数据优先、坏 JSON 降级、路径边界。

### 阶段 2：扫描器

- 实现 skills 和 plugins 扫描。
- 补 tools/apps 的可展示信息。
- 测试 front matter、fallback、插件 skill、重名 skill、坏文件降级。

### 阶段 3：主界面接入

- 前端调用 Tauri command 获取能力列表。
- 实现搜索、筛选、排序、列表选择和底部摘要。
- 实现收藏、隐藏、备注、标签、模板复制、打开文件/目录/复制路径。

### 阶段 4：使用统计

- 实现真实使用次数统计。
- 接入 `data/stats-cache.json`。
- UI 显示 `N 使用` 或 `未统计`。
- 测试噪音排除和同会话重复计数。

### 阶段 5：Skill 详情窗口

- 实现独立详情窗口。
- 实现文件列表、文件内容阅读、路径边界校验。
- 实现原文 / 译文切换状态。

### 阶段 6：翻译与 AI 整理

- 实现单文件手动翻译。
- 实现翻译缓存。
- 实现 AI 整理当前可见列表，只写 AI 数据层。

### 阶段 7：打包、性能与审查

- 构建 Windows exe。
- 做 GUI smoke。
- 验证启动速度、内存、CPU。
- 更新 README 和使用说明。
- 通过最终代码审查。
- 提交并推送 `origin/dev`。

## Subagent-Driven 要求

下一会话执行时必须使用 Subagent-Driven Development：

- 主 agent 负责计划、拆任务、调度、审查和最终合并。
- 每个开发任务派一个新的 implementer 子 agent。
- 不要并行派多个实现子 agent 改代码。
- 每个任务完成后，主 agent 派 spec reviewer 子 agent 检查是否满足需求。
- spec 问题修复并复审通过后，再派 code quality reviewer 子 agent。
- 质量问题修复并复审通过后，才能进入下一个任务。
- 全部任务完成后，再派 final reviewer 子 agent 做整体审查。

推荐使用 skills：

- `superpowers:brainstorming`
- `superpowers:writing-plans`
- `superpowers:test-driven-development`
- `superpowers:subagent-driven-development`
- `superpowers:requesting-code-review`
- `superpowers:verification-before-completion`
- `auto-git-sync`

## 验证清单

后端：

- `cargo test`
- 扫描 fixture。
- 统计 fixture。
- Skill 文件读取 fixture。
- 翻译缓存 fixture。

前端：

- TypeScript 类型检查。
- 前端构建。
- 截图确认接近第二版设计稿。
- 检查主窗口和详情窗口没有明显遮挡、重叠、截断。

集成：

- Tauri dev 能启动。
- Tauri build 能产出 exe。
- 真实 Codex 数据扫描数量合理。
- 刷新统计后不是全 `未统计`。
- 翻译缓存命中后默认显示中文。
- 中文内容不显示翻译按钮。

## 完成标准

- Rust + Tauri 项目可运行。
- 第二版设计被真实实现，而不是只做旧界面微调。
- 核心功能通过测试。
- GUI smoke 通过。
- 文档更新。
- 子 agent 审查问题已修复。
- `dev` 分支提交并推送。
