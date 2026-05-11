import type React from "react";
import type {
  ApplyResultState,
  Dashboard,
  ExtractionTreeNode,
  JsonSheetReport,
  JsonTranslationSheet,
  JsonValidation,
  LanguageCompareValue,
  ModRow,
  Page,
  PasteCandidate,
  TranslationProjectInfo,
  UiSettings,
} from "../../types";

export type StateSetter<T> = React.Dispatch<React.SetStateAction<T>>;

export type TranslationActionsParams = {
  appendLog: (message: string) => void;
  busy: string | null;
  compareSamplePaths: string[];
  compareValuesByLanguage: Record<string, Record<string, string>>;
  compareViewEnabled: boolean;
  dashboard: Dashboard | null;
  jsonApplyResult: ApplyResultState | null;
  jsonExistingSheet: string;
  jsonOutputSheet: string;
  jsonPckTargetPath: string;
  jsonSheet: JsonTranslationSheet | null;
  jsonSource: string;
  jsonTargetLanguage: string;
  jsonTranslatedOutput: string;
  jsonValidation: JsonValidation | null;
  load: () => Promise<void>;
  page: Page;
  pasteCandidatesByKey: Record<string, PasteCandidate>;
  selectedRows: number[];
  settingsDraft: UiSettings | null;
  translationProject: TranslationProjectInfo | null;
  clearStoredSession: () => void;
  setBusy: StateSetter<string | null>;
  setCompareSamplePaths: StateSetter<string[]>;
  setCompareValuesByLanguage: StateSetter<Record<string, Record<string, string>>>;
  setCompareViewEnabled: StateSetter<boolean>;
  setJsonApplyResult: StateSetter<ApplyResultState | null>;
  setJsonExistingSheet: StateSetter<string>;
  setJsonOutputSheet: StateSetter<string>;
  setJsonPckTargetPath: StateSetter<string>;
  setJsonReport: StateSetter<JsonSheetReport | null>;
  setJsonSheet: StateSetter<JsonTranslationSheet | null>;
  setJsonSource: StateSetter<string>;
  setJsonTargetLanguage: StateSetter<string>;
  setJsonToolError: StateSetter<string>;
  setJsonTranslatedOutput: StateSetter<string>;
  setJsonValidation: StateSetter<JsonValidation | null>;
  setPage: StateSetter<Page>;
  setPasteCandidatesByKey: StateSetter<Record<string, PasteCandidate>>;
  setPendingExtract: StateSetter<ModRow | null>;
  setSelectedRows: StateSetter<number[]>;
  setTranslationProject: StateSetter<TranslationProjectInfo | null>;
};

export type RunJsonTool = (command: string, args: Record<string, unknown>) => Promise<boolean>;
export type CompareValueCache = Record<string, Record<string, string>>;
export type CompareLanguageValues = Record<string, LanguageCompareValue[]>;
