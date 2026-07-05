# Codex Atlas Rust + Tauri 重写实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. 每个开发任务由新的 implementer 子 agent 执行；主 agent 再派 spec reviewer 和 code quality reviewer 审查。Critical / Important 问题必须修复并复审通过后，主 agent 才能提交该任务。

**Goal:** 在 `F:\workspace\small-projects\codex-atlas-rust` 的 `dev` 分支完整实现 Codex Atlas 的 Rust + Tauri v2 重写版。

**Architecture:** 使用 Rust + Tauri v2 负责本机扫描、统计、存储、Skill 文件读取、翻译命令和路径边界；使用 Vite + React + TypeScript 实现第二版浅色工作面板 UI。数据分为原始扫描数据、AI 数据和用户数据三层，合并时用户数据永远优先，AI 和扫描都不能覆盖用户手写内容。

**Tech Stack:** Rust 1.96.1, Tauri CLI 2.11.4, Tauri v2, Vite, React, TypeScript, Vitest, Playwright smoke, Windows PowerShell scripts.

---

## 固定约束

- 只允许在当前项目目录内增删改文件：`F:\workspace\small-projects\codex-atlas-rust`。
- 只读访问外部目录：`C:\Users\Administrator\.codex`、`C:\Users\Administrator\.agents\skills`、`F:\workspace\dev-tools`。
- 不写入、删除、移动任何项目外文件。
- 不修改旧项目：`F:\workspace\small-projects\codex-atlas`。
- 不复制旧 Go + Walk 代码；业务规则以 `docs/Rust重写交接文档.md` 为准。
- 文档文件名使用中文；代码注释使用中文，重点说明路径边界、扫描、统计、缓存、外部命令和并发状态。
- Rust/Cargo/Node 缓存固定在当前项目：`.cache/`、`.tmp/`。
- 每个任务 TDD：先写测试或可验证脚本，运行看到失败，再实现，最后运行通过。
- implementer 子 agent 不提交；主 agent 完成审查和验证后提交。
- 不并行派多个实现子 agent 改代码。
- 翻译只能手动触发，不自动批量翻译所有 Skill。
- 未知统计显示 `未统计`，不能显示成 `0 使用`。

## 开发环境命令

所有 Rust/Tauri/Node 命令前使用同一段环境设置：

```powershell
$env:RUSTUP_HOME = "F:\workspace\dev-tools\rust\rustup"
$env:CARGO_HOME = "F:\workspace\dev-tools\rust\cargo"
$env:PATH = "F:\workspace\dev-tools\rust\cargo\bin;F:\workspace\dev-tools\bun;$env:PATH"
$env:CARGO_TARGET_DIR = "$PWD\.cache\cargo-target"
$env:npm_config_cache = "$PWD\.cache\npm"
$env:TEMP = "$PWD\.tmp"
$env:TMP = "$PWD\.tmp"
New-Item -ItemType Directory -Force ".cache\cargo-target", ".cache\npm", ".tmp" | Out-Null
```

主验证命令：

```powershell
npm run test:rust
npm run test:frontend
npm run typecheck
npm run build
npm run tauri:build
npm run smoke:scan
npm run smoke:skill-detail
npm run smoke:translation-cache
npm run smoke:gui
```

## 目标文件结构

```text
package.json
package-lock.json
index.html
vite.config.ts
tsconfig.json
tsconfig.node.json
playwright.config.ts
src/
  main.tsx
  App.tsx
  api/atlasApi.ts
  api/tauriBridge.ts
  components/
    AbilityList.tsx
    AbilityToolbar.tsx
    BottomPanel.tsx
    DetailWindow.tsx
    EditableMetadata.tsx
    FilterBar.tsx
    MetricCards.tsx
    SearchBox.tsx
  lib/
    abilityFilters.ts
    abilityFormatting.ts
    language.ts
  styles/atlas.css
  test/
    fixtures.ts
    setup.ts
src-tauri/
  Cargo.toml
  Cargo.lock
  build.rs
  tauri.conf.json
  capabilities/default.json
  src/
    main.rs
    lib.rs
    app_state.rs
    commands/mod.rs
    domain/mod.rs
    scanner/mod.rs
    skilldoc/mod.rs
    stats/mod.rs
    store/mod.rs
    translate/mod.rs
    util/mod.rs
  tests/
    fixtures.rs
    domain_store_tests.rs
    scanner_tests.rs
    stats_tests.rs
    skilldoc_tests.rs
    translate_tests.rs
    command_tests.rs
scripts/
  设置开发环境.ps1
  验证项目骨架.ps1
  smoke-scan.ps1
  smoke-skill-detail.ps1
  smoke-translation-cache.ps1
  smoke-gui.ps1
data/
  .gitkeep
docs/
  使用说明.md
  方案设计.md
  开发记录.md
  superpowers/plans/2026-07-05-Codex-Atlas-Rust-Tauri重写实施计划.md
dist/
  .gitkeep
```

