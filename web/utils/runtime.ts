export function isPreviewRuntime() {
  return !("__TAURI_INTERNALS__" in window);
}
