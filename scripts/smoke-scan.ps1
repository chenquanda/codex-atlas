$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $ProjectRoot

$MaxProbeDepth = 8
$MaxProbeDirectories = 2048

function Add-UniquePath {
  param(
    [System.Collections.Generic.List[string]] $Paths,
    [string] $Path
  )

  if ([string]::IsNullOrWhiteSpace($Path)) {
    return
  }

  try {
    $resolved = (Resolve-Path -LiteralPath $Path -ErrorAction Stop).Path
  } catch {
    return
  }

  if (-not $Paths.Contains($resolved)) {
    $Paths.Add($resolved)
  }
}

function Test-RealDirectory {
  param([string] $Path)

  try {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
  } catch {
    return $false
  }

  return $item.PSIsContainer -and -not (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Test-RealFile {
  param([string] $Path)

  try {
    $item = Get-Item -LiteralPath $Path -Force -ErrorAction Stop
  } catch {
    return $false
  }

  return -not $item.PSIsContainer -and -not (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0)
}

function Get-RealChildDirectories {
  param([string] $Directory)

  Get-ChildItem -LiteralPath $Directory -Directory -Force -ErrorAction SilentlyContinue |
    Where-Object { -not (($_.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) } |
    Sort-Object FullName
}

function Test-ContainsSkillFileBelow {
  param([string] $Root)

  if (-not (Test-RealDirectory $Root)) {
    return $false
  }

  $queue = [System.Collections.Generic.Queue[object]]::new()
  $queue.Enqueue([pscustomobject]@{ Path = (Resolve-Path -LiteralPath $Root).Path; Depth = 0 })
  $visited = 0

  while ($queue.Count -gt 0) {
    $current = $queue.Dequeue()
    $visited += 1
    if ($visited -gt $MaxProbeDirectories) {
      Write-Warning "真实扫描预探测达到目录数量上限 $MaxProbeDirectories，提前停止: $Root"
      return $false
    }

    foreach ($child in Get-RealChildDirectories $current.Path) {
      $childDepth = [int]$current.Depth + 1
      if ($childDepth -gt $MaxProbeDepth) {
        continue
      }

      $skillFile = Join-Path $child.FullName "SKILL.md"
      if (Test-RealFile $skillFile) {
        return $true
      }

      $queue.Enqueue([pscustomobject]@{ Path = $child.FullName; Depth = $childDepth })
    }
  }

  return $false
}

function Test-HasSkillRootData {
  param([string] $Root)

  # 真实扫描 smoke 的预探测镜像 Rust scanner 的边界：显式队列、最大深度、跳过 reparse point。
  # 这里只读外部 Codex 目录，不复制、不写入，也不把外部内容落到项目缓存里。
  return Test-ContainsSkillFileBelow $Root
}

function Test-HasPluginSkillData {
  param([string] $Root)

  if (-not (Test-RealDirectory $Root)) {
    return $false
  }

  $queue = [System.Collections.Generic.Queue[object]]::new()
  $queue.Enqueue([pscustomobject]@{ Path = (Resolve-Path -LiteralPath $Root).Path; Depth = 0 })
  $visited = 0

  while ($queue.Count -gt 0) {
    $current = $queue.Dequeue()
    $visited += 1
    if ($visited -gt $MaxProbeDirectories) {
      Write-Warning "插件 smoke 预探测达到目录数量上限 $MaxProbeDirectories，提前停止: $Root"
      return $false
    }

    foreach ($child in Get-RealChildDirectories $current.Path) {
      $childDepth = [int]$current.Depth + 1
      if ($childDepth -gt $MaxProbeDepth) {
        continue
      }

      if ($child.Name -eq "skills" -and (Test-ContainsSkillFileBelow $child.FullName)) {
        return $true
      }

      $queue.Enqueue([pscustomobject]@{ Path = $child.FullName; Depth = $childDepth })
    }
  }

  return $false
}

try {
  $homes = [System.Collections.Generic.List[string]]::new()
  Add-UniquePath $homes "C:\Users\Administrator"
  Add-UniquePath $homes $env:USERPROFILE
  Add-UniquePath $homes $env:HOME

  $skillCandidates = [System.Collections.Generic.List[string]]::new()
  $pluginCandidates = [System.Collections.Generic.List[string]]::new()

  foreach ($homePath in $homes) {
    Add-UniquePath $skillCandidates (Join-Path $homePath ".codex\skills")
    Add-UniquePath $skillCandidates (Join-Path $homePath ".agents\skills")
    Add-UniquePath $pluginCandidates (Join-Path $homePath ".codex\plugins")
  }

  $skillRoots = @()
  foreach ($root in $skillCandidates) {
    if (Test-HasSkillRootData $root) {
      $skillRoots += $root
    }
  }

  $pluginRoots = @()
  foreach ($root in $pluginCandidates) {
    if (Test-HasPluginSkillData $root) {
      $pluginRoots += $root
    }
  }

  if (($skillRoots.Count + $pluginRoots.Count) -eq 0) {
    Write-Host "跳过：未找到可扫描的真实 Codex skill-like 数据。"
    Write-Host "已检查 roots:"
    foreach ($root in @($skillCandidates + $pluginCandidates)) {
      Write-Host "  $root"
    }
    exit 0
  }

  Write-Host "真实扫描 smoke 将只读以下 roots:"
  foreach ($root in $skillRoots) {
    Write-Host "  skill root: $root"
  }
  foreach ($root in $pluginRoots) {
    Write-Host "  plugin root: $root"
  }

  $oldSkillRoots = $env:CODEX_ATLAS_SMOKE_SKILL_ROOTS
  $oldPluginRoots = $env:CODEX_ATLAS_SMOKE_PLUGIN_ROOTS
  $env:CODEX_ATLAS_SMOKE_SKILL_ROOTS = [string]::Join([System.IO.Path]::PathSeparator, $skillRoots)
  $env:CODEX_ATLAS_SMOKE_PLUGIN_ROOTS = [string]::Join([System.IO.Path]::PathSeparator, $pluginRoots)

  try {
    & (Join-Path $ProjectRoot "scripts\设置开发环境.ps1") cargo test --manifest-path src-tauri/Cargo.toml --test real_scan_smoke
    if ($LASTEXITCODE -ne 0) {
      exit $LASTEXITCODE
    }
  } finally {
    $env:CODEX_ATLAS_SMOKE_SKILL_ROOTS = $oldSkillRoots
    $env:CODEX_ATLAS_SMOKE_PLUGIN_ROOTS = $oldPluginRoots
  }
} finally {
  Pop-Location
}
