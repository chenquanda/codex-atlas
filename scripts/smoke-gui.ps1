$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$TmpDir = Join-Path $ProjectRoot ".tmp"
$CacheDir = Join-Path $ProjectRoot ".cache"
New-Item -ItemType Directory -Force $TmpDir, $CacheDir | Out-Null

function Test-PortAvailable {
  param([int] $Port)

  $listener = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse("127.0.0.1"), $Port)
  try {
    $listener.Start()
    return $true
  } catch {
    return $false
  } finally {
    $listener.Stop()
  }
}

function Get-AvailablePort {
  param([int] $StartPort)

  for ($port = $StartPort; $port -lt ($StartPort + 80); $port += 1) {
    if (Test-PortAvailable $port) {
      return $port
    }
  }

  throw "无法在 127.0.0.1:$StartPort 起的 80 个端口内找到可用端口"
}

function Wait-ServerReady {
  param(
    [string] $Url,
    [System.Diagnostics.Process] $Process,
    [string] $StdoutPath,
    [string] $StderrPath
  )

  $deadline = (Get-Date).AddSeconds(45)
  while ((Get-Date) -lt $deadline) {
    if ($Process.HasExited) {
      $stdout = if (Test-Path $StdoutPath) { Get-Content -Raw $StdoutPath } else { "" }
      $stderr = if (Test-Path $StderrPath) { Get-Content -Raw $StderrPath } else { "" }
      throw "Vite dev server 提前退出，exit=$($Process.ExitCode)。`nSTDOUT:`n$stdout`nSTDERR:`n$stderr"
    }

    try {
      $response = Invoke-WebRequest -Uri $Url -UseBasicParsing -TimeoutSec 2
      if ($response.StatusCode -ge 200 -and $response.StatusCode -lt 500) {
        return
      }
    } catch {
      Start-Sleep -Milliseconds 500
    }
  }

  throw "等待 Vite dev server 超时: $Url"
}

function Stop-ProcessTree {
  param([int] $ProcessId)

  $children = Get-CimInstance Win32_Process -Filter "ParentProcessId = $ProcessId" -ErrorAction SilentlyContinue
  foreach ($child in $children) {
    Stop-ProcessTree -ProcessId ([int] $child.ProcessId)
  }

  Stop-Process -Id $ProcessId -Force -ErrorAction SilentlyContinue
}

Push-Location $ProjectRoot

$serverProcess = $null
$port = Get-AvailablePort 1420
$baseUrl = "http://127.0.0.1:$port"
$stdoutPath = Join-Path $TmpDir "smoke-gui-vite.out.log"
$stderrPath = Join-Path $TmpDir "smoke-gui-vite.err.log"
$exitCode = 0

try {
  $env:PLAYWRIGHT_BASE_URL = $baseUrl
  $env:PLAYWRIGHT_BROWSERS_PATH = Join-Path $CacheDir "ms-playwright"
  $env:npm_config_cache = Join-Path $CacheDir "npm"
  $env:TEMP = $TmpDir
  $env:TMP = $TmpDir

  Remove-Item -LiteralPath $stdoutPath, $stderrPath -Force -ErrorAction SilentlyContinue

  # 只启动本脚本自己的 Vite 进程；收尾时按进程树关闭，不碰用户已打开的 Chrome。
  $serverProcess = Start-Process -FilePath "npm.cmd" `
    -ArgumentList @("run", "dev", "--", "--host", "127.0.0.1", "--port", "$port", "--strictPort") `
    -WorkingDirectory $ProjectRoot `
    -RedirectStandardOutput $stdoutPath `
    -RedirectStandardError $stderrPath `
    -WindowStyle Hidden `
    -PassThru

  Wait-ServerReady -Url $baseUrl -Process $serverProcess -StdoutPath $stdoutPath -StderrPath $stderrPath

  # 首次 smoke 时 Chromium 下载到项目 .cache，避免写入全局用户缓存。
  npx playwright install chromium
  if ($LASTEXITCODE -ne 0) {
    $exitCode = $LASTEXITCODE
  } else {
    npx playwright test --config playwright.config.ts
    if ($LASTEXITCODE -ne 0) {
      $exitCode = $LASTEXITCODE
    }
  }
} finally {
  if ($serverProcess -ne $null -and -not $serverProcess.HasExited) {
    Stop-ProcessTree -ProcessId $serverProcess.Id
  }
  Pop-Location
}

exit $exitCode
