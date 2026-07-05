# Codex Atlas

Codex Atlas 是一个 Windows 个人自用的 Codex 能力字典侧边栏。它只读本机 Codex 数据目录，扫描 `skills`、`plugins`、`tools` 和 `apps`，并把它们整理成可搜索、可收藏、可备注、可复制调用模板的桌面小工具。

## 运行

直接运行：

```powershell
dist\codex-atlas.exe
```

注意：`dist\codex-atlas.exe.manifest` 必须和 `codex-atlas.exe` 放在同一个目录，这是 Windows 原生控件正常显示所需的 manifest。

## 构建

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\build.ps1
```

构建脚本会把 Go 缓存、临时目录和产物都固定在当前项目内。
如果当前项目存在 `data/translation-cache.json`，构建脚本会同步到 `dist/data/translation-cache.json`，并随 portable zip 一起打包；不存在时会自动跳过。

## 已实现

- 扫描本机 Codex skills、plugins、tools、apps。
- Skills 主视图，Plugins/Tools/Apps 辅助视图。
- Claude Code 风格短侧边栏，支持默认、`更多`、`编辑` 三种状态。
- 搜索、快速筛选、排序、收藏、隐藏、备注、手动标签、别名、自定义调用模板。
- 原生轻量能力列表、底部详情抽屉、完整详情弹窗、悬停状态栏说明、复制调用模板、打开文件、打开目录、复制路径。
- 使用统计：真实使用次数、最近使用时间，并缓存到当前项目 `data/stats-cache.json`。
- 手动刷新能力清单、统计、全部刷新。
- AI 离线整理入口，通过本机 `codex exec --ephemeral` 整理当前可见列表，结果只写入 AI 数据层。
- 全局快捷键 `Ctrl+Alt+Space` 显示/隐藏窗口。

## 第二版功能

- 默认主窗口加宽，Skills 列表和详情入口在常用屏幕上更容易阅读。
- Skills 支持独立详情弹窗：左侧浏览该 Skill 文件夹内的 Markdown / 文本文件，右侧查看当前文件内容。
- 详情弹窗支持原文 / 译文切换。中文文件默认不需要翻译；英文或其他语言文件可以手动点击翻译。
- 翻译只在用户手动触发时调用本机 `codex exec --ephemeral`，不会自动批量翻译整个 skill 仓库。
- 翻译结果缓存到当前项目 `data/translation-cache.json`。缓存命中时默认显示中文译文，并保留切回原文的能力。
- 便携构建会把 `data/translation-cache.json` 同步到 `dist/data/`，方便把已翻译内容随包带走。

## 验证

最近一次验证：

```powershell
go test ./...
.\.tmp\codex-atlas-console.exe --smoke-scan
```

当前第二版验证结果：`abilities=120`，`skills=87`。GUI smoke 已确认主窗口和独立 Skill 详情窗口可以打开，进程响应正常，工作集约 `27.4MB`，CPU 时间约 `0.25s`，列表显示 `N 使用`。

详细说明见 [docs/使用说明.md](docs/使用说明.md)。
