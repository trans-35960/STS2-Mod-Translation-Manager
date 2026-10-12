import React from "react";
import { invokeCommand } from "../../api/tauri";
import type { TranslationSaveTarget } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";

/**
 * Shows where "적용" sends the sheet (Sts2ModTranslator overrides or a patch
 * mod) and lets the user force one for this mod.
 */
export function SaveTargetSelect({ sheetPath, busy }: { sheetPath: string; busy: string | null }) {
  const [target, setTarget] = React.useState<TranslationSaveTarget | null>(null);
  const idle = !busy;

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

  if (!target) {
    return null;
  }

  async function changeMode(mode: string) {
    try {
      setTarget(await invokeCommand("set_translation_save_mode", { sheetPath, mode }));
    } catch {
      // The selector simply keeps the previous state; the reason tooltip explains failures on save.
    }
  }

  const destinationLabel = target.destination === "translator" ? "Translator" : "패치 모드";
  const tooltip = [
    target.reason,
    target.unsupported_entries > 0 ? `Translator 미지원 항목 ${target.unsupported_entries}개(DLL 문자열 등)` : "",
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <label className={`save-target-select ${target.destination}`} data-tooltip={tooltip} aria-label={`저장 위치: ${destinationLabel}`}>
      <span>저장 위치</span>
      <select value={target.mode} onChange={(event) => void changeMode(event.target.value)} disabled={Boolean(busy)}>
        <option value="auto">자동 · {destinationLabel}</option>
        <option value="translator">Translator</option>
        <option value="patch">패치 모드</option>
      </select>
    </label>
  );
}
