export type Page = "mods" | "translationTools" | "settings";
export type Locale = "ko" | "en";
export type ActiveFilter = "all" | "enabled" | "disabled";
export type ChangeFilter = "all" | "changed" | "new" | "updated" | "clean";
export type TranslationApplyFilter = "all" | "applied" | "notApplied";
export type DashboardStatFilter = "all" | "active" | "inactive" | "external" | "workshop" | "changed";
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
  cache_usage: CacheUsage;
  tools: Tool[];
  launch: LaunchStatus;
  bridge: Bridge;
};

/** The in-game companion mod that applies translations from the shared folder. */
export type Bridge = {
  installed: boolean;
  active: boolean;
  installed_version: string | null;
  bundled_version: string | null;
  shared_dir: string;
  status: BridgeStatus | null;
};

export type BridgeStatus = {
  bridge_version: string;
  game_version: string | null;
  language: string;
  applied_at: string;
  applied_entries: number;
  mods: [string, number][];
  invalid_count: number;
  invalid: string[];
  missing_tables: string[];
  errors: string[];
};

export type BridgeInstall = {
  state: "installed" | "updated" | "current" | "game_running" | "missing_source";
  version: string | null;
  message: string;
};

export type BridgeApply = {
  mod_id: string;
  language: string;
  tables: number;
  unsupported_entries: number;
  unsupported_hardcoded: number;
  /** DLL strings written for the bridge to replace (DLL sheets only). */
  dll_strings: number | null;
  code_like_skipped: number;
  disabled_patch_mods: string[];
  bridge_installed: boolean;
  bridge_active: boolean;
  game_running: boolean;
};

/** In-game edits (F8 panel) and mistranslation reports for one sheet. */
export type BridgeEdits = {
  mod_id: string;
  language: string;
  items: BridgeEditItem[];
  reports: BridgeReport[];
  ack_id: string | null;
};

export type BridgeEditItem = {
  kind: "game_changed" | "conflict";
  sheet_key: string;
  table: string;
  key: string;
  source: string | null;
  sheet_value: string;
  game_value: string;
};

export type BridgeReport = {
  id: string;
  table: string;
  key: string;
  sheet_key: string | null;
  source: string | null;
  shown: string | null;
  note: string;
  pending: boolean;
};

export type CacheUsage = {
  bytes: number;
  files: number;
  dirs: number;
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

export type SaveSettingsRequest = {
  translationWorkDir: string;
  targetLanguage: string;
  gameExePath: string;
  gameLogPath: string;
  saveDir: string;
  saveBackupDir: string;
  saveBackupRetentionDays: number;
  saveBackupMaxEntries: number;
  deletedRetentionDays: number;
  modViewMode: "detail" | "simple";
};

export type ModToggleRequest = {
  key: string;
  active: boolean;
  force?: boolean | null;
};

export type ModDeleteRequest = {
  key: string;
  path: string;
};

export type Paths = {
  workspace: string;
  game: string;
  game_mods: string;
  save_dir: string;
  save_backup: string;
  disabled: string;
  presets: string;
  translation_work: string;
  state: string;
  vendor: string;
  external_manager_dirs: string[];
  workshop_dirs: string[];
};

export type Stats = {
  active_mods: number;
  inactive_mods: number;
  disabled_mods: number;
  external_mods: number;
  workshop_mods: number;
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
  manifest_id: string | null;
  group_name: string | null;
  active: boolean;
  managed: boolean;
  external: boolean;
  workshop: boolean;
  workshop_item_id: string | null;
  source_label: string;
  kind: string;
  version_hint: string | null;
  bytes: number;
  modified_epoch: number | null;
  registered_epoch: number | null;
  updated_epoch: number | null;
  path: string;
  download_state?: string | null;
  update_state: string;
  change_reasons: string[];
  previous_version: string | null;
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
  branch_issue?: BranchIssue | null;
  system_mod?: boolean;
  /** In-game (F8) edits and open reports waiting in the translation tool. */
  bridge_edits?: { edits: number; reports: number } | null;
  language_preview: LanguagePreview[];
  extraction_tree: ExtractionTreeNode[];
};

export type LeftoverText = {
  text: string;
  code_like: boolean;
};

export type LeftoverFile = {
  location: "dll" | "resource" | "patch";
  file: string;
  texts: LeftoverText[];
};

export type LeftoverReport = {
  target_language: string;
  scanned_files: number;
  total_texts: number;
  code_like_texts: number;
  unreadable: string[];
  files: LeftoverFile[];
};

export type BranchIssue = {
  kind: "unsupported" | "awaiting_download";
  current_branch: string;
  branch_min: string;
  branch_max: string;
  message: string;
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
  game_branch: string | null;
  game_version: string | null;
};

export type TranslationPreparationProgress = {
  phase: "source_check" | "cache_hit" | "extracting" | "selecting_files" | "copying_files" | "source_ready" | "creating_sheet" | "opening_tools";
  detail: string;
  step: number;
  total_steps: number;
  source_bytes: number;
  cache_hit: boolean | null;
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
  can_export_patch_mod: boolean;
};

export type TranslationProjectInfo = {
  modKey?: string;
  modPath?: string;
  modName: string;
  version: string;
  author: string;
  description: string;
  languages: LanguagePreview[];
  canExportPatchMod?: boolean;
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
  previous_source_value?: string | null;
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
  /** Advisory term mismatches; never make the sheet invalid. */
  glossary_issues?: JsonValidationIssue[];
};

export type JsonValidationIssue = {
  key: string;
  kind: string;
  message: string;
  /** Translation the suggestion was computed from. */
  original?: string | null;
  /** Corrected translation, applicable while the value still equals `original`. */
  suggestion?: string | null;
};

export type JsonApply = {
  output_path: string;
  applied_entries: number;
  language_output_path?: string;
  packed_pck_path?: string;
  installed_mod_path?: string;
  bridge?: BridgeApply | null;
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

export type AiKitExport = {
  output_path: string;
  rows: number;
  guide_path: string;
  glossary_path: string;
  glossary_terms: number;
};

export type GlossaryTerm = {
  source: string;
  targets: string[];
};

export type GlossaryInfo = {
  source_language: string;
  target_language: string;
  game_pck_path?: string | null;
  game_terms: number;
  user_terms: GlossaryTerm[];
};

export type ProjectTranslationSettings = {
  mod_description: string;
  tone: string;
  terms: GlossaryTerm[];
  extra_instructions: string;
  include_prompt?: boolean | null;
  only_empty?: boolean | null;
  max_chars?: number | null;
};

export type AiPromptPart = {
  text: string;
  rows: number;
  chars: number;
  terms: number;
  oversized: boolean;
  slots: Array<{ file: string; id: string }>;
};

export type AiPromptParts = {
  parts: AiPromptPart[];
  total_rows: number;
};

export type TermCandidate = {
  source: string;
  count: number;
  kinds: string[];
  suggestions: Array<{ text: string; count: number }>;
  shares_translation_with: string[];
  example: string;
};
