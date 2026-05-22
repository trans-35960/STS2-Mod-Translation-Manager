import React from "react";
import { invokeCommand, openDialog } from "../api/tauri";
import type { ActionCommand, CommandArgs } from "../api/tauri";
import { DEFAULT_UI_SETTINGS } from "../constants";
import { previewGameLogs } from "../previewData";
import type { DeletedMod, GameLog, UiSettings } from "../types";
import { formatCommandError } from "../utils/logging";
import { isPreviewRuntime } from "../utils/runtime";

type RunAction = (command: ActionCommand, args?: CommandArgs[ActionCommand]) => Promise<unknown>;

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
      request: {
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
      },
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
      const result = await invokeCommand("read_game_logs");
      setGameLogs(result);
      if (showToast) {
        const existing = result.filter((log) => log.exists).length;
        appendLog(`게임 로그 확인: ${existing}개 파일 발견`);
      }
    } catch (error) {
      appendLog(formatCommandError("read_game_logs", undefined, error));
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

  async function clearCurrentRuns() {
    const confirmed = window.confirm(
      [
        "진행 중 런(current_run.save)을 정리할까요?",
        "",
        "현재 진행 중인 전투/런은 게임에서 이어할 수 없게 됩니다.",
        "앱이 먼저 백업한 뒤 로컬 세이브와 Steam remote 위치의 current_run.save/current_run.save.backup을 제거합니다.",
        "Steam Cloud가 다시 내려받지 않도록 remotecache.vdf의 current_run 항목도 함께 정리합니다.",
        "Steam이 켜져 있어도 먼저 시도합니다. 파일이 잠겨 실패하면 그때 종료 안내를 표시합니다.",
        "정리 후 Steam에 '동기화 불가'가 남을 수 있습니다. 그때는 게임을 한 번 정상 종료하거나 Steam을 재시작한 뒤, 충돌 창이 뜨면 로컬 파일을 선택하세요.",
        "",
        "세이브 슬롯 자체는 삭제하지 않습니다. 계속할까요?",
      ].join("\n"),
    );
    if (!confirmed) {
      return;
    }
    appendLog("진행 중 런 정리를 시작합니다. Steam Cloud 캐시까지 확인합니다.");
    await runAction("clear_current_runs");
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
    clearCurrentRuns,
    restoreDeletedMod,
    emptyDeletedMods,
  };
}
