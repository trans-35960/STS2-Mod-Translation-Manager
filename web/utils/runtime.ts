import { isTauri } from "@tauri-apps/api/core";

type TauriWindow = Window & {
  __TAURI_INTERNALS__?: {
    metadata?: unknown;
  };
};

export function isPreviewRuntime() {
  const tauriInternals = (window as TauriWindow).__TAURI_INTERNALS__;
  return !isTauri() || !tauriInternals?.metadata;
}
