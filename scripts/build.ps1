$ErrorActionPreference = "Stop"

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root

# 所有 Go 缓存都固定在项目内，避免构建过程写入用户目录。
$env:GOMODCACHE = Join-Path $root ".cache\gomod"
$env:GOCACHE = Join-Path $root ".cache\gobuild"
$env:GOPATH = Join-Path $root ".cache\gopath"
$env:TMP = Join-Path $root ".tmp\build"
$env:TEMP = $env:TMP

$dist = Join-Path $root "dist"
$distData = Join-Path $dist "data"
New-Item -ItemType Directory -Force -Path $env:GOMODCACHE, $env:GOCACHE, $env:GOPATH, $env:TMP, $dist, $distData | Out-Null
Get-ChildItem -LiteralPath $distData -File -ErrorAction SilentlyContinue | Remove-Item -Force

# walk 需要 Common Controls v6 manifest。sidecar manifest 必须和 exe 同名同目录。
Copy-Item -Force (Join-Path $root "assets\codex-atlas.exe.manifest") (Join-Path $root "dist\codex-atlas.exe.manifest")

go test ./...
go build -ldflags="-H=windowsgui" -o (Join-Path $root "dist\codex-atlas.exe") .\cmd\codex-atlas

Copy-Item -Force (Join-Path $root "docs\使用说明.md") (Join-Path $root "dist\使用说明.md")
foreach ($name in @("last-scan.json", "stats-cache.json", "translation-cache.json")) {
  $sourceData = Join-Path $root "data\$name"
  if (Test-Path $sourceData) {
    Copy-Item -Force $sourceData (Join-Path $distData $name)
  }
}
$zip = Join-Path $root "dist\codex-atlas-portable.zip"
if (Test-Path $zip) {
  Remove-Item -Force $zip
}
$staleExeBackup = Join-Path $root "dist\codex-atlas.exe~"
if (Test-Path $staleExeBackup) {
  Remove-Item -Force $staleExeBackup
}
Compress-Archive -Path `
  (Join-Path $root "dist\codex-atlas.exe"), `
  (Join-Path $root "dist\codex-atlas.exe.manifest"), `
  (Join-Path $root "dist\使用说明.md"), `
  $distData `
  -DestinationPath $zip

Write-Output "Built dist\codex-atlas.exe"
Write-Output "Built dist\codex-atlas-portable.zip"
