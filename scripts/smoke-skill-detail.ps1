$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $ProjectRoot

try {
  # Skill 详情 smoke 只覆盖本任务边界：安全文件阅读的 Rust 测试和前端详情阅读器测试。
  npm run test:rust -- --test skilldoc_tests
  npm run test:frontend
} finally {
  Pop-Location
}
