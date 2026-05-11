export type Page = "mods" | "translationTools" | "settings";
export type Locale = "ko" | "en";
export type ActiveFilter = "all" | "enabled" | "disabled";
export type ChangeFilter = "all" | "changed" | "new" | "updated" | "clean";
export type TranslationApplyFilter = "all" | "applied" | "notApplied";
export type DashboardStatFilter = "all" | "active" | "inactive" | "external" | "changed";
export type ModSort =
  | "name"
  | "registered"
  | "updated"
  | "modified"
  | "translationApplied"
  | "active"
  | "change"
  | "source"
  | "version";
export type EntryFilter = "all" | "new" | "empty" | "updated" | "removed" | "warning" | "conflict";

export type Dashboard = {
  paths: Paths;
  settings: UiSettings;
  stats: Stats;
  setup_issues: SetupIssue[];
  diagnostics: TroubleshootDiagnostic[];
  mods: ModRow[];
  presets: Preset[];
  translations: TranslationWorkspace[];
  deleted_mods: DeletedMod[];
  save_backups: SaveBackup[];
  tools: Tool[];
  launch: LaunchStatus;
};

export type UiSettings = {
  translation_work_dir: string;
  target_language: string;
  game_exe_path: string;
  game_log_path: string;
  save_dir: string;
  save_backup_dir: string;
  save_backup_retention_days: number;
  save_backup_max_entries: number;
  deleted_retention_days: number;
  mod_view_mode: "detail" | "simple";
};

export type Paths = {
  workspace: string;
  game: string;
  game_mods: string;
  save_dir: string;
  save_backup: string;
  vault: string;
  presets: string;
  translation_work: string;
  state: string;
  vendor: string;
  external_manager_dirs: string[];
};

export type Stats = {
  active_mods: number;
  inactive_mods: number;
  vault_mods: number;
  external_mods: number;
  presets: number;
  translations: number;
  detected_changes: number;
  vanilla_safe: boolean;
};

export type SetupIssue = {
  field: string;
  message: string;
  blocking: boolean;
};

export type TroubleshootDiagnostic = {
  id: string;
  category: string;
  severity: "good" | "info" | "warn" | "error" | string;
  title: string;
  detail: string;
  action_label: string;
  related_path: string;
  mod_key: string | null;
  can_auto_fix: boolean;
};

export type ModRow = {
  key: string;
  name: string;
  group_name: string | null;
  active: boolean;
  managed: boolean;
  external: boolean;
  source_label: string;
  kind: string;
  version_hint: string | null;
  bytes: number;
  modified_epoch: number | null;
  registered_epoch: number | null;
  updated_epoch: number | null;
  path: string;
  update_state: string;
  change_reasons: string[];
  translation_state: string;
  translation_applied: boolean;
  translation_applied_epoch: number | null;
  translation_patch_count: number;
  translation_patch_active_count: number;
  translation_patch_names: string[];
  needs_recheck: boolean;
  translation_review_required: boolean;
  safety_warnings: string[];
  extraction_hint: string;
  extraction_source_path: string;
  extraction_target: string;
  is_translation_patch: boolean;
  translation_target_id: string | null;
  translation_target_key: string | null;
  translation_target_name: string | null;
  translation_target_version: string | null;
  dependencies: ModDependency[];
  language_preview: LanguagePreview[];
  extraction_tree: ExtractionTreeNode[];
};

export type ModGroup = {
  id: string;
  name: string;
  mods: ModRow[];
  activeCount: number;
  updateCount: number;
};

export type ModDependency = {
  id: string;
  key: string | null;
  name: string;
  active: boolean;
  available: boolean;
  version_required: string | null;
  version_current: string | null;
  version_matches: boolean | null;
};

export type LanguagePreview = {
  code: string;
  label: string;
  files: number;
  keys?: number;
  sample_path: string;
};

export type ExtractionTreeNode = {
  name: string;
  path: string;
  source_path?: string;
  kind: "dir" | "file" | "language" | string;
  children: ExtractionTreeNode[];
};

export type Preset = {
  name: string;
  mod_count: number;
  mods: PresetMod[];
};