## 任务 0：计划落盘与仓库基线

**Files:**
- Create: `docs/superpowers/plans/2026-07-05-Codex-Atlas-Rust-Tauri重写实施计划.md`
- Modify: none

- [ ] **Step 1: 写入计划文档**

  主 agent 使用 `apply_patch` 创建本文档。

- [ ] **Step 2: 验证基线**

  Run:

  ```powershell
  git branch --show-current
  git status -sb
  rg --files
  ```

  Expected:

  ```text
  dev
  ## dev...origin/dev
  README.md
  docs\Rust重写交接文档.md
  design-demos\Codex Atlas 第二版界面设计.html
  docs\superpowers\plans\2026-07-05-Codex-Atlas-Rust-Tauri重写实施计划.md
  ```

- [ ] **Step 3: 提交计划**

  Run:

  ```powershell
  git add docs/superpowers/plans/2026-07-05-Codex-Atlas-Rust-Tauri重写实施计划.md
  git commit -m "添加 Rust Tauri 重写实施计划

  - 在 docs/superpowers/plans 中记录完整开发计划
  - 明确 TDD、子代理审查、路径边界和缓存约束
  - 为后续 Rust + Tauri 重写拆分可独立提交的任务"
  ```

## 任务 1：初始化 Tauri v2 骨架与项目内缓存

