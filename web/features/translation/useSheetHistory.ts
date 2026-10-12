import React from "react";
import type { JsonTranslationSheet } from "../../types";

type Sheet = JsonTranslationSheet | null;
type HistoryStep = { sheet: JsonTranslationSheet; label: string; mergeKey?: string };

const HISTORY_LIMIT = 100;
/** Edits of the same cell this close together are one undo step. */
const MERGE_WINDOW_MS = 4000;

export type SheetHistoryState = {
  undoLabel: string | null;
  redoLabel: string | null;
  /** The sheet differs from how it was when this work session started. */
  changedSinceStart: boolean;
};

export type SheetHistoryMove = { label: string; changedKeys: string[] };

/**
 * Undo/redo for the translation sheet. Every edit (a functional update or a
 * draft) records the previous sheet; a value from the backend (load, create,
 * recalculate) starts a new history. Sheets are immutable and share their
 * unchanged entries, so a step costs one entries array.
 */
export function useSheetHistory(setSheetState: React.Dispatch<React.SetStateAction<Sheet>>) {
  const sheetRef = React.useRef<Sheet>(null);
  const startRef = React.useRef<Sheet>(null);
  const undoRef = React.useRef<HistoryStep[]>([]);
  const redoRef = React.useRef<HistoryStep[]>([]);
  const pendingRef = React.useRef<{ label: string; mergeKey?: string } | null>(null);
  const lastEditRef = React.useRef<{ mergeKey: string; at: number } | null>(null);
  const [state, setState] = React.useState<SheetHistoryState>({ undoLabel: null, redoLabel: null, changedSinceStart: false });

  const sync = React.useCallback(() => {
    setState({
      undoLabel: lastOf(undoRef.current)?.label ?? null,
      redoLabel: lastOf(redoRef.current)?.label ?? null,
      changedSinceStart: Boolean(startRef.current && sheetRef.current && startRef.current !== sheetRef.current),
    });
  }, []);

  const show = React.useCallback(
    (next: Sheet) => {
      sheetRef.current = next;
      setSheetState(next);
    },
    [setSheetState],
  );

  /** Applies an edit and records the sheet it replaces. */
  const edit = React.useCallback(
    (next: Sheet, fallbackLabel: string) => {
      const previous = sheetRef.current;
      if (next === previous) {
        return;
      }
      const pending = pendingRef.current;
      pendingRef.current = null;
      if (previous && next) {
        const label = pending?.label ?? fallbackLabel;
        const mergeKey = pending?.mergeKey;
        const now = Date.now();
        const last = lastEditRef.current;
        const merges = Boolean(mergeKey && last && last.mergeKey === mergeKey && now - last.at < MERGE_WINDOW_MS && undoRef.current.length > 0);
        if (!merges) {
          undoRef.current = [...undoRef.current.slice(-(HISTORY_LIMIT - 1)), { sheet: previous, label, mergeKey }];
        }
        lastEditRef.current = mergeKey ? { mergeKey, at: now } : null;
        redoRef.current = [];
      }
      show(next);
      sync();
    },
    [show, sync],
  );

  /** A sheet from the backend: the new baseline, with an empty history. */
  const reset = React.useCallback(
    (next: Sheet) => {
      undoRef.current = [];
      redoRef.current = [];
      lastEditRef.current = null;
      pendingRef.current = null;
      startRef.current = next;
      show(next);
      sync();
    },
    [show, sync],
  );

  /** Names the next edit in the history (and lets edits of one cell merge). */
  const labelNextEdit = React.useCallback((label: string, mergeKey?: string) => {
    pendingRef.current = { label, mergeKey };
  }, []);

  const move = React.useCallback(
    (from: React.MutableRefObject<HistoryStep[]>, to: React.MutableRefObject<HistoryStep[]>): SheetHistoryMove | null => {
      const step = lastOf(from.current);
      const current = sheetRef.current;
      if (!step || !current) {
        return null;
      }
      from.current = from.current.slice(0, -1);
      to.current = [...to.current, { sheet: current, label: step.label }];
      lastEditRef.current = null;
      show(step.sheet);
      sync();
      return { label: step.label, changedKeys: changedEntryKeys(current, step.sheet) };
    },
    [show, sync],
  );

  const undo = React.useCallback(() => move(undoRef, redoRef), [move]);
  const redo = React.useCallback(() => move(redoRef, undoRef), [move]);

  /** Goes back to the sheet as it was when the session started, as one undoable step. */
  const revertToStart = React.useCallback((): SheetHistoryMove | null => {
    const start = startRef.current;
    const current = sheetRef.current;
    if (!start || !current || start === current) {
      return null;
    }
    const label = "작업 시작 시점으로 되돌리기";
    pendingRef.current = { label };
    edit(start, label);
    return { label, changedKeys: changedEntryKeys(current, start) };
  }, [edit]);

  return { sheetRef, state, edit, reset, labelNextEdit, undo, redo, revertToStart };
}

function changedEntryKeys(left: JsonTranslationSheet, right: JsonTranslationSheet) {
  const rightByKey = new Map(right.entries.map((entry) => [entry.key, entry]));
  return left.entries
    .filter((entry) => {
      const other = rightByKey.get(entry.key);
      return other !== entry && other?.translated_value !== entry.translated_value;
    })
    .map((entry) => entry.key);
}

function lastOf<T>(items: T[]): T | undefined {
  return items[items.length - 1];
}
