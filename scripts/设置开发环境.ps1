param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]] $Command
)

$ErrorActionPreference = "Stop"

$projectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Set-Location $projectRoot

# RUSTUP_HOME 和 PATH 只用于只读调用已安装工具链。
$env:RUSTUP_HOME = "F:\workspace\dev-tools\rust\rustup"
$env:PATH = "F:\workspace\dev-tools\rust\cargo\bin;F:\workspace\dev-tools\bun;$env:PATH"

# Cargo 下载缓存、构建产物、npm 缓存和临时文件都放在当前项目内。
$env:CARGO_HOME = Join-Path $projectRoot ".cache\cargo-home"
$env:CARGO_TARGET_DIR = Join-Path $projectRoot ".cache\cargo-target"
$env:npm_config_cache = Join-Path $projectRoot ".cache\npm"
$env:TEMP = Join-Path $projectRoot ".tmp"
$env:TMP = Join-Path $projectRoot ".tmp"

New-Item -ItemType Directory -Force ".cache\cargo-home", ".cache\cargo-target", ".cache\npm", ".tmp" | Out-Null

if ($Command.Count -eq 0) {
  Write-Host "开发环境已设置"
  return
}

$program = $Command[0]
$arguments = @()
if ($Command.Count -gt 1) {
  $arguments = $Command[1..($Command.Count - 1)]
}

& $program @arguments
if ($LASTEXITCODE -ne $null) {
  exit $LASTEXITCODE
}
