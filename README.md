# Codex Atlas Rust

这是 Codex Atlas 的 Rust + Tauri 重写起点。

本分支故意不保留旧 Go + Walk 源码、旧 exe、旧构建脚本和旧备选设计稿，避免后续 AI 开发时被旧实现污染上下文。旧版实现仍可在仓库历史或 `main` 分支查看；当前 `dev` 分支只保留重写所需的最小资料。

## 当前保留内容

- `docs/Rust重写交接文档.md`：Rust/Tauri 重写目标、约束、业务规则、开发节奏和验证清单。
- `design-demos/Codex Atlas 第二版界面设计.html`：第二版前端界面的唯一视觉基准。
- `README.md`：当前分支说明。

## 重写方向

- 技术栈：Rust + Tauri v2 + Web 前端。
- 目标平台：Windows。
- 产品形态：个人自用的 Codex 能力字典侧边栏。
- 主功能：扫描 skills/plugins/tools/apps、搜索筛选、使用次数统计、Skill 详情文件浏览、手动翻译与缓存。
- UI 要求：以第二版 HTML 设计稿为准，浅色调，不要回到第一版拥挤界面。

## 工具链位置

Rust / Tauri 工具链已安装在：

```text
F:\workspace\dev-tools
```

推荐新会话进入项目后先设置：

```powershell
$env:RUSTUP_HOME = "F:\workspace\dev-tools\rust\rustup"
$env:CARGO_HOME = "F:\workspace\dev-tools\rust\cargo"
$env:PATH = "F:\workspace\dev-tools\rust\cargo\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "$PWD\.cache\cargo-target"
$env:TEMP = "$PWD\.tmp"
$env:TMP = "$PWD\.tmp"
```

## 下一步

新会话应先阅读：

```text
docs/Rust重写交接文档.md
design-demos/Codex Atlas 第二版界面设计.html
```

然后先写开发计划，再使用 Subagent-Driven Development 按任务开发、测试、审查和提交。
