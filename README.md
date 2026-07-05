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

## 验证

最近一次验证：

```powershell
go test ./...
.\.tmp\codex-atlas-console.exe --smoke-scan
```

真实扫描结果：`abilities=119`，`skills=86`。GUI 启动 smoke：工作集约 `21.7MB`，4 秒 CPU 时间约 `0.14s`。

当前使用次数修复后验证结果：`abilities=120`，`skills=87`，新版统计缓存 `87` 项，非零使用次数 `39` 项。GUI 启动 smoke：工作集约 `23.1MB`，4 秒 CPU 时间约 `0.141s`，列表截图显示 `N 使用`。

详细说明见 [docs/使用说明.md](docs/使用说明.md)。
