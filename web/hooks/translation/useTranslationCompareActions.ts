import React from "react";
import { invokeCommand } from "../../api/tauri";
import {
  createCompareValueMap,
  isTranslatableEntry,
  languageCodeFromSourcePath,
  languageCodeFromSheetKey,
  languageFolderCode,
  normalizedLocalizationKey,
  stableCompareKey,
} from "../../features/translation/translationUtils";
import { normalizeLanguageTag } from "../../features/mods/modUtils";
import { formatCommandError } from "../../utils/logging";
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
  setCompareViewEnabled,
}: TranslationActionsParams) {
  const emptyCompareReloadsRef = React.useRef(new Set<string>());
  const compareSheetKey = React.useMemo(() => {
    if (!jsonSheet) {
      return "";
    }
    return [
      jsonSheet.source_path,
      jsonSheet.target_language,
      jsonSheet.updated_epoch,
      jsonSheet.entries.length,
      jsonSheet.entries[0]?.key ?? "",
      jsonSheet.entries[jsonSheet.entries.length - 1]?.key ?? "",
    ].join("\u0000");
  }, [jsonSheet]);

  React.useEffect(() => {
    emptyCompareReloadsRef.current.clear();
  }, [compareSheetKey, compareSamplePaths]);

  React.useEffect(() => {
    if (!compareViewEnabled || !jsonSheet || busy) {
      return;
    }
    const sourceLanguage =
      languageCodeFromSourcePath(jsonSheet.source_path) ||
      (jsonSheet.entries.map((entry) => languageCodeFromSheetKey(entry.key)).find(Boolean) ?? "");
    const missingSamplePath = compareSamplePaths.find((samplePath) => {
      const language = translationProject?.languages.find((item) => item.sample_path === samplePath);
      if (
        sourceLanguage &&
        language &&
        normalizeLanguageTag(languageFolderCode(language)) === normalizeLanguageTag(sourceLanguage)
      ) {
        return false;
      }
      if (!Object.prototype.hasOwnProperty.call(compareValuesByLanguage, samplePath)) {
        return true;
      }
      const cachedValues = compareValuesByLanguage[samplePath] ?? {};
      if (cachedCompareValuesMatchSheet(jsonSheet, cachedValues)) {
        return false;
      }
      const reloadKey = `${compareSheetKey}\u0000${samplePath}`;
      if (emptyCompareReloadsRef.current.has(reloadKey)) {
        return false;
      }
      emptyCompareReloadsRef.current.add(reloadKey);
      return true;
    });
    if (missingSamplePath) {
      void loadCompareLanguageValues(missingSamplePath);
    }
  }, [busy, compareSamplePaths, compareSheetKey, compareValuesByLanguage, compareViewEnabled, jsonSheet, translationProject]);

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
      setCompareViewEnabled(compareSamplePaths.length > 1);
      return;
    }
    setCompareSamplePaths((current) => [...current, samplePath]);
    setCompareViewEnabled(true);
    if (isSourceLanguageSample(samplePath)) {
      return;
    }
    const loaded = await loadCompareLanguageValues(samplePath);
    if (!loaded) {
      setCompareSamplePaths((current) => current.filter((path) => path !== samplePath));
      setCompareViewEnabled(compareSamplePaths.length > 0);
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
      const result = await invokeCommand("compare_translation_language", {
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
      appendLog(formatCommandError("compare_translation_language", { sheetPath, samplePath }, error));
      return false;
    } finally {
      setBusy(null);
    }
  }

  return {
    loadCompareLanguageValues,
    toggleCompareLanguage,
  };

  function isSourceLanguageSample(samplePath: string) {
    const sourceLanguage =
      languageCodeFromSourcePath(jsonSheet?.source_path ?? "") ||
      (jsonSheet?.entries.map((entry) => languageCodeFromSheetKey(entry.key)).find(Boolean) ?? "");
    const language = translationProject?.languages.find((item) => item.sample_path === samplePath);
    return Boolean(
      sourceLanguage &&
        language &&
        normalizeLanguageTag(languageFolderCode(language)) === normalizeLanguageTag(sourceLanguage),
    );
  }
}

function cachedCompareValuesMatchSheet(
  sheet: NonNullable<TranslationActionsParams["jsonSheet"]>,
  values: Record<string, string>,
): boolean {
  if (Object.keys(values).length === 0) {
    return false;
  }
  const sampleEntries = sheet.entries.filter(isTranslatableEntry).slice(0, 80);
  if (sampleEntries.length === 0) {
    return true;
  }
  return sampleEntries.some((entry) =>
    Object.prototype.hasOwnProperty.call(values, entry.key) ||
    Object.prototype.hasOwnProperty.call(values, normalizedLocalizationKey(entry.key)) ||
    Object.prototype.hasOwnProperty.call(values, stableCompareKey(entry.key)),
  );
}
