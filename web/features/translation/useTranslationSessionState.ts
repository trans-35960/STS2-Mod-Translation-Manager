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
import { readStoredTranslationSession, writeStoredTranslationSession } from "../../utils/storage";

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
  const [jsonSheet, setJsonSheet] = React.useState<JsonTranslationSheet | null>(null);
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
      setJsonSheet(session.sheet);
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
  }, [appendLog, setPage]);

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
