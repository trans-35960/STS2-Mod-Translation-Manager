import React from "react";
import {
  compactTranslationFile,
  containsLegacyTranslationShortId,
  hasTranslationValue,
  isLegacyTranslationShortId,
  isTabularTranslationPaste,
  isTranslationSlotId,
  looksLikeJsonPaste,
  parsePastedTranslationJson,
  splitSheetKey,
  structuredTranslationEntries,
  translationSlotEntries,
  translationSlotKey,
} from "../../features/translation/translationUtils";
import type { JsonTranslationEntry, PasteCandidate } from "../../types";
import type { TranslationActionsParams } from "./types";

export function useTranslationSheetActions({
  appendLog,
  jsonSheet,
  page,
  pasteCandidatesByKey,
  selectedRows,
  setJsonApplyResult,
  setJsonSheet,
  setPasteCandidatesByKey,
  setSelectedRows,
}: TranslationActionsParams) {
  function updateTranslationEntry(index: number, value: string) {
    setJsonApplyResult(null);
    const entryKey = jsonSheet?.entries[index]?.key;
    if (entryKey) {
      setPasteCandidatesByKey((candidates) => {
        const next = { ...candidates };
        delete next[entryKey];
        return next;
      });
    }
    setJsonSheet((current) => {
      if (!current) {
        return current;
      }
      const entries = current.entries.map((entry, entryIndex) => {
        if (entryIndex !== index) {
          return entry;
        }
        return {
          ...entry,
          translated_value: value,
          status: editedTranslationStatus(entry, value),
        } satisfies JsonTranslationEntry;
      });
      return { ...current, entries };
    });
  }

  function replaceTranslationEntries(updates: Array<{ index: number; value: string }>) {
    if (updates.length === 0) {
      return;
    }
    setJsonApplyResult(null);
    const updateByIndex = new Map(updates.map((update) => [update.index, update.value]));
    const affectedKeys = new Set(
      (jsonSheet?.entries ?? [])
        .filter((_, index) => updateByIndex.has(index))
        .map((entry) => entry.key),
    );
    setJsonSheet((current) => {
      if (!current) {
        return current;
      }
      const entries = current.entries.map((entry, index) => {
        const value = updateByIndex.get(index);
        if (value === undefined) {
          return entry;
        }
        return {
          ...entry,
          translated_value: value,
          status: editedTranslationStatus(entry, value),
        } satisfies JsonTranslationEntry;
      });
      return { ...current, entries };
    });
    if (affectedKeys.size > 0) {
      setPasteCandidatesByKey((candidates) => {
        const next = { ...candidates };
        for (const key of affectedKeys) {
          delete next[key];
        }
        return next;
      });
    }
    appendLog(`치환 완료: ${updates.length}개 항목`);
  }

  function pasteTranslationValues(startIndex: number, text: string) {
    if (!isTabularTranslationPaste(text)) {
      appendLog("표 형식이 아니라 현재 셀에만 붙여넣습니다.");
      return;
    }
    const values = text
      .replace(/\r/g, "")
      .split("\n")
      .filter((line, index, lines) => line.length > 0 || index < lines.length - 1)
      .map((line) => {
        const columns = line.split("\t");
        return columns[columns.length - 1] ?? "";
      });
    if (values.length === 0) {
      return;
    }
    setJsonApplyResult(null);
    const valuesByKey = new Map<string, string>();
    const pasteConflicts: Record<string, PasteCandidate> = {};
    const affectedKeys = new Set<string>();
    for (const [offset, entry] of (jsonSheet?.entries ?? []).entries()) {
      const pasted = values[offset - startIndex];
      if (pasted === undefined) {
        continue;
      }
      affectedKeys.add(entry.key);
      if (hasTranslationValue(entry.translated_value) && entry.translated_value !== pasted) {
        pasteConflicts[entry.key] = { value: pasted, source: "표 붙여넣기" };
        continue;
      }
      valuesByKey.set(entry.key, pasted);
    }
    setJsonSheet((current) => {
      if (!current) {
        return current;
      }
      const entries = current.entries.map((entry) => {
        const pasted = valuesByKey.get(entry.key);
        if (pasted === undefined) {
          return entry;
        }
        return {
          ...entry,
          translated_value: pasted,
          status: editedTranslationStatus(entry, pasted),
        } satisfies JsonTranslationEntry;
      });
      return { ...current, entries };
    });
    if (affectedKeys.size > 0) {
      setPasteCandidatesByKey((current) => {
        const next = { ...current };
        for (const key of affectedKeys) {
          if (!Object.prototype.hasOwnProperty.call(pasteConflicts, key)) {
            delete next[key];
          }
        }
        Object.assign(next, pasteConflicts);
        return next;
      });
    }
    const candidateCount = Object.keys(pasteConflicts).length;
    const appliedCount = valuesByKey.size;
    appendLog(`표 붙여넣기 완료: ${appliedCount}개 적용${candidateCount > 0 ? ` / ${candidateCount}개 충돌 후보` : ""}`);
  }

  function pasteStructuredTranslationJson(text: string): boolean {
    if (!jsonSheet) {
      return false;
    }
    const parsed = parsePastedTranslationJson(text);
    if (parsed === null) {
      if (looksLikeJsonPaste(text)) {
        appendLog("JSON 붙여넣기 실패: JSON 형식이 올바르지 않습니다.");
        return true;
      }
      return false;
    }
    const entries = structuredTranslationEntries(parsed);
    if (entries.length === 0) {
      if (containsLegacyTranslationShortId(parsed)) {
        appendLog("이전 short-id JSON은 새 slot-id 검증을 지원하지 않습니다. 번역용 JSON을 다시 내보내세요.");
        return true;
      }
      appendLog("JSON 붙여넣기 실패: 매칭할 번역 slot id(k001-a1 형식)를 찾지 못했습니다. 번역용 JSON 내보내기 파일을 사용하세요.");
      return true;
    }
    const slotEntries = translationSlotEntries(jsonSheet);
    const entryBySlot = new Map(slotEntries.map((slot) => [translationSlotKey(slot.compactFile, slot.id), slot.entry]));
    const entriesById = new Map<string, JsonTranslationEntry[]>();
    for (const slot of slotEntries) {
      entriesById.set(slot.id, [...(entriesById.get(slot.id) ?? []), slot.entry]);
    }
    const values = new Map<string, string>();
    const candidates: Record<string, PasteCandidate> = {};
    const unmatchedIds: string[] = [];
    const invalidIds: string[] = [];
    const emptyIds: string[] = [];
    const duplicateIds: string[] = [];
    const seenEntryKeys = new Set<string>();
    for (const entry of entries) {
      if (entry.id && entry.translated_value !== undefined) {
        if (isLegacyTranslationShortId(entry.id)) {
          invalidIds.push(entry.id);
          continue;
        }
        if (!isTranslationSlotId(entry.id)) {
          unmatchedIds.push(entry.id);
          continue;
        }
        if (!hasTranslationValue(entry.translated_value)) {
          emptyIds.push(entry.id);
          continue;
        }
        const sheetEntry = entry.source
          ? entryBySlot.get(translationSlotKey(compactTranslationFile(entry.source), entry.id))
          : (entriesById.get(entry.id)?.length === 1 ? entriesById.get(entry.id)?.[0] : undefined);
        if (!sheetEntry) {
          unmatchedIds.push(entry.source ? `${entry.source}:${entry.id}` : entry.id);
          continue;
        }
        if (seenEntryKeys.has(sheetEntry.key)) {
          duplicateIds.push(entry.source ? `${entry.source}:${entry.id}` : entry.id);
          continue;
        }
        seenEntryKeys.add(sheetEntry.key);
        if (hasTranslationValue(sheetEntry?.translated_value ?? "") && sheetEntry.translated_value !== entry.translated_value) {
          candidates[sheetEntry.key] = { value: entry.translated_value, source: entry.source || "붙여넣기 JSON" };
          continue;
        }
        values.set(sheetEntry.key, entry.translated_value);
      }
    }
    if (invalidIds.length > 0) {
      appendLog("이전 short-id JSON은 새 slot-id 검증을 지원하지 않습니다. 번역용 JSON을 다시 내보내세요.");
      return true;
    }
    if (emptyIds.length > 0) {
      appendLog(`빈 번역값이 있는 slot id: ${emptyIds.slice(0, 8).join(", ")}${emptyIds.length > 8 ? " ..." : ""}`);
      return true;
    }
    if (duplicateIds.length > 0) {
      appendLog(`중복 slot id: ${duplicateIds.slice(0, 8).join(", ")}${duplicateIds.length > 8 ? " ..." : ""}`);
      return true;
    }
    if (unmatchedIds.length > 0 && values.size === 0 && Object.keys(candidates).length === 0) {
      appendLog(`JSON 붙여넣기 실패: 현재 시트와 매칭되는 id가 없습니다. 미매칭 id: ${unmatchedIds.slice(0, 8).join(", ")}${unmatchedIds.length > 8 ? " ..." : ""}`);
      return true;
    }
    if (values.size === 0 && Object.keys(candidates).length === 0) {
      appendLog("JSON 붙여넣기 실패: 적용할 번역값이 없습니다.");
      return true;
    }
    setJsonApplyResult(null);
    setPasteCandidatesByKey((current) => ({ ...current, ...candidates }));
    const updatedCount = jsonSheet.entries.filter((entry) => {
      const value = values.get(entry.key);
      return value !== undefined && entry.translated_value !== value;
    }).length;
    const unchangedCount = jsonSheet.entries.filter((entry) => {
      const value = values.get(entry.key);
      return value !== undefined && entry.translated_value === value;
    }).length;
    setJsonSheet((current) => {
      if (!current) {
        return current;
      }
      const updatedEntries = current.entries.map((entry) => {
        const value = values.get(entry.key);
        if (value === undefined) {
          return entry;
        }
        if (entry.translated_value === value) {
          return entry;
        }
        return {
          ...entry,
          translated_value: value,
          status: editedTranslationStatus(entry, value),
        } satisfies JsonTranslationEntry;
      });
      return { ...current, entries: updatedEntries };
    });
    const candidateCount = Object.keys(candidates).length;
    const summaryParts = [`${updatedCount}개 적용`];
    if (unchangedCount > 0) {
      summaryParts.push(`${unchangedCount}개 동일`);
    }
    if (candidateCount > 0) {
      summaryParts.push(`${candidateCount}개 충돌 후보`);
    }
    if (unmatchedIds.length > 0) {
      summaryParts.push(`${unmatchedIds.length}개 id 미매칭`);
    }
    appendLog(`JSON 붙여넣기 완료: ${summaryParts.join(" / ")}`);
    if (unmatchedIds.length > 0) {
      appendLog(`미매칭 id: ${unmatchedIds.slice(0, 8).join(", ")}${unmatchedIds.length > 8 ? " ..." : ""}`);
    }
    return true;
  }

  React.useEffect(() => {
    if (page !== "translationTools" || !jsonSheet) {
      return;
    }
    const handlePaste = (event: ClipboardEvent) => {
      const text = event.clipboardData?.getData("text/plain") ?? "";
      if (!text || parsePastedTranslationJson(text) === null) {
        return;
      }
      if (pasteStructuredTranslationJson(text)) {
        event.preventDefault();
        event.stopPropagation();
        event.stopImmediatePropagation();
      }
    };
    window.addEventListener("paste", handlePaste, { capture: true });
    return () => window.removeEventListener("paste", handlePaste, { capture: true });
  }, [jsonSheet, page]);

  function applyPasteCandidates(entryKeys: string[], logBulk = false) {
    const entriesToApply = entryKeys
      .map((entryKey) => ({ entryKey, candidate: pasteCandidatesByKey[entryKey] }))
      .filter((item): item is { entryKey: string; candidate: PasteCandidate } => Boolean(item.candidate));
    if (entriesToApply.length === 0) {
      return;
    }
    const candidateByKey = new Map(entriesToApply.map((item) => [item.entryKey, item.candidate]));
    setJsonApplyResult(null);
    setJsonSheet((current) => {
      if (!current) {
        return current;
      }
      return {
        ...current,
        entries: current.entries.map((entry) => {
          const candidate = candidateByKey.get(entry.key);
          if (!candidate) {
            return entry;
          }
          return {
            ...entry,
            translated_value: candidate.value,
            status: editedTranslationStatus(entry, candidate.value),
          };
        }),
      };
    });
    setPasteCandidatesByKey((current) => {
      const next = { ...current };
      for (const entryKey of candidateByKey.keys()) {
        delete next[entryKey];
      }
      return next;
    });
    if (logBulk) {
      appendLog(`붙여넣기 충돌 후보 ${entriesToApply.length}개를 모두 적용했습니다.`);
    }
  }

  function applyPasteCandidate(entryKey: string) {
    applyPasteCandidates([entryKey]);
  }

  function applyAllPasteCandidates() {
    applyPasteCandidates(Object.keys(pasteCandidatesByKey), true);
  }

  function dismissPasteCandidates(entryKeys: string[], logBulk = false) {
    const entriesToDismiss = entryKeys.filter((entryKey) => Boolean(pasteCandidatesByKey[entryKey]));
    if (entriesToDismiss.length === 0) {
      return;
    }
    setPasteCandidatesByKey((current) => {
      const next = { ...current };
      for (const entryKey of entriesToDismiss) {
        if (next[entryKey]) {
          delete next[entryKey];
        }
      }
      return next;
    });
    if (logBulk) {
      appendLog(`붙여넣기 충돌 후보 ${entriesToDismiss.length}개를 모두 취소했습니다.`);
    }
  }

  function dismissPasteCandidate(entryKey: string) {
    dismissPasteCandidates([entryKey]);
  }

  function dismissAllPasteCandidates() {
    dismissPasteCandidates(Object.keys(pasteCandidatesByKey), true);
  }

  function selectTranslationRow(index: number, event: React.MouseEvent) {
    if (event.shiftKey && selectedRows.length > 0) {
      const anchor = selectedRows[0];
      const start = Math.min(anchor, index);
      const end = Math.max(anchor, index);
      setSelectedRows(Array.from({ length: end - start + 1 }, (_, offset) => start + offset));
      return;
    }
    if (event.ctrlKey || event.metaKey) {
      setSelectedRows((current) =>
        current.includes(index) ? current.filter((item) => item !== index) : [...current, index].sort((a, b) => a - b),
      );
      return;
    }
    setSelectedRows([index]);
  }

  async function copySelectedTranslations() {
    if (!jsonSheet || selectedRows.length === 0) {
      return;
    }
    const text = selectedRows
      .map((index) => jsonSheet.entries[index])
      .filter(Boolean)
      .map((entry) => {
        const parts = splitSheetKey(entry.key);
        return [parts.file, parts.key, entry.source_value, entry.translated_value].join("\t");
      })
      .join("\n");
    await navigator.clipboard?.writeText(text);
    appendLog(`${selectedRows.length}개 행 복사 완료`);
  }

  return {
    applyAllPasteCandidates,
    applyPasteCandidate,
    copySelectedTranslations,
    dismissAllPasteCandidates,
    dismissPasteCandidate,
    pasteStructuredTranslationJson,
    pasteTranslationValues,
    replaceTranslationEntries,
    selectTranslationRow,
    updateTranslationEntry,
  };
}

function editedTranslationStatus(entry: JsonTranslationEntry, value: string): JsonTranslationEntry["status"] {
  if (entry.status === "removed") {
    return "removed";
  }
  if (!hasTranslationValue(value)) {
    return "missing";
  }
  if (entry.status === "new" || entry.status === "updated") {
    return entry.status;
  }
  return "ready";
}
