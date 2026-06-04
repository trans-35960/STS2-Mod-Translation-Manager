import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  ActionResult,
  Dashboard,
  DroppedModPreview,
  GameLog,
  JsonApply,
  JsonCsvExport,
  JsonSheetAction,
  JsonTranslationSheet,
  JsonValidation,
  LanguageCompareValue,
  ModDeleteRequest,
  ModToggleRequest,
  NodeTranslationResult,
  SaveSettingsRequest,
  ShortJsonExport,
  TranslationPatchExport,
} from "../types";

export type CommandArgs = {
  load_dashboard: undefined;
  save_settings: { request: SaveSettingsRequest };
  save_mod_view_mode: { modViewMode: "detail" | "simple" };
  read_game_logs: undefined;
  scan_updates: undefined;
  repair_mod_installations: undefined;
  open_path: { path: string };
  preview_dropped_mods: { paths: string[] };
  import_dropped_mod: { path: string; replacePath?: string | null };
  import_dropped_mods: {
    decisions: Array<{ path: string; mode: "new" | "skip" | "replace"; replacePath?: string | null }>;
  };
  toggle_mod: { key: string; active: boolean; force?: boolean | null };
  toggle_mods: { changes: ModToggleRequest[] };
  delete_mod: { key: string; path: string };
  delete_mods: { items: ModDeleteRequest[] };
  restore_deleted_mod: { id: string };
  empty_deleted_mods: undefined;
  cleanup_orphan_caches: undefined;
  cleanup_dropped_mod_preview_cache: undefined;
  extract_translation: {
    key: string;
    outputDir?: string | null;
    resourcePath?: string | null;
    force?: boolean | null;
  };
  clear_translation_extract_cache: { key: string };
  prepare_translation_node: {
    key: string;
    resourcePath: string;
    outputDir?: string | null;
    force?: boolean | null;
  };
  save_preset: { name: string };
  apply_preset: { name: string };
  export_preset: { name: string; archivePath: string };
  import_preset_archive: { archivePath: string };
  launch_current: undefined;
  launch_vanilla: undefined;
  create_save_backup: undefined;
  clear_current_runs: undefined;
  restore_save_backup: { id: string };
  delete_save_backups: { ids: string[] };
  create_json_translation_sheet: {
    sourcePath: string;
    existingSheetPath?: string | null;
    outputPath?: string | null;
    targetLanguage?: string | null;
  };
  recalculate_json_translation_sheet: {
    sourcePath: string;
    currentSheetPath: string;
    outputPath?: string | null;
    targetLanguage?: string | null;
  };
  load_json_translation_sheet: { sheetPath: string };
  validate_json_translation_sheet: { sheetPath: string };
  validate_json_translation_sheet_data: { sheet: JsonTranslationSheet };
  save_json_translation_sheet: { sheetPath: string; sheet: JsonTranslationSheet };
  export_json_translation_csv: { outputPath: string; sheet: JsonTranslationSheet };
  export_json_translation_short_json: {
    outputPath: string;
    sheet: JsonTranslationSheet;
    onlyEmpty?: boolean | null;
    includeKeys?: string[] | null;
  };
  export_json_translation_warning_json: {
    outputPath: string;
    sheet: JsonTranslationSheet;
    includeKeys?: string[] | null;
  };
  export_json_translation_change_json: {
    outputPath: string;
    sheet: JsonTranslationSheet;
    includeKeys?: string[] | null;
  };
  import_json_translation_values: { inputPath: string; sheet: JsonTranslationSheet };
  compare_translation_language: { sheetPath: string; samplePath: string };
  apply_json_translation_sheet: {
    sheetPath: string;
    outputPath: string;
    pckTargetPath?: string | null;
  };
  export_translation_patch_mod: { sheetPath: string; outputDir: string };
};

export type CommandResult = {
  load_dashboard: Dashboard;
  save_settings: ActionResult;
  save_mod_view_mode: void;
  read_game_logs: GameLog[];
  scan_updates: ActionResult;
  repair_mod_installations: ActionResult;
  open_path: void;
  preview_dropped_mods: DroppedModPreview[];
  import_dropped_mod: ActionResult;
  import_dropped_mods: ActionResult;
  toggle_mod: ActionResult;
  toggle_mods: ActionResult;
  delete_mod: ActionResult;
  delete_mods: ActionResult;
  restore_deleted_mod: ActionResult;
  empty_deleted_mods: ActionResult;
  cleanup_orphan_caches: ActionResult;
  cleanup_dropped_mod_preview_cache: void;
  extract_translation: ActionResult;
  clear_translation_extract_cache: ActionResult;
  prepare_translation_node: NodeTranslationResult;
  save_preset: ActionResult;
  apply_preset: ActionResult;
  export_preset: ActionResult;
  import_preset_archive: ActionResult;
  launch_current: ActionResult;
  launch_vanilla: ActionResult;
  create_save_backup: ActionResult;
  clear_current_runs: ActionResult;
  restore_save_backup: ActionResult;
  delete_save_backups: ActionResult;
  create_json_translation_sheet: JsonSheetAction;
  recalculate_json_translation_sheet: JsonSheetAction;
  load_json_translation_sheet: JsonTranslationSheet;
  validate_json_translation_sheet: JsonValidation;
  validate_json_translation_sheet_data: JsonValidation;
  save_json_translation_sheet: JsonSheetAction;
  export_json_translation_csv: JsonCsvExport;
  export_json_translation_short_json: ShortJsonExport;
  export_json_translation_warning_json: ShortJsonExport;
  export_json_translation_change_json: ShortJsonExport;
  import_json_translation_values: JsonSheetAction;
  compare_translation_language: LanguageCompareValue[];
  apply_json_translation_sheet: JsonApply;
  export_translation_patch_mod: TranslationPatchExport;
};

export type CommandName = keyof CommandArgs;
export type CommandArgTuple<C extends CommandName> = CommandArgs[C] extends undefined
  ? []
  : [args: CommandArgs[C]];
export type ActionCommand = {
  [K in CommandName]: CommandResult[K] extends ActionResult ? K : never;
}[CommandName];
export type JsonToolCommand =
  | "create_json_translation_sheet"
  | "recalculate_json_translation_sheet"
  | "load_json_translation_sheet"
  | "validate_json_translation_sheet"
  | "apply_json_translation_sheet";

export function invokeCommand<C extends CommandName>(
  command: C,
  ...args: CommandArgTuple<C>
): Promise<CommandResult[C]>;
export function invokeCommand(
  command: CommandName,
  args?: CommandArgs[CommandName],
): Promise<CommandResult[CommandName]>;
export function invokeCommand(
  command: CommandName,
  args?: CommandArgs[CommandName],
): Promise<CommandResult[CommandName]> {
  return invoke<CommandResult[CommandName]>(command, args);
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