export type PresetMod = {
  key: string;
  version_hint: string | null;
  bytes: number | null;
  modified_epoch: number | null;
  file_name: string | null;
};

export type TranslationWorkspace = {
  mod_key: string;
  version_id: string;
  review_required: boolean;
  path: string;
};

export type Tool = {
  name: string;
  available: boolean;
  purpose: string;
  expected_path: string;
};

export type LaunchStatus = {
  ready: boolean;
  game_exe: string | null;
  steam_exe: string | null;
  target_label: string;
  running: boolean;
};

export type GameLog = {
  path: string;
  exists: boolean;
  modified_epoch: number | null;
  bytes: number;
  lines: string[];
};

export type DeletedMod = {
  id: string;
  key: string;
  name: string;
  original_path: string;
  backup_path: string;
  deleted_epoch: number;
  expires_epoch: number | null;
  bytes: number;
};

export type SaveBackup = {
  id: string;
  kind: "vanilla" | "modded" | string;
  kind_label: string;
  created_epoch: number;
  path: string;
  bytes: number;
};

export type ActionResult = {
  message: string;
  dashboard: Dashboard;
};

export type DroppedModPreview = {
  path: string;
  display_path: string;
  key: string;
  name: string;
  kind: string;
  version_hint: string | null;
  bytes: number;
  modified_epoch: number | null;
};

export type NodeTranslationResult = {
  message: string;
  source_path: string;
  existing_sheet_path: string;
  output_sheet_path: string;
  translated_output_path: string;
  copied_files: number;
  mod_key: string;
  mod_path: string;
  mod_name: string;
  mod_version: string;
  mod_author: string;
  mod_description: string;
  available_languages: LanguagePreview[];
};

export type TranslationProjectInfo = {
  modKey?: string;
  modPath?: string;
  modName: string;
  version: string;
  author: string;
  description: string;
  languages: LanguagePreview[];
};

export type LanguageCompareValue = {
  key: string;
  value: string;
};

export type JsonSheetAction = {
  message: string;
  report: JsonSheetReport;
  sheet: JsonTranslationSheet;
};

export type JsonSheetReport = {
  sheet_path: string;
  entries: number;
  new_entries: number;
  updated_entries: number;
  missing_entries: number;
  removed_entries: number;
};

export type JsonTranslationSheet = {
  source_path: string;
  target_language: string;
  updated_epoch: number;
  entries: JsonTranslationEntry[];
};

export type JsonTranslationEntry = {
  key: string;
  slot_id?: string | null;
  source_value: string;
  translated_value: string;
  status: "new" | "ready" | "updated" | "missing" | "removed";
};

export type JsonValidation = {
  valid: boolean;
  total_entries: number;
  missing_entries: string[];
  updated_entries: string[];
  removed_entries: string[];
  format_issues: JsonValidationIssue[];
};

export type JsonValidationIssue = {
  key: string;
  kind: string;
  message: string;
};

export type JsonApply = {
  output_path: string;
  applied_entries: number;
  language_output_path?: string;
  packed_pck_path?: string;
  installed_mod_path?: string;
};

export type TranslationPatchExport = {
  output_dir: string;
  manifest_path: string;
  pck_path: string;
  package_id: string;
  dependency_id: string;
  dependency_version: string;
  languages: string[];
  files: number;
  applied_entries: number;
};

export type PasteCandidate = {
  value: string;
  source: string;
};

export type ApplyResultState = JsonApply & {
  message: string;
  error?: boolean;
};

export type TranslationSessionState = {
  sourcePath: string;
  existingSheetPath: string;
  outputSheetPath: string;
  translatedOutputPath: string;
  pckTargetPath: string;
  sheet: JsonTranslationSheet | null;
  report: JsonSheetReport | null;
  validation: JsonValidation | null;
  applyResult: ApplyResultState | null;
  projectInfo: TranslationProjectInfo | null;
  targetLanguage: string;
  compareSamplePaths: string[];
  compareValuesByLanguage: Record<string, Record<string, string>>;
  compareViewEnabled: boolean;
};

export type JsonCsvExport = {
  output_path: string;
  rows: number;
};

export type ShortJsonExport = {
  output_path: string;
  rows: number;
};

export type LogTone = "info" | "warn" | "error";
