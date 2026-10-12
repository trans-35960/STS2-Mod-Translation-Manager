import React from "react";
import { invokeCommand } from "../../api/tauri";
import type { JsonTranslationSheet, TranslationSaveTarget, TranslatorSyncItem, TranslatorSyncPreview } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";
import { TranslatorSyncModal } from "./TranslatorSyncModal";

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
  onReplaceEntries,
  onApply,
}: {
  sheetPath: string;
  sheet: JsonTranslationSheet | null;
  busy: string | null;
  onReplaceEntries: (updates: Array<{ index: number; value: string }>, label?: string) => void;
  onApply: (options?: ApplyOptions) => void;
}) {
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
