import React from "react";
import { invokeCommand } from "../api/tauri";
import type { ActionCommand, CommandArgs } from "../api/tauri";
import type { Dashboard, Page, UiSettings } from "../types";
import { formatCommandError } from "../utils/logging";
import { isPreviewRuntime } from "../utils/runtime";

export function useAppActions({
  appendLog,
  contentRef,
  page,
  selectedPreset,
  setDashboard,
  setSettingsDraft,
  setSelectedPreset,
  setBusy,
}: {
  appendLog: (message: string) => void;
  contentRef: React.RefObject<HTMLElement | null>;
  page: Page;
  selectedPreset: string;
  setDashboard: React.Dispatch<React.SetStateAction<Dashboard | null>>;
  setSettingsDraft: React.Dispatch<React.SetStateAction<UiSettings | null>>;
  setSelectedPreset: React.Dispatch<React.SetStateAction<string>>;
  setBusy: React.Dispatch<React.SetStateAction<string | null>>;
}) {
  async function runAction(command: ActionCommand, args?: CommandArgs[ActionCommand]) {
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
      const result = await invokeCommand(command, args);
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
      appendLog(formatCommandError(command, args, error));
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
      appendLog(formatCommandError("open_path", { path }, error));
    }
  }

  return {
    runAction,
    openPath,
  };
}
