import React from "react";
import { invokeCommand } from "../../api/tauri";
import type { JsonTranslationSheet, TranslationSaveTarget, TranslatorSave, TranslatorSyncItem, TranslatorSyncPreview } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { TranslatorSyncModal } from "./TranslatorSyncModal";
import type { SheetSaveState } from "./useTranslationSessionState";

export type ApplyOptions = { translatorKeepSheetKeys?: string[] };

/**
 * Shows where "적용" sends the sheet (Sts2ModTranslator overrides or a patch
 * mod), lets the user force one for this mod, and surfaces values the player
 * changed in game so they can be pulled into the sheet before saving.
 */
export function SaveTargetSelect({
  sheetPath,
  sheet,
  busy,
  saveState,
  onReplaceEntries,
  onApply,
}: {
  sheetPath: string;
  sheet: JsonTranslationSheet | null;
  busy: string | null;
  saveState: SheetSaveState;
  onReplaceEntries: (updates: Array<{ index: number; value: string }>, label?: string) => void;
  onApply: (options?: ApplyOptions) => void;
}) {
  const [lastSync, setLastSync] = React.useState<{ at: Date; result: TranslatorSave | null; error?: string } | null>(null);
  const previousSaveStateRef = React.useRef(saveState);
  const [target, setTarget] = React.useState<TranslationSaveTarget | null>(null);
  const [preview, setPreview] = React.useState<TranslatorSyncPreview | null>(null);
  const [modalOpen, setModalOpen] = React.useState(false);
  // Apply waits one render after the imported values land in the sheet, so
  // the save that apply starts with already contains them.
  const [pendingApply, setPendingApply] = React.useState<ApplyOptions | null>(null);
  const idle = !busy;
  const sheetRef = React.useRef(sheet);
  sheetRef.current = sheet;

  React.useEffect(() => {
    if (!sheetPath || !idle || isPreviewRuntime()) {
      return;
    }
    let cancelled = false;
    invokeCommand("translation_save_target", { sheetPath })
      .then((result) => {
        if (!cancelled) setTarget(result);
      })
      .catch(() => {
        if (!cancelled) setTarget(null);
      });
    return () => {
      cancelled = true;
    };
  }, [sheetPath, idle]);

  const loadPreview = React.useCallback(async () => {
    const current = sheetRef.current;
    if (!current || isPreviewRuntime()) {
      return null;
    }
    try {
      const result = await invokeCommand("translator_sync_preview", { sheet: current });
      setPreview(result);
      return result;
    } catch {
      setPreview(null);
      return null;
    }
  }, []);

  React.useEffect(() => {
    if (target?.destination === "translator" && idle) {
      void loadPreview();
    } else {
      setPreview(null);
    }
  }, [target?.destination, idle, sheetPath, loadPreview]);

  // Each finished save of a sheet that goes to Translator also writes it
  // there (unless turned off), so the game picks it up on the next Reload.
  React.useEffect(() => {
    const previous = previousSaveStateRef.current;
    previousSaveStateRef.current = saveState;
    if (previous !== "saving" || saveState !== "saved" || target?.destination !== "translator" || !target.auto_sync) {
      return;
    }
    invokeCommand("translator_auto_sync", { sheetPath })
      .then((result) => {
        setLastSync({ at: new Date(), result });
        void loadPreview();
      })
      .catch((error) => setLastSync({ at: new Date(), result: null, error: String(error) }));
  }, [saveState, sheetPath, target?.destination, target?.auto_sync, loadPreview]);

  React.useEffect(() => {
    if (pendingApply) {
      setPendingApply(null);
      onApply(pendingApply);
    }
  }, [pendingApply, sheet, onApply]);

  if (!target) {
    return null;
  }

  async function changeMode(mode: string) {
    try {
      setTarget(await invokeCommand("set_translation_save_mode", { sheetPath, mode }));
    } catch {
      // The selector keeps the previous state; the reason tooltip explains failures on save.
    }
  }

  async function toggleAutoSync(enabled: boolean) {
    try {
      const saved = await invokeCommand("set_translator_auto_sync", { enabled });
      setTarget((current) => (current ? { ...current, auto_sync: saved } : current));
    } catch {
      // Keeps the previous setting.
    }
  }

  async function openModal() {
    const fresh = await loadPreview();
    if (fresh && fresh.items.length > 0) {
      setModalOpen(true);
    }
  }

  function resolve(gameItems: TranslatorSyncItem[], sheetItems: TranslatorSyncItem[]) {
    setModalOpen(false);
    const entries = sheetRef.current?.entries ?? [];
    const indexByKey = new Map(entries.map((entry, index) => [entry.key, index]));
    const updates = gameItems.flatMap((item) =>
      item.sheet_keys
        .map((key) => indexByKey.get(key))
        .filter((index): index is number => index !== undefined)
        .map((index) => ({ index, value: item.theirs })),
    );
    onReplaceEntries(updates, `Translator 게임 안 변경 가져오기 ${gameItems.length}개`);
    setPendingApply({ translatorKeepSheetKeys: sheetItems.flatMap((item) => item.sheet_keys) });
  }

  const destinationLabel = target.destination === "translator" ? "Translator" : "패치 모드";
  const tooltip = [
    target.reason,
    target.unsupported_entries > 0 ? `Translator 미지원 항목 ${target.unsupported_entries}개(DLL 문자열 등)` : "",
    lastSync ? autoSyncLabel(lastSync) : "",
  ]
    .filter(Boolean)
    .join(" · ");
  const changedCount = preview?.items.length ?? 0;
  const conflictCount = preview?.items.filter((item) => item.kind === "conflict").length ?? 0;
  return (
    <>
      <label className={`save-target-select ${target.destination}`} data-tooltip={tooltip} aria-label={`저장 위치: ${destinationLabel}`}>
        <span>저장 위치</span>
        <select value={target.mode} onChange={(event) => void changeMode(event.target.value)} disabled={Boolean(busy)}>
          <option value="auto">자동 · {destinationLabel}</option>
          <option value="translator">Translator</option>
          <option value="patch">패치 모드</option>
        </select>
      </label>
      {target.destination === "translator" && (
        <label className="translator-auto-sync" data-tooltip="시트를 저장할 때마다 Translator에도 바로 씁니다. 게임 안 값과 충돌하면 쓰지 않습니다.">
          <input type="checkbox" checked={target.auto_sync} onChange={(event) => void toggleAutoSync(event.target.checked)} disabled={Boolean(busy)} />
          <span>저장 시 자동 반영</span>
        </label>
      )}
      {changedCount > 0 && (
        <button
          type="button"
          className={`translator-sync-badge${conflictCount > 0 ? " conflict" : ""}`}
          data-tooltip={conflictCount > 0 ? `양쪽에서 바뀐 항목 ${conflictCount}개는 적용 전에 골라야 합니다` : "게임 안에서 고친 값을 시트로 가져옵니다"}
          onClick={() => void openModal()}
          disabled={Boolean(busy)}
        >
          게임 안 변경 {changedCount}
        </button>
      )}
      {modalOpen && preview && <TranslatorSyncModal preview={preview} onCancel={() => setModalOpen(false)} onConfirm={resolve} />}
    </>
  );
}

function autoSyncLabel(sync: { at: Date; result: TranslatorSave | null; error?: string }): string {
  const time = sync.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (sync.error) {
    return `자동 반영 실패 (${time}): ${sync.error}`;
  }
  if (!sync.result) {
    return "";
  }
  if (sync.result.conflicts.length > 0) {
    return `자동 반영 보류 (${time}): 게임 안 값과 다른 항목 ${sync.result.conflicts.length}개`;
  }
  return `자동 반영 (${time}): ${sync.result.written_entries}개 변경`;
}
