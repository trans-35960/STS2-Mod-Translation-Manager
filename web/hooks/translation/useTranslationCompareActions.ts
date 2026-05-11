import React from "react";
import { invokeCommand } from "../../api/tauri";
import {
  createCompareValueMap,
  languageCodeFromSheetKey,
  languageFolderCode,
} from "../../features/translation/translationUtils";
import type { LanguageCompareValue } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import type { TranslationActionsParams } from "./types";

export function useTranslationCompareActions({
  appendLog,
  busy,
  compareSamplePaths,
  compareValuesByLanguage,
  compareViewEnabled,
  jsonExistingSheet,
  jsonOutputSheet,
  jsonSheet,
  translationProject,
  setBusy,
  setCompareSamplePaths,
  setCompareValuesByLanguage,
}: TranslationActionsParams) {
  React.useEffect(() => {
    if (!compareViewEnabled || !jsonSheet || busy) {
      return;
    }
    const sourceLanguage = jsonSheet.entries.map((entry) => languageCodeFromSheetKey(entry.key)).find(Boolean) ?? "";
    const missingSamplePath = compareSamplePaths.find((samplePath) => {
      const language = translationProject?.languages.find((item) => item.sample_path === samplePath);
      if (sourceLanguage && language && languageFolderCode(language) === sourceLanguage) {
        return false;
      }
      return !Object.prototype.hasOwnProperty.call(compareValuesByLanguage, samplePath);
    });
    if (missingSamplePath) {
      void loadCompareLanguageValues(missingSamplePath);
    }
  }, [busy, compareSamplePaths, compareValuesByLanguage, compareViewEnabled, jsonSheet, translationProject]);

  async function toggleCompareLanguage(samplePath: string) {
    if (!samplePath) {
      return;
    }
    if (compareSamplePaths.includes(samplePath)) {
      setCompareSamplePaths((current) => current.filter((path) => path !== samplePath));
      setCompareValuesByLanguage((current) => {
        const next = { ...current };
        delete next[samplePath];
        return next;
      });
      return;
    }
    setCompareSamplePaths((current) => [...current, samplePath]);
    const loaded = await loadCompareLanguageValues(samplePath);
    if (!loaded) {
      setCompareSamplePaths((current) => current.filter((path) => path !== samplePath));
    }
  }

  async function loadCompareLanguageValues(samplePath: string) {
    const sheetPath = jsonOutputSheet || jsonExistingSheet;
    if (!sheetPath) {
      appendLog("비교할 번역 시트를 먼저 생성하거나 불러오세요.");
      return false;
    }
    setBusy("compare_translation_language");
    try {
      if (isPreviewRuntime()) {
        const previewValues = createCompareValueMap(
          (jsonSheet?.entries ?? []).map((entry) => ({ key: entry.key, value: `compare: ${entry.source_value}` })),
        );
        setCompareValuesByLanguage((current) => ({ ...current, [samplePath]: previewValues }));
        appendLog("Preview comparison loaded.");
        return true;
      }
      const result = await invokeCommand<LanguageCompareValue[]>("compare_translation_language", {
        sheetPath,
        samplePath,
      });
      setCompareValuesByLanguage((current) => ({
        ...current,
        [samplePath]: createCompareValueMap(result),
      }));
      appendLog(`비교 언어 불러오기 완료: ${result.length}개 값`);
      return true;
    } catch (error) {
      appendLog(String(error));
      return false;
    } finally {
      setBusy(null);
    }
  }

  return {
    loadCompareLanguageValues,
    toggleCompareLanguage,
  };
}
