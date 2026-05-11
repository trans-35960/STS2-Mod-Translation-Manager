import React from "react";
import type { JsonSheetReport, JsonValidation } from "../../types";
import {
  normalizeLanguageTag,
  recommendedSourceLanguage,
  translationTargetOptions,
  uniqueLanguagePreviews,
} from "../mods/modUtils";
import type {
  ReplaceScope,
  TranslationColumnKey,
  TranslationColumns,
  TranslationEntryFilter,
  TranslationToolsPageProps,
} from "./TranslationToolsTypes";
import {
  buildTranslationProjectTree,
  hasTranslationValue,
  isTranslatableEntry,
  languageCodeFromSourcePath,
  languageCodeFromSheetKey,
  languageFolderCode,
  pathMatchesProjectNode,
  splitSheetKey,
  translationSlotEntries,
  type TranslationProjectNode,
} from "./translationUtils";

const loadMoreBatch = 120;
const defaultColumns: TranslationColumns = {
  status: 76,
  id: 150,
  file: 220,
  key: 280,
  source: 300,
  compare: 300,
  translated: 360,
};

export function useTranslationToolsUiState(props: TranslationToolsPageProps) {
  const sheetPath = props.outputSheetPath || props.existingSheetPath;
  const [activeProjectPath, setActiveProjectPath] = React.useState<string | null>(null);
  const [entryFilter, setEntryFilter] = React.useState<TranslationEntryFilter>("all");
  const [entrySearch, setEntrySearch] = React.useState("");
  const [replaceSearch, setReplaceSearch] = React.useState("");
  const [replaceWith, setReplaceWith] = React.useState("");
  const [visibleLimit, setVisibleLimit] = React.useState(loadMoreBatch);
  const [exportOnlyEmpty, setExportOnlyEmpty] = React.useState(false);
  const [showIds, setShowIds] = React.useState(true);
  const [columns, setColumns] = React.useState<TranslationColumns>(defaultColumns);
  const [focusEntryKey, setFocusEntryKey] = React.useState<string | null>(null);
  const [validationIssueKindFilter, setValidationIssueKindFilter] = React.useState<string | null>(null);

  const sheetSourceLanguage = React.useMemo(
    () =>
      languageCodeFromSourcePath(props.sheet?.source_path ?? "") ||
      props.sheet?.entries.map((entry) => languageCodeFromSheetKey(entry.key)).find(Boolean) ||
      "",
    [props.sheet],
  );
  const compareLanguageOptions = React.useMemo(
    () =>
      uniqueLanguagePreviews(props.availableLanguages).filter(
        (language) =>
          language.sample_path &&
          normalizeLanguageTag(languageFolderCode(language)) !== normalizeLanguageTag(props.targetLanguage),
      ),
    [props.availableLanguages, props.targetLanguage],
  );
  const sourceLanguageOptions = React.useMemo(
    () => uniqueLanguagePreviews(props.availableLanguages).filter((language) => language.sample_path),
    [props.availableLanguages],
  );
  const selectedSourceLanguage = React.useMemo(
    () =>
      sourceLanguageOptions.find(
        (language) =>
          normalizeLanguageTag(languageFolderCode(language)) === normalizeLanguageTag(sheetSourceLanguage),
      ) ?? recommendedSourceLanguage(sourceLanguageOptions),
    [sheetSourceLanguage, sourceLanguageOptions],
  );
  const targetLanguageOptions = React.useMemo(
    () => translationTargetOptions(props.targetLanguage, props.settings.target_language, props.availableLanguages),
    [props.availableLanguages, props.settings.target_language, props.targetLanguage],
  );
  const selectedCompareLanguages = React.useMemo(
    () => compareLanguageOptions.filter((language) => props.compareSamplePaths.includes(language.sample_path)),
    [compareLanguageOptions, props.compareSamplePaths],
  );
  const pasteCandidateCount = Object.keys(props.pasteCandidatesByKey).length;
  const allValidationWarningKeys = React.useMemo(
    () => validationIssueKeys(props.validation, null),
    [props.validation],
  );
  const validationWarningKeys = React.useMemo(
    () => validationIssueKeys(props.validation, validationIssueKindFilter),
    [props.validation, validationIssueKindFilter],
  );
  const showCompareColumn = props.compareViewEnabled && selectedCompareLanguages.length > 0;
  const filledTranslations = React.useMemo(
    () =>
      (props.sheet?.entries ?? []).filter(
        (entry) => isTranslatableEntry(entry) && hasTranslationValue(entry.translated_value),
      ).length,
    [props.sheet],
  );
  const slotIdByEntryKey = React.useMemo(() => {
    if (!props.sheet) {
      return new Map<string, string>();
    }
    return new Map(translationSlotEntries(props.sheet).map((slot) => [slot.entry.key, slot.id]));
  }, [props.sheet]);
  const sheetStats = React.useMemo((): JsonSheetReport | null => {
    if (!props.sheet) {
      return props.report;
    }
    const translatableEntries = props.sheet.entries.filter(
      (entry) => entry.status !== "removed" && isTranslatableEntry(entry),
    );
    return {
      sheet_path: props.report?.sheet_path ?? sheetPath,
      entries: translatableEntries.length,
      new_entries: translatableEntries.filter((entry) => entry.status === "new").length,
      updated_entries: translatableEntries.filter((entry) => entry.status === "updated").length,
      missing_entries: translatableEntries.filter((entry) => !hasTranslationValue(entry.translated_value)).length,
      removed_entries: props.sheet.entries.filter((entry) => entry.status === "removed").length,
    };
  }, [props.report, props.sheet, sheetPath]);
  const projectTree = React.useMemo(() => buildTranslationProjectTree(props.sheet), [props.sheet]);
  const filteredEntries = React.useMemo(
    () => {
      const query = entrySearch.trim().toLowerCase();
      return (props.sheet?.entries ?? [])
        .map((entry, index) => ({ entry, index, parts: splitSheetKey(entry.key) }))
        .filter(({ entry, parts }) => {
          const isRemoved = entry.status === "removed";
          const showRemovedWarning = entryFilter === "warning" && validationIssueKindFilter === "removed";
          if (isRemoved && entryFilter !== "removed" && !showRemovedWarning) {
            return false;
          }
          if (!isRemoved && !isTranslatableEntry(entry)) {
            return false;
          }
          if (activeProjectPath && !pathMatchesProjectNode(parts.file, activeProjectPath)) {
            return false;
          }
          let matchesFilter = true;
          if (entryFilter === "empty") {
            matchesFilter = isTranslatableEntry(entry) && !hasTranslationValue(entry.translated_value);
          } else if (entryFilter === "new") {
            matchesFilter = entry.status === "new";
          } else if (entryFilter === "updated") {
            matchesFilter = entry.status === "updated";
          } else if (entryFilter === "removed") {
            matchesFilter = isRemoved;
          } else if (entryFilter === "warning") {
            matchesFilter = validationWarningKeys.has(entry.key);
          } else if (entryFilter === "conflict") {
            matchesFilter = Boolean(props.pasteCandidatesByKey[entry.key]);
          }
          if (!matchesFilter) {
            return false;
          }
          if (!query) {
            return true;
          }
          const shortId = (slotIdByEntryKey.get(entry.key) ?? "").toLowerCase();
          return [shortId, entry.key, entry.source_value, entry.translated_value].some((value) =>
            value.toLowerCase().includes(query),
          );
        });
    },
    [
      activeProjectPath,
      entryFilter,
      entrySearch,
      props.pasteCandidatesByKey,
      props.sheet,
      slotIdByEntryKey,
      validationIssueKindFilter,
      validationWarningKeys,
    ],
  );
  const visibleEntries = filteredEntries.slice(0, visibleLimit);
  const replaceTargetEntries =
    props.selectedRows.length > 0
      ? filteredEntries.filter(({ index }) => props.selectedRows.includes(index))
      : filteredEntries;
  const replaceMatchCount = replaceSearch
    ? replaceTargetEntries.filter(({ entry }) => entry.translated_value.includes(replaceSearch)).length
    : 0;

  React.useEffect(() => {
    setVisibleLimit(loadMoreBatch);
  }, [activeProjectPath, entryFilter, entrySearch, props.sheet]);

  React.useEffect(() => {
    if (entryFilter === "conflict" && pasteCandidateCount === 0) {
      setEntryFilter("all");
    }
    if (entryFilter === "warning" && validationWarningKeys.size === 0) {
      setEntryFilter("all");
      setValidationIssueKindFilter(null);
    }
  }, [entryFilter, pasteCandidateCount, validationWarningKeys.size]);

  React.useEffect(() => {
    setValidationIssueKindFilter(null);
  }, [props.validation]);

  function startColumnResize(column: TranslationColumnKey, event: React.MouseEvent) {
    event.preventDefault();
    event.stopPropagation();
    const startX = event.clientX;
    const startWidth = columns[column];
    const onMove = (moveEvent: MouseEvent) => {
      const minWidth = column === "status" ? 70 : 140;
      setColumns((current) => ({
        ...current,
        [column]: Math.max(minWidth, startWidth + moveEvent.clientX - startX),
      }));
    };
    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
  }

  function loadMoreVisibleEntries() {
    setVisibleLimit((value) => Math.min(value + loadMoreBatch, filteredEntries.length));
  }

  function showEntryCondition(filter: TranslationEntryFilter) {
    setActiveProjectPath(null);
    setEntrySearch("");
    setValidationIssueKindFilter(null);
    setEntryFilter(filter);
  }

  function showValidationIssueKind(kind: string | null) {
    setActiveProjectPath(null);
    setEntrySearch("");
    setValidationIssueKindFilter(kind);
    setEntryFilter("warning");
  }

  function revealEntryKey(key: string) {
    if (!props.sheet) {
      return;
    }
    const targetEntry = props.sheet.entries.find((entry) => entry.key === key);
    const position = props.sheet.entries
      .map((entry, index) => ({ entry, index }))
      .filter(({ entry }) => entry.status === "removed" || isTranslatableEntry(entry))
      .findIndex(({ entry }) => entry.key === key);
    setActiveProjectPath(null);
    setEntrySearch("");
    setValidationIssueKindFilter(null);
    setEntryFilter(targetEntry?.status === "removed" ? "removed" : "all");
    setVisibleLimit(Math.max(loadMoreBatch, position + 1));
    setFocusEntryKey(key);
  }

  function replaceTranslatedValues(scope: ReplaceScope) {
    if (!props.sheet || !replaceSearch) {
      return;
    }
    const candidates =
      scope === "all" ? props.sheet.entries.map((entry, index) => ({ entry, index })) : replaceTargetEntries;
    const updates = candidates
      .filter(({ entry }) => entry.status !== "removed" && entry.translated_value.includes(replaceSearch))
      .map(({ entry, index }) => ({
        index,
        value: entry.translated_value.split(replaceSearch).join(replaceWith),
      }));
    if (updates.length === 0) {
      return;
    }
    const label = scope === "all" ? "전체 시트" : props.selectedRows.length > 0 ? "선택 항목" : "현재 표시 항목";
    const ok = window.confirm(`${label}에서 ${updates.length}개 항목의 translated_value를 치환할까요?`);
    if (!ok) {
      return;
    }
    props.onReplaceEntries(updates);
  }

  function expandVisibleEntriesOnScroll(event: React.UIEvent<HTMLDivElement>) {
    if (visibleEntries.length >= filteredEntries.length) {
      return;
    }
    const target = event.currentTarget;
    const distanceToBottom = target.scrollHeight - target.scrollTop - target.clientHeight;
    if (distanceToBottom < 220) {
      loadMoreVisibleEntries();
    }
  }

  async function copyTreeJson(node: TranslationProjectNode) {
    if (!props.sheet) {
      return;
    }
    const scope = node.filterPath;
    const output: Record<string, Record<string, string>> = {};
    for (const slot of translationSlotEntries(props.sheet)) {
      const entry = slot.entry;
      if (entry.status === "removed") {
        continue;
      }
      if (scope && !pathMatchesProjectNode(slot.file, scope)) {
        continue;
      }
      output[slot.compactFile] = {
        ...(output[slot.compactFile] ?? {}),
        [slot.id]: entry.source_value,
      };
    }
    await navigator.clipboard?.writeText(JSON.stringify(output, null, 2));
  }

  return {
    activeProjectPath,
    setActiveProjectPath,
    entryFilter,
    setEntryFilter,
    entrySearch,
    setEntrySearch,
    replaceSearch,
    setReplaceSearch,
    replaceWith,
    setReplaceWith,
    exportOnlyEmpty,
    setExportOnlyEmpty,
    showIds,
    setShowIds,
    columns,
    sheetPath,
    sheetSourceLanguage,
    compareLanguageOptions,
    sourceLanguageOptions,
    selectedSourceLanguage,
    targetLanguageOptions,
    selectedCompareLanguages,
    pasteCandidateCount,
    validationWarningCount: allValidationWarningKeys.size,
    validationIssueKindFilter,
    showCompareColumn,
    filledTranslations,
    slotIdByEntryKey,
    sheetStats,
    projectTree,
    filteredEntries,
    visibleEntries,
    replaceMatchCount,
    startColumnResize,
    loadMoreVisibleEntries,
    showEntryCondition,
    showValidationIssueKind,
    replaceTranslatedValues,
    expandVisibleEntriesOnScroll,
    copyTreeJson,
    focusEntryKey,
    revealEntryKey,
    clearFocusEntryKey: () => setFocusEntryKey(null),
  };
}

function validationIssueKeys(validation: JsonValidation | null, kind: string | null): Set<string> {
  if (!validation) {
    return new Set();
  }
  const keys = new Set<string>();
  if (!kind || kind === "missing") {
    validation.missing_entries.forEach((key) => keys.add(key));
  }
  if (!kind || kind === "updated") {
    validation.updated_entries.forEach((key) => keys.add(key));
  }
  if (kind === "removed") {
    validation.removed_entries.forEach((key) => keys.add(key));
  }
  for (const issue of validation.format_issues ?? []) {
    if (!kind || issue.kind === kind) {
      keys.add(issue.key);
    }
  }
  return keys;
}
