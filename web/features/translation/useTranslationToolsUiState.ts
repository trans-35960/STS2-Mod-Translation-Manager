import React from "react";
import { invokeCommand } from "../../api/tauri";
import type { DllStringUses, JsonSheetReport, JsonValidation } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
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
  dllTextKeys as findDllTextKeys,
  dllUseOf,
  isHardcodedSheet,
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

/** How often a DLL sheet re-reads the bridge's analysis and on-screen record. */
const DLL_USES_REFRESH_MS = 15000;

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
  const [exportOnlyEmpty, setExportOnlyEmpty] = React.useState(false);
  const [showIds, setShowIds] = React.useState(true);
  const [dllTextOnly, setDllTextOnly] = React.useState(true);
  const [columns, setColumns] = React.useState<TranslationColumns>(defaultColumns);
  const [focusEntryKey, setFocusEntryKey] = React.useState<string | null>(null);
  // Bumped on every request, so asking for the same row again still scrolls.
  const [focusRequest, setFocusRequest] = React.useState(0);
  const [focusEntryInput, setFocusEntryInput] = React.useState(true);
  const [validationIssueKindFilter, setValidationIssueKindFilter] = React.useState<string | null>(null);
  const splitKeyCacheRef = React.useRef(new Map<string, ReturnType<typeof splitSheetKey>>());

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
  const dllSourcePath = props.sheet && isHardcodedSheet(props.sheet.source_path) ? props.sheet.source_path : "";
  const [dllUses, setDllUses] = React.useState<DllStringUses | null>(null);
  // The bridge analyses the DLL and records what it sees on screen while the
  // game runs. Focus events do not always arrive when coming back from the
  // game, so also look again every few seconds; nothing re-renders unless
  // the files changed.
  React.useEffect(() => {
    setDllUses(null);
    if (!dllSourcePath || isPreviewRuntime()) {
      return;
    }
    let cancelled = false;
    let lastSignature = "";
    const load = () => {
      invokeCommand("dll_string_uses", { sourcePath: dllSourcePath })
        .then((uses) => {
          const signature = uses
            ? `${uses.analyzed_at}|${uses.seen.length}|${uses.partial.length}|${Object.keys(uses.uses).length}`
            : "";
          if (!cancelled && signature !== lastSignature) {
            lastSignature = signature;
            setDllUses(uses);
          }
        })
        .catch(() => undefined);
    };
    load();
    const timer = window.setInterval(load, DLL_USES_REFRESH_MS);
    const onVisible = () => document.visibilityState === "visible" && load();
    window.addEventListener("focus", load);
    document.addEventListener("visibilitychange", onVisible);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      window.removeEventListener("focus", load);
      document.removeEventListener("visibilitychange", onVisible);
    };
  }, [dllSourcePath]);
  const dllSeen = React.useMemo(() => (dllUses ? new Set(dllUses.seen) : null), [dllUses]);
  const dllPartial = React.useMemo(() => (dllUses ? new Set(dllUses.partial ?? []) : null), [dllUses]);
  const dllTextKeys = React.useMemo(
    () => findDllTextKeys(props.sheet, props.targetLanguage, dllUses?.uses ?? null, dllSeen, dllPartial),
    [dllPartial, dllSeen, dllUses, props.sheet, props.targetLanguage],
  );
  /** DLL sheet rows the bridge replaces only where they are shown. */
  const dllPartialKeys = React.useMemo(() => {
    if (!dllPartial || dllPartial.size === 0 || !props.sheet) {
      return null;
    }
    return new Set(props.sheet.entries.filter((entry) => dllPartial.has(entry.source_value)).map((entry) => entry.key));
  }, [dllPartial, props.sheet]);
  /** DLL sheet rows the bridge saw on screen (and may translate). */
  const dllSeenKeys = React.useMemo(() => {
    if (!dllSeen || dllSeen.size === 0 || !props.sheet) {
      return null;
    }
    return new Set(
      props.sheet.entries
        .filter(
          (entry) =>
            dllUseOf(entry.source_value, dllUses?.uses ?? null, dllSeen, dllPartial) === "shown" &&
            dllSeen.has(entry.source_value),
        )
        .map((entry) => entry.key),
    );
  }, [dllPartial, dllSeen, dllUses, props.sheet]);
  const hiddenDllKeys = dllTextOnly ? dllTextKeys : null;
  const filteredEntries = React.useMemo(
    () => {
      const query = entrySearch.trim().toLowerCase();
      return (props.sheet?.entries ?? [])
        .map((entry, index) => ({
          entry,
          index,
          parts: cachedSplitSheetKey(splitKeyCacheRef.current, entry.key),
        }))
        .filter(({ entry, parts }) => {
          const isRemoved = entry.status === "removed";
          const showRemovedWarning = entryFilter === "warning" && validationIssueKindFilter === "removed";
          if (isRemoved && entryFilter !== "removed" && !showRemovedWarning) {
            return false;
          }
          if (!isRemoved && !isTranslatableEntry(entry)) {
            return false;
          }
          if (hiddenDllKeys && !isRemoved && !hiddenDllKeys.has(entry.key)) {
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
      hiddenDllKeys,
      props.pasteCandidatesByKey,
      props.sheet,
      slotIdByEntryKey,
      validationIssueKindFilter,
      validationWarningKeys,
    ],
  );
  const selectedRowSet = React.useMemo(() => new Set(props.selectedRows), [props.selectedRows]);
  const replaceTargetEntries =
    selectedRowSet.size > 0
      ? filteredEntries.filter(({ index }) => selectedRowSet.has(index))
      : filteredEntries;
  const replaceMatchCount = replaceSearch
    ? replaceTargetEntries.filter(({ entry }) => entry.translated_value.includes(replaceSearch)).length
    : 0;

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
    const minWidth = column === "status" ? 70 : 140;
    const grid = event.currentTarget.closest<HTMLElement>(".json-grid");
    const cssVariable = columnCssVariable(column);
    let latestWidth = startWidth;
    let moveCount = 0;
    const startedAt = window.performance.now();
    const onMove = (moveEvent: MouseEvent) => {
      moveCount += 1;
      latestWidth = Math.max(minWidth, startWidth + moveEvent.clientX - startX);
      grid?.style.setProperty(cssVariable, `${latestWidth}px`);
    };
    const onUp = () => {
      setColumns((current) => ({
        ...current,
        [column]: latestWidth,
      }));
      const elapsed = Math.round(window.performance.now() - startedAt);
      if (elapsed >= 250 || moveCount >= 10) {
        console.info(`[perf] translation_column_resize ${column} ${elapsed}ms moves=${moveCount}`);
      }
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
    };
    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
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
    setActiveProjectPath(null);
    setEntrySearch("");
    setValidationIssueKindFilter(null);
    setEntryFilter(targetEntry?.status === "removed" ? "removed" : "all");
    if (dllTextKeys && !dllTextKeys.has(key)) {
      setDllTextOnly(false);
    }
    setFocusEntryInput(true);
    setFocusEntryKey(key);
    setFocusRequest((request) => request + 1);
  }

  /**
   * Goes to a row from the warning list without leaving it: the warning
   * filter (and the list) stays, so the next warning is one click away.
   */
  function revealWarningEntry(key: string) {
    if (entryFilter !== "warning" || !validationWarningKeys.has(key)) {
      revealEntryKey(key);
      return;
    }
    setActiveProjectPath(null);
    setEntrySearch("");
    if (dllTextKeys && !dllTextKeys.has(key)) {
      setDllTextOnly(false);
    }
    setFocusEntryInput(true);
    setFocusEntryKey(key);
    setFocusRequest((request) => request + 1);
  }

  /** Scrolls to a row changed by undo/redo, keeping filters when it is visible. */
  function showChangedEntry(key: string) {
    if (!filteredEntries.some(({ entry }) => entry.key === key)) {
      revealEntryKey(key);
    }
    setFocusEntryInput(false);
    setFocusEntryKey(key);
    setFocusRequest((request) => request + 1);
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
    props.onReplaceEntries(updates, `찾아 바꾸기 ${updates.length}개`);
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
    dllTextOnly,
    setDllTextOnly,
    dllTextCount: dllTextKeys?.size ?? null,
    dllTextKeys,
    dllUses,
    dllSeenKeys,
    dllPartialKeys,
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
    replaceMatchCount,
    startColumnResize,
    showEntryCondition,
    showValidationIssueKind,
    replaceTranslatedValues,
    copyTreeJson,
    focusEntryKey,
    focusRequest,
    focusEntryInput,
    revealWarningEntry,
    revealEntryKey,
    showChangedEntry,
    clearFocusEntryKey: () => setFocusEntryKey(null),
  };
}

function columnCssVariable(column: TranslationColumnKey): string {
  switch (column) {
    case "status":
      return "--col-status";
    case "id":
      return "--col-id";
    case "file":
      return "--col-file";
    case "key":
      return "--col-key";
    case "source":
      return "--col-source";
    case "compare":
      return "--col-compare";
    case "translated":
      return "--col-translated";
  }
}

function cachedSplitSheetKey(
  cache: Map<string, ReturnType<typeof splitSheetKey>>,
  key: string,
): ReturnType<typeof splitSheetKey> {
  const cached = cache.get(key);
  if (cached) {
    return cached;
  }
  const parts = splitSheetKey(key);
  cache.set(key, parts);
  return parts;
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
  for (const issue of [...(validation.format_issues ?? []), ...(validation.glossary_issues ?? [])]) {
    if (!kind || issue.kind === kind) {
      keys.add(issue.key);
    }
  }
  return keys;
}
