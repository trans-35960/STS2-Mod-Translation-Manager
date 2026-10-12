import React from "react";
import type {
  ApplyResultState,
  JsonSheetReport,
  JsonTranslationSheet,
  JsonValidation,
  Page,
  PasteCandidate,
  TranslationProjectInfo,
} from "../../types";
import { invokeCommand } from "../../api/tauri";
import { isPreviewRuntime } from "../../utils/runtime";
import { readStoredTranslationSession, writeStoredTranslationSession } from "../../utils/storage";
import { countUnsavedChanges, translationLanguagesMatch } from "./translationUtils";
import { useSheetHistory } from "./useSheetHistory";

/** Edits are written to the sheet file this long after the last change. */
const AUTOSAVE_DELAY_MS = 1500;

export type SheetSaveState = "idle" | "saving" | "saved" | "error";

export function useTranslationSessionState({
  appendLog,
  setPage,
}: {
  appendLog: (message: string) => void;
  setPage: (page: Page) => void;
}) {
  const [jsonSource, setJsonSource] = React.useState("");
  const [jsonExistingSheet, setJsonExistingSheet] = React.useState("");
  const [jsonOutputSheet, setJsonOutputSheet] = React.useState("");
  const [jsonTranslatedOutput, setJsonTranslatedOutput] = React.useState("");
  const [jsonPckTargetPath, setJsonPckTargetPath] = React.useState("");
  const [jsonSheet, setJsonSheetState] = React.useState<JsonTranslationSheet | null>(null);
  const history = useSheetHistory(setJsonSheetState);
  // The sheet as it is on disk, to count unsaved edits. Values set directly
  // (load, create, recalculate) come from the backend and match the file;
  // functional updates are edits and go into the undo history.
  const [savedSheet, setSavedSheet] = React.useState<JsonTranslationSheet | null>(null);
  const savedSheetRef = React.useRef<JsonTranslationSheet | null>(null);
  savedSheetRef.current = savedSheet;
  const { edit: editSheet, reset: resetSheet, sheetRef } = history;
  const setJsonSheet = React.useCallback<React.Dispatch<React.SetStateAction<JsonTranslationSheet | null>>>(
    (next) => {
      if (typeof next === "function") {
        editSheet(next(sheetRef.current), "편집");
        return;
      }
      setSavedSheet(next);
      resetSheet(next);
    },
    [editSheet, resetSheet, sheetRef],
  );
  /** In-memory results (imports): an undoable edit that is not saved yet. */
  const setJsonSheetDraft = React.useCallback<React.Dispatch<React.SetStateAction<JsonTranslationSheet | null>>>(
    (next) => editSheet(typeof next === "function" ? next(sheetRef.current) : next, "가져오기"),
    [editSheet, sheetRef],
  );
  const [jsonReport, setJsonReport] = React.useState<JsonSheetReport | null>(null);
  const [jsonValidation, setJsonValidation] = React.useState<JsonValidation | null>(null);
  const [jsonApplyResult, setJsonApplyResult] = React.useState<ApplyResultState | null>(null);
  const [jsonToolError, setJsonToolError] = React.useState("");
  const [translationProject, setTranslationProject] = React.useState<TranslationProjectInfo | null>(null);
  const [jsonTargetLanguage, setJsonTargetLanguage] = React.useState("kor");
  const [compareSamplePaths, setCompareSamplePaths] = React.useState<string[]>([]);
  const [compareValuesByLanguage, setCompareValuesByLanguage] = React.useState<Record<string, Record<string, string>>>({});
  const [compareViewEnabled, setCompareViewEnabled] = React.useState(false);
  const [selectedRows, setSelectedRows] = React.useState<number[]>([]);
  const [pasteCandidatesByKey, setPasteCandidatesByKey] = React.useState<Record<string, PasteCandidate>>({});
  const [translationSessionReady, setTranslationSessionReady] = React.useState(false);

  React.useEffect(() => {
    const session = readStoredTranslationSession();
    if (session) {
      setJsonSource(session.sourcePath);
      setJsonExistingSheet(session.existingSheetPath);
      setJsonOutputSheet(session.outputSheetPath);
      setJsonTranslatedOutput(session.translatedOutputPath);
      setJsonPckTargetPath(session.pckTargetPath);
      resetSheet(session.sheet);
      // The restored sheet may hold unsaved edits; compare against the file.
      const sheetPath = session.outputSheetPath || session.existingSheetPath;
      if (session.sheet && sheetPath && !isPreviewRuntime()) {
        invokeCommand("load_json_translation_sheet", { sheetPath })
          .then((saved) => setSavedSheet(saved))
          .catch(() => setSavedSheet(session.sheet));
      } else {
        setSavedSheet(session.sheet);
      }
      setJsonReport(session.report);
      setJsonValidation(session.validation);
      setJsonApplyResult(session.applyResult);
      setTranslationProject(session.projectInfo);
      setJsonTargetLanguage(session.targetLanguage);
      setCompareSamplePaths(session.compareSamplePaths);
      setCompareValuesByLanguage(session.compareValuesByLanguage);
      setCompareViewEnabled(session.compareViewEnabled);
      setSelectedRows([]);
      setPasteCandidatesByKey({});
      setPage("translationTools");
      appendLog(`이전 번역 작업을 복구했습니다: ${session.projectInfo?.modName || session.sourcePath}`);
    }
    setTranslationSessionReady(true);
  }, [appendLog, resetSheet, setPage]);

  React.useEffect(() => {
    if (!translationSessionReady) {
      return;
    }
    const timeoutId = window.setTimeout(() => {
      writeStoredTranslationSession({
        sourcePath: jsonSource,
        existingSheetPath: jsonExistingSheet,
        outputSheetPath: jsonOutputSheet,
        translatedOutputPath: jsonTranslatedOutput,
        pckTargetPath: jsonPckTargetPath,
        sheet: jsonSheet,
        report: jsonReport,
        validation: jsonValidation,
        applyResult: jsonApplyResult,
        projectInfo: translationProject,
        targetLanguage: jsonTargetLanguage,
        compareSamplePaths,
        compareValuesByLanguage,
        compareViewEnabled,
      });
    }, 1000);
    return () => window.clearTimeout(timeoutId);
  }, [
    compareSamplePaths,
    compareValuesByLanguage,
    compareViewEnabled,
    jsonApplyResult,
    jsonExistingSheet,
    jsonOutputSheet,
    jsonPckTargetPath,
    jsonReport,
    jsonSheet,
    jsonSource,
    jsonTargetLanguage,
    jsonTranslatedOutput,
    jsonValidation,
    translationProject,
    translationSessionReady,
  ]);

  // Autosave: write edits to the sheet file shortly after they stop. Only the
  // file and the report change; validation results, conflict candidates and
  // the sheet being edited are left alone.
  const [saveState, setSaveState] = React.useState<SheetSaveState>("idle");
  const [saveError, setSaveError] = React.useState("");
  const saveTargetRef = React.useRef({ path: "", targetLanguage: "" });
  saveTargetRef.current = { path: jsonOutputSheet || jsonExistingSheet, targetLanguage: jsonTargetLanguage };
  const inFlightSaveRef = React.useRef<Promise<boolean> | null>(null);

  const saveSheetNow = React.useCallback(async (): Promise<boolean> => {
    while (inFlightSaveRef.current) {
      await inFlightSaveRef.current;
    }
    const sheet = sheetRef.current;
    const { path, targetLanguage } = saveTargetRef.current;
    if (!sheet) {
      return true;
    }
    if (isPreviewRuntime()) {
      setSavedSheet(sheet);
      setSaveState("saved");
      return true;
    }
    if (!path) {
      setSaveState("error");
      setSaveError("저장할 번역 시트 경로가 없습니다.");
      return false;
    }
    if (targetLanguage && !translationLanguagesMatch(sheet.target_language, targetLanguage)) {
      setSaveState("error");
      setSaveError(`현재 시트는 ${sheet.target_language}용이라 ${targetLanguage} 작업 중에는 저장하지 않습니다.`);
      return false;
    }
    if (countUnsavedChanges(sheet, savedSheetRef.current) === 0 && savedSheetRef.current) {
      return true;
    }
    setSaveState("saving");
    const run = invokeCommand("save_json_translation_sheet", { sheetPath: path, sheet })
      .then((result) => {
        setSavedSheet(sheet);
        setJsonReport(result.report);
        setSaveState("saved");
        setSaveError("");
        return true;
      })
      .catch((error) => {
        setSaveState("error");
        setSaveError(String(error));
        return false;
      })
      .finally(() => {
        inFlightSaveRef.current = null;
      });
    inFlightSaveRef.current = run;
    return run;
  }, [sheetRef]);

  const pendingChanges = React.useMemo(() => countUnsavedChanges(jsonSheet, savedSheet), [jsonSheet, savedSheet]);
  React.useEffect(() => {
    if (!translationSessionReady || pendingChanges === 0) {
      return;
    }
    const timeoutId = window.setTimeout(() => void saveSheetNow(), AUTOSAVE_DELAY_MS);
    return () => window.clearTimeout(timeoutId);
  }, [jsonSheet, pendingChanges, saveSheetNow, translationSessionReady]);

  const lastLoggedErrorRef = React.useRef("");
  React.useEffect(() => {
    if (saveError && saveError !== lastLoggedErrorRef.current) {
      appendLog(`자동 저장 실패: ${saveError}`);
    }
    lastLoggedErrorRef.current = saveError;
  }, [appendLog, saveError]);

  return {
    jsonSource,
    setJsonSource,
    jsonExistingSheet,
    setJsonExistingSheet,
    jsonOutputSheet,
    setJsonOutputSheet,
    jsonTranslatedOutput,
    setJsonTranslatedOutput,
    jsonPckTargetPath,
    setJsonPckTargetPath,
    jsonSheet,
    setJsonSheet,
    setJsonSheetDraft,
    savedSheet,
    sheetHistory: history.state,
    labelNextSheetEdit: history.labelNextEdit,
    undoSheetEdit: history.undo,
    redoSheetEdit: history.redo,
    revertSheetToStart: history.revertToStart,
    sheetSaveState: saveState,
    sheetSaveError: saveError,
    saveSheetNow,
    jsonReport,
    setJsonReport,
    jsonValidation,
    setJsonValidation,
    jsonApplyResult,
    setJsonApplyResult,
    jsonToolError,
    setJsonToolError,
    translationProject,
    setTranslationProject,
    jsonTargetLanguage,
    setJsonTargetLanguage,
    compareSamplePaths,
    setCompareSamplePaths,
    compareValuesByLanguage,
    setCompareValuesByLanguage,
    compareViewEnabled,
    setCompareViewEnabled,
    selectedRows,
    setSelectedRows,
    pasteCandidatesByKey,
    setPasteCandidatesByKey,
    clearStoredSession: () => writeStoredTranslationSession(null),
  };
}
