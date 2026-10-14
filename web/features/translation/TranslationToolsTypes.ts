import type { SheetHistoryMove, SheetHistoryState } from "./useSheetHistory";
import type { SheetSaveState } from "./useTranslationSessionState";
import type React from "react";
import type { labels } from "../../i18n";
import type {
  ApplyResultState,
  EntryFilter,
  JsonSheetReport,
  JsonTranslationEntry,
  JsonTranslationSheet,
  JsonValidation,
  LanguagePreview,
  PasteCandidate,
  TranslationProjectInfo,
  UiSettings,
} from "../../types";

export type TranslationToolsPageProps = {
  labels: typeof labels.ko;
  settings: UiSettings;
  sourcePath: string;
  existingSheetPath: string;
  outputSheetPath: string;
  translatedOutputPath: string;
  pckTargetPath: string;
  sheet: JsonTranslationSheet | null;
  report: JsonSheetReport | null;
  validation: JsonValidation | null;
  applyResult: ApplyResultState | null;
  toolError: string;
  projectInfo: TranslationProjectInfo | null;
  targetLanguage: string;
  availableLanguages: LanguagePreview[];
  compareSamplePaths: string[];
  compareViewEnabled: boolean;
  compareValuesByLanguage: Record<string, Record<string, string>>;
  pasteCandidatesByKey: Record<string, PasteCandidate>;
  selectedRows: number[];
  busy: string | null;
  setSourcePath: (value: string) => void;
  setExistingSheetPath: (value: string) => void;
  setOutputSheetPath: (value: string) => void;
  setTranslatedOutputPath: (value: string) => void;
  setPckTargetPath: (value: string) => void;
  setTargetLanguage: (value: string) => void;
  onEditEntry: (index: number, value: string) => void;
  onReplaceEntries: (updates: Array<{ index: number; value: string }>, label?: string) => void;
  onPasteEntries: (index: number, text: string) => void;
  onPasteStructuredJson: (text: string, options?: { replace?: boolean }) => boolean;
  onApplyPasteCandidate: (entryKey: string) => void;
  onApplyAllPasteCandidates: () => void;
  onDismissPasteCandidate: (entryKey: string) => void;
  onDismissAllPasteCandidates: () => void;
  onSelectRow: (index: number, event: React.MouseEvent) => void;
  onCopySelected: () => void;
  onSave: () => void;
  /** Entries changed since the sheet was last loaded or saved. */
  unsavedChanges: number;
  /** Autosave status of the sheet file. */
  saveState: SheetSaveState;
  saveError: string;
  /** Saves pending edits now (before closing). */
  onFlushSave: () => Promise<boolean>;
  history: SheetHistoryState;
  onUndo: () => SheetHistoryMove | null;
  onRedo: () => SheetHistoryMove | null;
  onRevertToStart: () => SheetHistoryMove | null;
  onExportCsv: () => void;
  onExportPatchMod: () => void;
  onExportShortJson: (options: boolean | { aiKit?: boolean; changeOnly?: boolean; onlyEmpty?: boolean; scopePath?: string; warningOnly?: boolean }) => void;
  onImportValues: () => void;
  onToggleCompareLanguage: (samplePath: string) => void;
  onToggleCompareView: () => void;
  onSwitchSourceLanguage: (samplePath: string) => void;
  onOpenPath: (path: string) => void;
  onCloseSession: () => void;
  onCreate: () => void;
  onRecalculate: () => void;
  onLoad: () => void;
  onValidate: () => void | boolean | Promise<void | boolean>;
  onApply: () => void;
};

export type TranslationEntryRow = {
  entry: JsonTranslationEntry;
  index: number;
  parts: {
    file: string;
    key: string;
  };
};

export type TranslationColumnKey =
  | "status"
  | "id"
  | "file"
  | "key"
  | "source"
  | "compare"
  | "translated";

export type TranslationColumns = Record<TranslationColumnKey, number>;

export type ReplaceScope = "filtered" | "all";

export type TranslationEntryFilter = EntryFilter;
