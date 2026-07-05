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
  "src-tauri\icons\icon.ico",
  "playwright.config.ts",
  "scripts\设置开发环境.ps1",
  "scripts\smoke-scan.ps1",
  "scripts\smoke-gui.ps1",
  "scripts\smoke-skill-detail.ps1",
  "scripts\smoke-translation-cache.ps1",
  ".cache",
  ".cache\.gitkeep",
  ".tmp",
  ".tmp\.gitkeep",
  "data",
  "data\.gitkeep",
  "dist",
  "dist\.gitkeep",
  "dist\说明.md"
)
foreach ($path in $required) {
  if (-not (Test-Path $path)) {
    throw "缺少项目骨架文件或目录: $path"
  }
}
$docFiles = @("README.md", "docs\使用说明.md", "docs\方案设计.md", "docs\开发记录.md")
foreach ($doc in $docFiles) {
  if (-not (Test-Path $doc)) { throw "缺少中文文档: $doc" }
  $content = Get-Content $doc -Raw
  if ($content.Length -lt 500) { throw "中文文档内容过少: $doc" }
}
$pkg = Get-Content "package.json" -Raw | ConvertFrom-Json
$expectedTauriDev = "powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1 tauri dev"
$expectedTauriBuild = "powershell -ExecutionPolicy Bypass -File scripts/设置开发环境.ps1 tauri build"
if ($pkg.scripts."tauri:dev" -ne $expectedTauriDev) {
  throw "package.json tauri:dev 必须通过设置开发环境脚本调用"
}
if ($pkg.scripts."tauri:build" -ne $expectedTauriBuild) {
  throw "package.json tauri:build 必须通过设置开发环境脚本调用"
}
$expectedSmokeScripts = @{
  "smoke:scan" = "powershell -ExecutionPolicy Bypass -File scripts/smoke-scan.ps1"
  "smoke:gui" = "powershell -ExecutionPolicy Bypass -File scripts/smoke-gui.ps1"
  "smoke:skill-detail" = "powershell -ExecutionPolicy Bypass -File scripts/smoke-skill-detail.ps1"
  "smoke:translation-cache" = "powershell -ExecutionPolicy Bypass -File scripts/smoke-translation-cache.ps1"
}
foreach ($scriptName in $expectedSmokeScripts.Keys) {
  if ($pkg.scripts.$scriptName -ne $expectedSmokeScripts[$scriptName]) {
    throw "package.json 缺少或错误配置 smoke 脚本: $scriptName"
  }
}
if ($pkg.scripts."test:frontend" -ne "vitest run") {
  throw "package.json test:frontend 脚本不应使用 passWithNoTests"
}
$conf = Get-Content "src-tauri\tauri.conf.json" -Raw | ConvertFrom-Json
if ($conf.identifier -ne "local.codex-atlas.rust") {
  throw "Tauri identifier 不正确"
}
$css = Get-Content "src\styles\atlas.css" -Raw
if ($css -match "font-size\s*:[^;]*vw") {
  throw "CSS font-size 不允许依赖 viewport 宽度"
}
$envScript = Get-Content "scripts\设置开发环境.ps1" -Raw
if ($envScript -notmatch "\.cache\\cargo-home") {
  throw "设置开发环境脚本必须把 CARGO_HOME 放在项目 .cache\cargo-home"
}
$schemaFiles = Get-ChildItem "src-tauri\gen\schemas" -Filter "*.json" -File -ErrorAction SilentlyContinue
foreach ($schemaFile in $schemaFiles) {
  $schemaPath = "src-tauri/gen/schemas/$($schemaFile.Name)"
  git check-ignore -q -- $schemaPath
  if ($LASTEXITCODE -ne 0) {
    throw "Tauri 生成 schema 产物必须被 .gitignore 忽略: $schemaPath"
  }
}
$gitignore = Get-Content ".gitignore" -Raw
if ($gitignore -notmatch "(?m)^src-tauri/gen/$") {
  throw ".gitignore 必须忽略 Tauri 生成 schema 目录 src-tauri/gen/"
}
$buildScript = Get-Content "src-tauri\build.rs" -Raw
if ($buildScript -match "cleanup_generated_schemas|remove_file") {
  throw "build.rs 不应删除源码树 gen/schemas 产物"
}
$capability = Get-Content "src-tauri\capabilities\default.json" -Raw | ConvertFrom-Json
if ($capability.PSObject.Properties.Name -contains '$schema') {
  throw "default capability 不应引用生成 schema"
}
Write-Host "项目骨架验证通过"
