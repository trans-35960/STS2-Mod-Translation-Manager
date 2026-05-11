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
  onReplaceEntries: (updates: Array<{ index: number; value: string }>) => void;
  onPasteEntries: (index: number, text: string) => void;
  onPasteStructuredJson: (text: string) => boolean;
  onApplyPasteCandidate: (entryKey: string) => void;
  onApplyAllPasteCandidates: () => void;
  onDismissPasteCandidate: (entryKey: string) => void;
  onDismissAllPasteCandidates: () => void;
  onSelectRow: (index: number, event: React.MouseEvent) => void;
  onCopySelected: () => void;
  onSave: () => void;
  onExportCsv: () => void;
  onExportPatchMod: () => void;
  onExportShortJson: (options: boolean | { onlyEmpty?: boolean; scopePath?: string; warningOnly?: boolean }) => void;
  onImportValues: () => void;
  onToggleCompareLanguage: (samplePath: string) => void;
  onToggleCompareView: () => void;
  onSwitchSourceLanguage: (samplePath: string) => void;
  onOpenPath: (path: string) => void;
  onCloseSession: () => void;
  onCreate: () => void;
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
