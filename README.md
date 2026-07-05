# Codex Atlas Rust

Codex Atlas Rust 是 Codex Atlas 的 Rust + Tauri v2 重写项目，目标是做一个 Windows 本机个人使用的 Codex 能力字典。它不是旧的 Go + Walk 项目，也不再保留旧 exe、旧构建脚本和旧 UI 代码；当前 `dev` 分支只维护 Rust 后端、Tauri v2 外壳和 React 前端。

项目第一屏就是可用的主界面：默认扫描本机 Codex 普通 Skill、插件 Skill，并在常见 Codex catalog manifest 存在时接入 plugins、tools、apps，按使用情况排序，整理收藏、隐藏、备注、标签和调用模板，并在 Skill 详情窗口里只读浏览相关文件。英文 Skill 文件可以由用户手动触发翻译，译文按文件内容缓存。

## 快速开始

先通过项目环境脚本安装 Node 依赖，避免 npm cache 写到系统用户目录：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1 npm install
```

也可以单独检查开发环境脚本是否可运行。注意 PowerShell 子进程不会把环境变量持久写回父 shell；真正执行 Rust/Tauri/npm 命令时，请使用已经包装好的 npm scripts，或把命令追加到环境脚本后面：

```powershell
powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1
```

常用命令：

```powershell
npm run dev
npm run typecheck
npm run test:frontend
npm run test:rust
npm run smoke:scan
npm run smoke:skill-detail
npm run smoke:translation-cache
npm run smoke:gui
npm run build
npm run tauri:dev
npm run tauri:build
powershell -ExecutionPolicy Bypass -File scripts/验证项目骨架.ps1
```

`tauri:dev` 和 `tauri:build` 必须通过 `scripts/设置开发环境.ps1` 进入工具链环境。Rust / Tauri 工具链来自 `F:\workspace\dev-tools`，构建产物默认在 `.cache\cargo-target`，前端构建产物在 `dist`。

## 功能概览

- 扫描：默认只读扫描 `.codex\skills`、`.agents\skills` 和 `.codex\plugins` 下的普通 Skill 与插件 Skill；同时发现已存在的 `.codex\plugins\plugins.json`、`.codex\plugins.json`、`.codex\tools\tools.json`、`.codex\tools.json`、`.codex\apps\apps.json`、`.codex\apps.json`，解析为 Plugin/Tool/App catalog。
- 统计：解析 Codex 会话 JSONL，统计用户显式 `$skill` / `$plugin:skill`，以及助手真实读取 `SKILL.md` 的证据；未知统计显示 `未统计`。
- 主界面：支持搜索、类型筛选、收藏筛选、隐藏视图、标签筛选，以及按使用次数、名称、类型排序。
- 个人整理：支持收藏、隐藏、备注、标签、自定义调用模板，用户数据优先于扫描层和 AI 层。
- 详情窗口：仅 Skill 可打开详情，左侧列出 Skill 目录内可阅读文件，右侧只读查看内容。
- 翻译缓存：非中文文件可手动翻译；缓存命中时默认显示中文译文；中文内容隐藏翻译按钮。
- smoke/打包：包含真实扫描 smoke、Skill 详情 smoke、翻译缓存 smoke、Playwright GUI smoke 和 Tauri build 骨架验证。

## 数据安全

外部 Codex roots 只读。扫描和统计会读取用户本机的 Codex skill/plugin/session 数据，但不会写入、删除或移动 `C:\Users\Administrator\.codex`、`.agents` 或其他项目外目录。

翻译会调用外部 `codex exec --ephemeral`。本项目会把发给该命令的 prompt/input/output、stdout/stderr 和临时文件约束到项目 `.tmp`，但外部 Codex CLI 自身的认证、网络和模型行为仍由用户本机 Codex 环境负责。

项目自己的持久数据写在当前仓库内：

- `data\store.json`：扫描能力、用户备注、收藏、隐藏、标签、自定义模板等。
- `data\stats-cache.json`：完整统计刷新后的可重建统计缓存。
- `data\translation-cache.json`：按 `skillId + relativePath + contentHash` 命中的单文件译文缓存。
- `.cache`：Cargo、npm、Playwright 和构建缓存。
- `.tmp`：翻译 prompt/input/output、Playwright 结果和 smoke 临时日志。

这些目录由脚本和 `.gitignore` 约束，避免把个人整理数据或临时输出误提交。

## 当前限制

- 翻译必须由用户在详情窗口里手动触发，不会自动批量翻译全部 Skill。
- GUI smoke 使用 Playwright 和 mock Tauri IPC 验证前端交互，不会启动真实 Tauri IPC 后端。
- 真实扫描 smoke 会先探测本机 Codex 数据；如果没有可扫描的真实 Skill-like 数据，会输出跳过信息并以成功退出。
- 使用统计依赖本机 Codex sessions JSONL。统计 discovery 不完整、文件读取失败或 JSONL 损坏时，不会把未知覆盖成 `0 使用`。
- 旧 Go + Walk 实现仅可从 Git 历史或 `main` 分支追溯，当前项目实现和文档不以旧工程为准。

## 文档入口

- `docs/使用说明.md`：面向日常使用，说明主界面、详情窗口、翻译、统计和常用命令。
- `docs/方案设计.md`：面向后续开发，说明 Rust/Tauri/React 架构、扫描、统计、存储、翻译和 smoke/build 设计。
- `docs/开发记录.md`：按任务记录已完成提交、验证命令、审查发现和剩余风险。