**Files:**
- Create: `package.json`
- Create: `package-lock.json`
- Create: `index.html`
- Create: `vite.config.ts`
- Create: `tsconfig.json`
- Create: `tsconfig.node.json`
- Create: `src/main.tsx`
- Create: `src/App.tsx`
- Create: `src/App.test.tsx`
- Create: `src/styles/atlas.css`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/Cargo.lock`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/capabilities/default.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `src-tauri/src/commands/mod.rs`
- Create: `scripts/设置开发环境.ps1`
- Create: `scripts/验证项目骨架.ps1`
- Create: `.cache/.gitkeep`
- Create: `.tmp/.gitkeep`
- Create: `data/.gitkeep`
- Create: `dist/.gitkeep`
- Modify: `.gitignore`

- [ ] **Step 1: 写失败验证脚本**

  Create `scripts/验证项目骨架.ps1`:

  ```powershell
  $ErrorActionPreference = "Stop"
  $required = @(
    "package.json",
    "package-lock.json",
    "index.html",
    "vite.config.ts",
    "tsconfig.json",
    "tsconfig.node.json",
    "src\main.tsx",
    "src\App.tsx",
    "src\App.test.tsx",
    "src\styles\atlas.css",
    "src-tauri\Cargo.toml",
    "src-tauri\Cargo.lock",
    "src-tauri\build.rs",
    "src-tauri\tauri.conf.json",
    "src-tauri\capabilities\default.json",
    "src-tauri\src\main.rs",
    "src-tauri\src\lib.rs",
    "src-tauri\src\commands\mod.rs",
    "scripts\设置开发环境.ps1",
    ".cache",
    ".cache\.gitkeep",
    ".tmp",
    ".tmp\.gitkeep",
    "data",
    "data\.gitkeep",
    "dist",
    "dist\.gitkeep"
  )
  foreach ($path in $required) {
    if (-not (Test-Path $path)) {
      throw "缺少项目骨架文件或目录: $path"
    }
  }
  $pkg = Get-Content "package.json" -Raw | ConvertFrom-Json
  $expectedTauriBuild = "powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1 tauri build"
  if ($pkg.scripts."tauri:build" -ne $expectedTauriBuild) {
    throw "package.json tauri:build 必须通过设置开发环境脚本调用"
  }
  $conf = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
  if ($conf.identifier -ne "local.codex-atlas.rust") {
    throw "Tauri identifier 不正确"
  }
  Write-Host "项目骨架验证通过"
  ```

- [ ] **Step 2: 运行脚本确认失败**

  Run:

  ```powershell
  powershell -ExecutionPolicy Bypass -File scripts/验证项目骨架.ps1
  ```

  Expected: FAIL with `缺少项目骨架文件或目录`。

- [ ] **Step 3: 创建最小可运行骨架**

  Implement:

  - `package.json` scripts:

    ```json
    {
      "scripts": {
        "dev": "vite --host 127.0.0.1",
        "build": "tsc && vite build",
        "typecheck": "tsc --noEmit",
        "test:frontend": "vitest run",
        "test:rust": "powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1 cargo test --manifest-path src-tauri/Cargo.toml",
        "tauri:dev": "powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1 tauri dev",
        "tauri:build": "powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1 tauri build"
      }
    }
    ```

  - `src-tauri/src/lib.rs` exposes a `run()` function and a temporary `health` command returning `Codex Atlas` so the Tauri skeleton compiles.
  - `src/App.tsx` renders a static top-level `Codex Atlas` shell using the second-version color tokens.
  - `src/App.test.tsx` contains a minimal real Vitest check so `npm run test:frontend` does not fail because no tests exist.
  - `.gitignore` ignores `.cache/*`, `.tmp/*`, `node_modules/`, `src-tauri/gen/`, `src-tauri/target/`, `dist/*`, while keeping `.gitkeep` files.

- [ ] **Step 4: 验证骨架通过**

  Run:

  ```powershell
  powershell -ExecutionPolicy Bypass -File scripts/验证项目骨架.ps1
  npm install
  npm run typecheck
  npm run build
  npm run test:rust
  ```

  Expected: all commands exit 0.

## 任务 2：核心领域模型、三层合并与 JSON 存储

**Files:**
- Create: `src-tauri/src/domain/mod.rs`
- Create: `src-tauri/src/store/mod.rs`
- Create: `src-tauri/src/util/mod.rs`
- Create: `src-tauri/tests/domain_store_tests.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写领域与存储失败测试**

  Create `src-tauri/tests/domain_store_tests.rs` with tests named:

  ```rust
  #[test]
  fn user_data_wins_over_ai_and_raw_layers() {}

  #[test]
  fn unknown_usage_is_not_rendered_as_zero() {}

  #[test]
  fn malformed_json_returns_default_store_without_panic() {}

  #[test]
  fn store_paths_stay_inside_project_data_dir() {}
  ```

  Assertions:

  - `merge_layers(raw, ai, user)` keeps user tags, note, favorite, hidden, and custom template.
  - `Stats { usage_count: None }` serializes as unknown and formatting helper returns `未统计`.
  - bad JSON loads as empty `Store` with a warning vector.
  - path resolver rejects `..\outside.json`.

- [ ] **Step 2: 运行测试确认失败**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/domain_store_tests.rs
  ```

  Expected: FAIL because modules and functions are missing.

- [ ] **Step 3: 实现模型和存储**

  Implement Rust types:

  ```rust
  pub enum AbilityKind { Skill, Plugin, Tool, App }
  pub struct Ability { pub id: String, pub name: String, pub kind: AbilityKind, pub path: Option<PathBuf>, pub summary: String, pub raw_tags: Vec<String>, pub ai: AIData, pub user: UserData, pub stats: Stats }
  pub struct Stats { pub usage_count: Option<u64>, pub last_used_at: Option<String> }
  pub struct UserData { pub alias: Option<String>, pub tags: Vec<String>, pub note: Option<String>, pub favorite: bool, pub hidden: bool, pub custom_template: Option<String> }
  pub struct AIData { pub summary_zh: Option<String>, pub tags: Vec<String>, pub call_template: Option<String>, pub scenarios: Vec<String> }
  pub struct Store { pub abilities: Vec<Ability>, pub user_data: BTreeMap<String, UserData>, pub ai_data: BTreeMap<String, AIData>, pub stats: BTreeMap<String, Stats> }
  ```

  Add Chinese comments around:

  - 用户数据优先级。
  - JSON 降级读取。
  - 项目内 `data/` 路径边界。

- [ ] **Step 4: 验证测试通过**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/domain_store_tests.rs
  ```

  Expected: PASS.

## 任务 3：本地扫描 skills/plugins/tools/apps

**Files:**
- Create: `src-tauri/src/scanner/mod.rs`
- Create: `src-tauri/tests/scanner_tests.rs`
- Create: `src-tauri/tests/fixtures.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写扫描失败测试**

  Create tests named:

  ```rust
  #[test]
  fn scans_user_skill_front_matter() {}

  #[test]
  fn scans_skill_without_front_matter_using_directory_fallback() {}

  #[test]
  fn plugin_skill_id_uses_plugin_name_prefix() {}

  #[test]
  fn bad_skill_file_does_not_abort_global_scan() {}

  #[test]
  fn scans_plugin_tool_and_app_catalog_entries() {}
  ```

  Fixture creates temporary directories under `.tmp/test-fixtures/scanner-*`, including:

  - `skills/brainstorming/SKILL.md`
  - `plugins/cache/superpowers/skills/test-skill/SKILL.md`
  - a malformed UTF-8 or oversized file represented as a scanner warning
  - lightweight JSON manifests for plugins/tools/apps

- [ ] **Step 2: 运行测试确认失败**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/scanner_tests.rs
  ```

  Expected: FAIL because scanner API is missing.

- [ ] **Step 3: 实现扫描器**

  Implement:

  - `ScanRoots` with read-only roots for `.codex` and `.agents/skills` plus fixture roots.
  - `scan_all(roots) -> ScanReport`.
  - `scan_skills`, `scan_plugin_skills`, `scan_plugins`, `scan_tools`, `scan_apps`.
  - Front matter parser for `name` and `description`.
  - Markdown fallback using first heading and first paragraph.
  - Per-file warning collection so bad files never stop the whole scan.
  - Plugin skill id format: `pluginName:skillName`.

  Add Chinese comments around read-only root handling and bad-file isolation.

- [ ] **Step 4: 验证扫描测试通过**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/scanner_tests.rs
  ```

  Expected: PASS.

## 任务 4：使用统计解析与缓存

**Files:**
- Create: `src-tauri/src/stats/mod.rs`
- Create: `src-tauri/tests/stats_tests.rs`
- Modify: `src-tauri/src/domain/mod.rs`
- Modify: `src-tauri/src/store/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 写统计失败测试**

  Create tests named:

  ```rust
  #[test]
  fn counts_explicit_user_skill_mentions_repeatedly() {}

  #[test]
  fn counts_plugin_skill_mentions_repeatedly() {}

  #[test]
  fn counts_assistant_skill_file_reads() {}

  #[test]
  fn ignores_system_developer_tool_and_function_noise() {}

  #[test]
  fn unknown_stats_remain_unknown_when_cache_missing() {}
  ```

  Fixture conversation JSONL contains user, assistant, system, developer, tool output and function output records.

- [ ] **Step 2: 运行测试确认失败**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/stats_tests.rs
  ```

  Expected: FAIL because stats parser is missing.

- [ ] **Step 3: 实现统计模块**

  Implement:

  - `refresh_usage_stats(roots, ability_index) -> StatsReport`。
  - User message regex for `$skill` and `$plugin:skill`，同一消息重复出现重复计数。
  - Assistant read detection for actual `SKILL.md` reads in relevant records.
  - Role exclusion for `system`、`developer`、`tool`、`function`。
  - `data/stats-cache.json` read/write through project store path resolver.
  - Formatting helper `format_usage(None) -> "未统计"` and `format_usage(Some(n)) -> "{n} 使用"`。

  Add Chinese comments around noise exclusion and repeated counting.

- [ ] **Step 4: 验证统计测试通过**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/stats_tests.rs
  ```

  Expected: PASS.

## 任务 5：Tauri commands 与持久化 API

**Files:**
- Create: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/tests/command_tests.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/capabilities/default.json`

- [ ] **Step 1: 写 command 失败测试**

  Create tests named:

  ```rust
  #[tokio::test]
  async fn list_abilities_returns_merged_data() {}

  #[tokio::test]
  async fn update_user_data_persists_without_overwriting_ai_data() {}

  #[tokio::test]
  async fn refresh_scan_keeps_existing_user_data() {}

  #[tokio::test]
  async fn refresh_stats_preserves_unknown_for_unseen_abilities() {}
  ```

- [ ] **Step 2: 运行测试确认失败**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/command_tests.rs
  ```

  Expected: FAIL because command handlers are missing.

- [ ] **Step 3: 实现 commands**

  Implement Tauri commands:

  - `list_abilities()`
  - `refresh_scan()`
  - `refresh_stats()`
  - `get_ability(id)`
  - `update_user_data(id, patch)`
  - `copy_call_template(id)`

  Use `Arc<RwLock<AppState>>` so scan/stat refresh has a clear concurrency boundary. Add Chinese comments around write locks and UI command error mapping.

- [ ] **Step 4: 验证 command 测试通过**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/command_tests.rs
  npm run test:rust
  ```

  Expected: PASS.

## 任务 6：主界面实现第二版视觉与交互

**Files:**
- Modify: `src/App.tsx`
- Create: `src/api/atlasApi.ts`
- Create: `src/api/tauriBridge.ts`
- Create: `src/components/AbilityList.tsx`
- Create: `src/components/AbilityToolbar.tsx`
- Create: `src/components/BottomPanel.tsx`
- Create: `src/components/EditableMetadata.tsx`
- Create: `src/components/FilterBar.tsx`
- Create: `src/components/MetricCards.tsx`
- Create: `src/components/SearchBox.tsx`
- Create: `src/lib/abilityFilters.ts`
- Create: `src/lib/abilityFormatting.ts`
- Create: `src/test/fixtures.ts`
- Create: `src/test/setup.ts`
- Modify: `src/styles/atlas.css`

- [ ] **Step 1: 写前端失败测试**

  Add Vitest tests:

  ```typescript
  it("filters abilities by query, kind, favorite, hidden and tag", () => {});
  it("sorts usage counts without treating unknown as zero", () => {});
  it("renders unknown usage as 未统计", () => {});
  it("copies the user custom template before AI template", async () => {});
  it("expands low-frequency actions behind the 更多 button", async () => {});
  ```

- [ ] **Step 2: 运行测试确认失败**

  Run:

  ```powershell
  npm run test:frontend
  ```

  Expected: FAIL because components and helpers are missing.

- [ ] **Step 3: 实现主界面**

  Implement:

  - Layout based on `design-demos/Codex Atlas 第二版界面设计.html`:
    - 默认主窗口宽度约 680px。
    - 浅色文档工具风格。
    - 列表列：能力、使用、语言。
    - 底部详情区只放高频动作。
  - Search, kind segmented control, tag filters, favorite/hidden filters, sort menu.
  - Favorite toggle, hidden toggle, note/tags editing, copy call template.
  - Low-frequency actions behind `更多`.
  - Text never relies on viewport-based font scaling.

- [ ] **Step 4: 验证前端测试和构建**

  Run:

  ```powershell
  npm run test:frontend
  npm run typecheck
  npm run build
  ```

  Expected: PASS.

## 任务 7：Skill 详情独立窗口与安全文件阅读

**Files:**
- Create: `src-tauri/src/skilldoc/mod.rs`
- Create: `src-tauri/tests/skilldoc_tests.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/components/DetailWindow.tsx`
- Create: `src/lib/language.ts`
- Modify: `src/App.tsx`
- Modify: `src/styles/atlas.css`
- Create: `scripts/smoke-skill-detail.ps1`

- [ ] **Step 1: 写详情与路径边界失败测试**

  Rust tests:

  ```rust
  #[test]
  fn lists_only_readable_files_inside_skill_root() {}

  #[test]
  fn rejects_relative_path_escape() {}

  #[test]
  fn skips_dependency_build_binary_and_oversized_files() {}

  #[test]
  fn read_file_returns_markdown_text_code_or_config_content() {}
  ```

  Frontend tests:

  ```typescript
  it("opens detail reader for a skill and lists files", async () => {});
  it("shows the selected file content on the right", async () => {});
  it("does not show translation controls for Chinese content", async () => {});
  ```

- [ ] **Step 2: 运行测试确认失败**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/skilldoc_tests.rs
  npm run test:frontend
  ```

  Expected: FAIL because skill document APIs and UI are missing.

- [ ] **Step 3: 实现详情窗口和文件阅读**

  Implement:

  - Tauri commands:
    - `open_skill_detail_window(id)`
    - `list_skill_files(id)`
    - `read_skill_file(id, relative_path)`
  - Secure canonical path check: resolved path must start with skill root.
  - Limits:
    - max depth: 4
    - max files: 200
    - max single file size: 512 KiB
  - Skip: `node_modules`, `target`, `.git`, `.cache`, binary extensions.
  - React detail window layout matching design稿：左侧文件树，右侧 reader，右侧 meta。

  Add Chinese comments around canonical path verification and file-skip policy.

- [ ] **Step 4: 验证详情测试与 smoke**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/skilldoc_tests.rs
  npm run test:frontend
  npm run smoke:skill-detail
  ```

  Expected: PASS.

## 任务 8：手动翻译、缓存与 `codex exec --ephemeral`

**Files:**
- Create: `src-tauri/src/translate/mod.rs`
- Create: `src-tauri/tests/translate_tests.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/components/DetailWindow.tsx`
- Modify: `src/lib/language.ts`
- Create: `scripts/smoke-translation-cache.ps1`

- [ ] **Step 1: 写翻译失败测试**

  Rust tests:

  ```rust
  #[test]
  fn cache_key_uses_skill_id_relative_path_and_content_hash() {}

  #[test]
  fn cache_hit_returns_chinese_translation_without_running_command() {}

  #[test]
  fn content_hash_change_invalidates_old_cache() {}

  #[test]
  fn chinese_content_hides_translation_action() {}

  #[test]
  fn codex_exec_uses_project_tmp_and_timeout() {}
  ```

  Test command execution through an injected fake runner so tests do not call real Codex.

- [ ] **Step 2: 运行测试确认失败**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/translate_tests.rs
  npm run test:frontend
  ```

  Expected: FAIL because translate module is missing.

- [ ] **Step 3: 实现翻译**

  Implement:

  - `translation-cache.json` under project `data/`。
  - Cache key: `skillID + relativePath + contentHash`。
  - Language heuristic that treats Chinese content as non-translatable.
  - Commands:
    - `get_translation_state(id, relative_path)`
    - `translate_skill_file(id, relative_path)`
  - External process:

    ```text
    codex exec --ephemeral
    ```

    It writes prompt/input/output temp files inside `.tmp`, sets `TEMP` and `TMP` to project `.tmp`, uses timeout, and returns structured errors to UI.

  Add Chinese comments around cache invalidation, external command boundaries, and no-batch policy.

- [ ] **Step 4: 验证翻译测试与 smoke**

  Run:

  ```powershell
  npm run test:rust -- src-tauri/tests/translate_tests.rs
  npm run test:frontend
  npm run smoke:translation-cache
  ```

  Expected: PASS.

## 任务 9：真实扫描 smoke、GUI smoke、打包脚本

**Files:**
- Create: `scripts/smoke-scan.ps1`
- Create: `scripts/smoke-gui.ps1`
- Create: `playwright.config.ts`
- Create: `src/smoke/AppSmoke.test.ts`
- Modify: `package.json`
- Modify: `src-tauri/tauri.conf.json`
- Modify: `dist/.gitkeep`

- [ ] **Step 1: 写 smoke 失败脚本**

  `scripts/smoke-scan.ps1` checks that real Codex roots can be scanned and at least one known skill-like entry is returned when available.

  `scripts/smoke-gui.ps1` runs a Playwright smoke against Vite preview with mocked Tauri API data:

  ```typescript
  test("main window renders v2 atlas shell without overlap", async ({ page }) => {});
  test("skill detail reader can open and select a file", async ({ page }) => {});
  ```

- [ ] **Step 2: 运行 smoke 确认失败**

  Run:

  ```powershell
  npm run smoke:scan
  npm run smoke:gui
  ```

  Expected: FAIL because scripts or smoke harness are incomplete.

- [ ] **Step 3: 实现 smoke 和打包配置**

  Implement:

  - `smoke-scan.ps1` invokes a Rust test binary or command helper using project env.
  - `smoke-gui.ps1` starts `npm run dev`, waits for local URL, runs Playwright tests, and shuts down the server.
  - Tauri build config emits Windows exe and keeps package metadata correct.
  - `dist/说明.md` explains which generated artifacts are expected after build and is created by packaging step.

- [ ] **Step 4: 验证 build 与 smoke**

  Run:

  ```powershell
  npm run smoke:scan
  npm run smoke:gui
  npm run tauri:build
  ```

  Expected: PASS and Windows executable exists under `src-tauri/target` or configured bundle output.

## 任务 10：README 与中文文档

**Files:**
- Modify: `README.md`
- Create: `docs/使用说明.md`
- Create: `docs/方案设计.md`
- Create: `docs/开发记录.md`

- [ ] **Step 1: 写文档检查失败脚本**

  Add checks to `scripts/验证项目骨架.ps1`:

  ```powershell
  $docFiles = @("README.md", "docs\使用说明.md", "docs\方案设计.md", "docs\开发记录.md")
  foreach ($doc in $docFiles) {
    if (-not (Test-Path $doc)) {
      throw "缺少中文文档: $doc"
    }
    $content = Get-Content $doc -Raw
    if ($content.Length -lt 500) {
      throw "中文文档内容过少: $doc"
    }
  }
  ```

- [ ] **Step 2: 运行文档检查确认失败**

  Run:

  ```powershell
  powershell -ExecutionPolicy Bypass -File scripts/验证项目骨架.ps1
  ```

  Expected: FAIL because new docs are missing or too short.

- [ ] **Step 3: 更新文档**

  Document:

  - README: project purpose, quick start, env setup, commands, data safety.
  - 使用说明: main UI, filters, user data, detail window, translation cache, smoke commands.
  - 方案设计: architecture, data layers, scanner, stats, secure file reading, translation external command, performance decisions.
  - 开发记录: task commits, tests run, reviewer findings, final known limitations.

- [ ] **Step 4: 验证文档检查通过**

  Run:

  ```powershell
  powershell -ExecutionPolicy Bypass -File scripts/验证项目骨架.ps1
  ```

  Expected: PASS.

## 任务 11：最终整体审查与完整验证

**Files:**
- Modify only files required by final reviewer fixes.

- [ ] **Step 1: 派 final reviewer 子 agent**

  Prompt includes:

  - Full goal text.
  - This plan path.
  - `git diff origin/dev..HEAD` or task commit range.
  - Requirement to classify Critical / Important / Minor.

- [ ] **Step 2: 修复 Critical / Important 并复审**

  Main agent dispatches a fix implementer if review finds Critical or Important issues, then re-runs final reviewer.

- [ ] **Step 3: 运行完整验证**

  Run:

  ```powershell
  npm run test:rust
  npm run test:frontend
  npm run typecheck
  npm run build
  npm run tauri:build
  npm run smoke:scan
  npm run smoke:skill-detail
  npm run smoke:translation-cache
  npm run smoke:gui
  ```

  Expected: every command exits 0.

- [ ] **Step 4: 最终提交并推送**

  Run:

  ```powershell
  git status -sb
  git diff
  git diff --cached
  git push origin dev
  ```

  Expected: push succeeds to `origin/dev`.

## 每个开发任务的主 agent 审查流程

For tasks 1 through 10:

1. Spawn a fresh implementer sub agent with the exact task text.
2. Implementer must follow TDD and report red/green evidence.
3. Main agent checks `git diff` and relevant files.
4. Spawn spec reviewer sub agent with exact task requirements and implementer report.
5. If spec reviewer finds issues, spawn or resume implementer to fix, then repeat spec review.
6. Spawn code quality reviewer sub agent only after spec review passes.
7. If reviewer finds Critical or Important issues, spawn or resume implementer to fix, then repeat quality review.
8. Main agent runs task verification commands freshly.
9. Main agent commits only the reviewed task files with a detailed Chinese commit message.

## 完成标准

- `dev` 分支包含 Rust + Tauri v2 可运行项目。
- 主窗口视觉接近 `design-demos/Codex Atlas 第二版界面设计.html`，浅色、更宽、不卡文本。
- 核心模型、扫描、统计、主界面、详情窗口、翻译缓存、打包、文档均通过测试。
- 真实 Codex 数据扫描 smoke、Skill 详情 smoke、翻译缓存 smoke、GUI smoke 均通过。
- final reviewer 没有未修复的 Critical / Important。
- 代码和文档已提交并推送到 `origin/dev`。
