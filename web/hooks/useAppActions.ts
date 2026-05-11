import React from "react";
import { invokeCommand } from "../api/tauri";
import type { ActionResult, ApplyResultState, Dashboard, Page, UiSettings } from "../types";
import { isPreviewRuntime } from "../utils/runtime";

export function useAppActions({
  appendLog,
  contentRef,
  page,
  selectedPreset,
  setDashboard,
  setSettingsDraft,
  setSelectedPreset,
  setJsonApplyResult,
  setBusy,
}: {
  appendLog: (message: string) => void;
  contentRef: React.RefObject<HTMLElement | null>;
  page: Page;
  selectedPreset: string;
  setDashboard: React.Dispatch<React.SetStateAction<Dashboard | null>>;
  setSettingsDraft: React.Dispatch<React.SetStateAction<UiSettings | null>>;
  setSelectedPreset: React.Dispatch<React.SetStateAction<string>>;
  setJsonApplyResult: React.Dispatch<React.SetStateAction<ApplyResultState | null>>;
  setBusy: React.Dispatch<React.SetStateAction<string | null>>;
}) {
  async function runAction(command: string, args?: Record<string, unknown>) {
    const restoreScrollTop = page === "mods" ? contentRef.current?.scrollTop : undefined;
    const usesGlobalBusy = command !== "toggle_mod" && command !== "toggle_mods";
    if (usesGlobalBusy) {
      setBusy(command);
    }
    try {
      if (isPreviewRuntime()) {
        appendLog(`Preview action: ${command} ${args ? JSON.stringify(args) : ""}`);
        return null;
      }
      const result = await invokeCommand<ActionResult>(command, args);
      setDashboard(result.dashboard);
      setSettingsDraft(result.dashboard.settings);
      if (restoreScrollTop !== undefined) {
        window.requestAnimationFrame(() => {
          if (contentRef.current) {
            contentRef.current.scrollTop = restoreScrollTop;
          }
        });
      }
      appendLog(result.message);
      if (result.dashboard.presets[0] && !selectedPreset) {
        setSelectedPreset(result.dashboard.presets[0].name);
      }
      return result;
    } catch (error) {
      if (command === "apply_json_translation_sheet") {
        setJsonApplyResult({
          output_path: "",
          applied_entries: 0,
          message: String(error),
          error: true,
        });
      }
      appendLog(String(error));
      return null;
    } finally {
      if (usesGlobalBusy) {
        setBusy(null);
      }
    }
  }

  async function openPath(path: string) {
    if (isPreviewRuntime()) {
      appendLog(`Preview action: open path ${path}`);
      return;
    }
    try {
      await invokeCommand("open_path", { path });
      appendLog(`경로 열기: ${path}`);
    } catch (error) {
      appendLog(String(error));
    }
  }

  return {
    runAction,
    openPath,
  };
}
