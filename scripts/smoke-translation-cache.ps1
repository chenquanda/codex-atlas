$ErrorActionPreference = "Stop"

$ProjectRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
Push-Location $ProjectRoot

try {
  # 翻译缓存 smoke 只验证当前文件的手动翻译与缓存命中；Rust 测试使用 fake runner，不调用真实 Codex。
  npm run test:rust -- --test translate_tests
  npm run test:frontend -- src/App.test.tsx
} finally {
  Pop-Location
}
