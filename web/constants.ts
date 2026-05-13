import type { UiSettings } from "./types";

export const DEFAULT_UI_SETTINGS: UiSettings = {
  target_language: "kor",
  translation_work_dir: "",
  game_exe_path: "",
  game_log_path: "",
  save_dir: "",
  save_backup_dir: "",
  save_backup_retention_days: 7,
  save_backup_max_entries: 14,
  deleted_retention_days: 30,
  mod_view_mode: "detail",
};

export const TRANSLATION_SESSION_STORAGE_KEY = "sts2.translationSession.v1";
export const MOD_VIEW_MODE_STORAGE_KEY = "sts2.modViewMode.v1";
export const MOD_TABLE_COLUMNS_STORAGE_KEY = "sts2.modTableColumns.v1";
export const EXTERNAL_MOD_PROMPT_STORAGE_KEY = "sts2.externalModPrompt.v1";

export const LOADING_STEPS = [
  "초기 설정을 확인하고 있습니다",
  "시스템 경로를 확인하고 있습니다",
  "세이브 백업 경로를 준비하고 있습니다",
  "모드 보관함을 스캔하는 중",
  "번역 작업 공간을 준비하는 중",
  "인터페이스를 정리하는 중",
];
