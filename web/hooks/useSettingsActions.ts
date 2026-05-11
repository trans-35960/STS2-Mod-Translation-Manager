import React from "react";
import { invokeCommand, openDialog } from "../api/tauri";
import { DEFAULT_UI_SETTINGS } from "../constants";
import { previewGameLogs } from "../previewData";
import type { DeletedMod, GameLog, UiSettings } from "../types";
import { isPreviewRuntime } from "../utils/runtime";

type RunAction = (command: string, args?: Record<string, unknown>) => Promise<unknown>;

export function useSettingsActions({
  appendLog,
  runAction,
  settingsDraft,
  setSettingsDraft,
}: {
  appendLog: (message: string) => void;
  runAction: RunAction;
  settingsDraft: UiSettings | null;
  setSettingsDraft: React.Dispatch<React.SetStateAction<UiSettings | null>>;
}) {
  const [gameLogs, setGameLogs] = React.useState<GameLog[]>([]);
  const [gameLogsLoading, setGameLogsLoading] = React.useState(false);

  async function saveSettings() {
    if (!settingsDraft) {
      return;
    }
    await runAction("save_settings", {
      translationWorkDir: settingsDraft.translation_work_dir,
      targetLanguage: settingsDraft.target_language,
      gameExePath: settingsDraft.game_exe_path,
      gameLogPath: settingsDraft.game_log_path,
      saveDir: settingsDraft.save_dir,
      saveBackupDir: settingsDraft.save_backup_dir,
      saveBackupRetentionDays: settingsDraft.save_backup_retention_days,
      saveBackupMaxEntries: settingsDraft.save_backup_max_entries,
      deletedRetentionDays: settingsDraft.deleted_retention_days,
      modViewMode: settingsDraft.mod_view_mode,
    });
  }

  async function loadGameLogs(showToast = true) {
    setGameLogsLoading(true);
    try {
      if (isPreviewRuntime()) {
        setGameLogs(previewGameLogs);
        if (showToast) {
          appendLog("Preview action: read game logs");
        }
        return;
      }
      const result = await invokeCommand<GameLog[]>("read_game_logs");
      setGameLogs(result);
      if (showToast) {
        const existing = result.filter((log) => log.exists).length;
        appendLog(`게임 로그 확인: ${existing}개 파일 발견`);
      }
    } catch (error) {
      appendLog(String(error));
    } finally {
      setGameLogsLoading(false);
    }
  }

  async function chooseTranslationWorkDir() {
    if (isPreviewRuntime()) {
      appendLog("Preview action: choose translation work directory");
      return;
    }
    const selected = await openDialog({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setSettingsDraft((current) => ({
        ...(current ?? DEFAULT_UI_SETTINGS),
        translation_work_dir: selected,
      }));
    }
  }

  async function chooseGameExePath() {
    if (isPreviewRuntime()) {
      appendLog("Preview action: choose game executable");
      return;
    }
    const selected = await openDialog({
      directory: false,
      multiple: false,
      filters: [{ name: "Executable", extensions: ["exe"] }],
    });
    if (typeof selected === "string") {
      setSettingsDraft((current) => ({
        ...(current ?? DEFAULT_UI_SETTINGS),
        game_exe_path: selected,
      }));
    }
  }

  async function chooseGameLogPath() {
    if (isPreviewRuntime()) {
      appendLog("Preview action: choose game log file");
      return;
    }
    const selected = await openDialog({
      directory: false,
      multiple: false,
      filters: [{ name: "Log", extensions: ["log", "txt"] }],
    });
    if (typeof selected === "string") {
      setSettingsDraft((current) => ({
        ...(current ?? DEFAULT_UI_SETTINGS),
        game_log_path: selected,
      }));
    }
  }

  async function chooseSaveDir() {
    if (isPreviewRuntime()) {
      appendLog("Preview action: choose save directory");
      return;
    }
    const selected = await openDialog({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setSettingsDraft((current) => ({
        ...(current ?? DEFAULT_UI_SETTINGS),
        save_dir: selected,
      }));
    }
  }

  async function chooseSaveBackupDir() {
    if (isPreviewRuntime()) {
      appendLog("Preview action: choose save backup directory");
      return;
    }
    const selected = await openDialog({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setSettingsDraft((current) => ({
        ...(current ?? DEFAULT_UI_SETTINGS),
        save_backup_dir: selected,
      }));
    }
  }

  async function repairInstallations() {
    await runAction("repair_mod_installations");
  }

  async function restoreDeletedMod(item: DeletedMod) {
    await runAction("restore_deleted_mod", { id: item.id });
  }

  async function emptyDeletedMods() {
    const confirmed = window.confirm("최근 삭제 항목을 비울까요?\n\n비운 항목은 설정에서 복원할 수 없습니다.");
    if (!confirmed) {
      return;
    }
    await runAction("empty_deleted_mods");
  }

  return {
    gameLogs,
    gameLogsLoading,
    saveSettings,
    loadGameLogs,
    chooseTranslationWorkDir,
    chooseGameExePath,
    chooseGameLogPath,
    chooseSaveDir,
    chooseSaveBackupDir,
    repairInstallations,
    restoreDeletedMod,
    emptyDeletedMods,
  };
}
