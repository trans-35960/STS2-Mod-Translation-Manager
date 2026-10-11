import React from "react";
import {
  compactTranslationFile,
  hasTranslationValue,
  isTabularTranslationPaste,
  isStructuredTranslationJsonPaste,
  isTranslationSlotId,
  normalizeSlotId,
  parsePastedTranslationJson,
  slotIdIndex,
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
        // Keep the same object when there is nothing to drop, so every edit
        // does not invalidate the sheet's filtered view.
        if (!(entryKey in candidates)) {
          return candidates;
        }
        const next = { ...candidates };
        delete next[entryKey];
        return next;
      });
    }
    setJsonSheet((current) => {
      if (!current) {
        return current;
      }
      const entry = current.entries[index];
      if (!entry) {
        return current;
      }
      const entries = current.entries.slice();
      entries[index] = {
        ...entry,
        translated_value: value,
        status: editedTranslationStatus(entry, value),
      } satisfies JsonTranslationEntry;
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
      return false;
    }
    const entries = structuredTranslationEntries(parsed);
    if (entries.length === 0) {
      appendLog("JSON 붙여넣기 실패: 매칭할 번역 slot id(k001-a1 형식)를 찾지 못했습니다. 번역용 JSON 내보내기 파일을 사용하세요.");
      return true;
    }
    const slotEntries = translationSlotEntries(jsonSheet);
    // Ids are matched loosely (case, zero padding) because AI answers often
    // reformat them.
    const slotKeyOf = (file: string, id: string) => translationSlotKey(file, normalizeSlotId(id) ?? id);
    const entryBySlot = new Map(slotEntries.map((slot) => [slotKeyOf(slot.compactFile, slot.id), slot.entry]));
    const entriesById = new Map<string, JsonTranslationEntry[]>();
    for (const slot of slotEntries) {
      const id = normalizeSlotId(slot.id) ?? slot.id;
      entriesById.set(id, [...(entriesById.get(id) ?? []), slot.entry]);
    }
    const pastedIndexesByFile = new Map<string, Set<number>>();
    const values = new Map<string, string>();
    const candidates: Record<string, PasteCandidate> = {};
    const unmatchedIds: string[] = [];
    const emptyIds: string[] = [];
    const duplicateIds: string[] = [];
    const seenEntryKeys = new Set<string>();
    for (const entry of entries) {
      if (entry.id && entry.translated_value !== undefined) {
        if (!isTranslationSlotId(entry.id)) {
          unmatchedIds.push(entry.id);
          continue;
        }
        if (!hasTranslationValue(entry.translated_value)) {
          emptyIds.push(entry.id);
          continue;
        }
        const normalizedId = normalizeSlotId(entry.id) ?? entry.id;
        const sheetEntry = entry.source
          ? entryBySlot.get(slotKeyOf(compactTranslationFile(entry.source), entry.id))
          : (entriesById.get(normalizedId)?.length === 1 ? entriesById.get(normalizedId)?.[0] : undefined);
        if (sheetEntry) {
          const file = entry.source ? compactTranslationFile(entry.source) : "";
          const index = slotIdIndex(entry.id);
          if (index !== null) {
            pastedIndexesByFile.set(file, (pastedIndexesByFile.get(file) ?? new Set()).add(index));
          }
        }
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
    const skippedNotes = [
      emptyIds.length > 0 ? `빈 번역값 ${emptyIds.length}개: ${previewIds(emptyIds)}` : "",
      duplicateIds.length > 0 ? `중복 id ${duplicateIds.length}개: ${previewIds(duplicateIds)}` : "",
      unmatchedIds.length > 0 ? `시트에 없는 id ${unmatchedIds.length}개: ${previewIds(unmatchedIds)}` : "",
    ].filter(Boolean);
    if (values.size === 0 && Object.keys(candidates).length === 0) {
      appendLog(`JSON 붙여넣기 실패: 적용할 번역값이 없습니다.${skippedNotes.length > 0 ? ` ${skippedNotes.join(" / ")}` : ""}`);
      return true;
    }
    // Ids the answer skipped: empty entries inside the pasted id range of a
    // file, which is how an AI answer for one prompt part usually looks.
    const answeredKeys = new Set([...values.keys(), ...Object.keys(candidates)]);
    const answeredIds = new Set(entries.map((entry) => (entry.id ? normalizeSlotId(entry.id) : null)).filter(Boolean));
    const missingIds: string[] = [];
    for (const slot of slotEntries) {
      const indexes = pastedIndexesByFile.get(slot.compactFile) ?? pastedIndexesByFile.get("");
      const index = slotIdIndex(slot.id);
      if (
        !indexes ||
        index === null ||
        answeredKeys.has(slot.entry.key) ||
        answeredIds.has(normalizeSlotId(slot.id)) ||
        slot.entry.translated_value.trim()
      ) {
        continue;
      }
      const sorted = [...indexes];
      if (index > Math.min(...sorted) && index < Math.max(...sorted)) {
        missingIds.push(`${slot.compactFile}:${slot.id}`);
      }
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
    if (emptyIds.length + duplicateIds.length > 0) {
      summaryParts.push(`${emptyIds.length + duplicateIds.length}개 건너뜀`);
    }
    if (missingIds.length > 0) {
      summaryParts.push(`답변에서 빠진 id ${missingIds.length}개`);
    }
    appendLog(`JSON 붙여넣기 완료: ${summaryParts.join(" / ")}`);
    for (const note of skippedNotes) {
      appendLog(`건너뜀 - ${note}`);
    }
    if (missingIds.length > 0) {
      appendLog(`답변에서 빠진 id (같은 범위의 빈 항목): ${previewIds(missingIds)}`);
    }
    return true;
  }

  React.useEffect(() => {
    if (page !== "translationTools" || !jsonSheet) {
      return;
    }
    const handlePaste = (event: ClipboardEvent) => {
      // Inputs that handle JSON answers themselves (the AI prompt dialog).
      if (event.target instanceof Element && event.target.closest("[data-own-paste]")) {
        return;
      }
      const text = event.clipboardData?.getData("text/plain") ?? "";
      if (!text || !isStructuredTranslationJsonPaste(text)) {
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

function previewIds(ids: string[]) {
  return `${ids.slice(0, 8).join(", ")}${ids.length > 8 ? " ..." : ""}`;
}
