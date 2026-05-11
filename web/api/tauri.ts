import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";

export function invokeCommand<T>(command: string, args?: Record<string, unknown>) {
  return invoke<T>(command, args);
}

export function openDialog(options: Parameters<typeof open>[0]) {
  return open(options);
}

export function saveDialog(options: Parameters<typeof save>[0]) {
  return save(options);
}

export function getAppWindow() {
  return getCurrentWindow();
}
