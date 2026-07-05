export function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  const smokeWindow = window as Window & { __CODEX_ATLAS_BROWSER_SMOKE__?: boolean };

  // Playwright smoke 需要 mock Tauri IPC，但仍按普通浏览器路径打开详情 overlay。
  if (smokeWindow.__CODEX_ATLAS_BROWSER_SMOKE__) {
    return false;
  }

  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}
