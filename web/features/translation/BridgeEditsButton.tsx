import React from "react";
import { Check, Crosshair, MessageSquareWarning, X } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { BridgeEditItem, BridgeEdits, BridgeReport, JsonTranslationSheet } from "../../types";
import { formatError } from "../../utils/logging";
import { isPreviewRuntime } from "../../utils/runtime";

type Choice = "game" | "sheet";

/**
 * Surfaces what was fixed or reported in game (the bridge's F8 panel) so it
 * can be pulled into the sheet. Re-checks whenever the app regains focus,
 * which is usually right after switching back from the game.
 */
export function BridgeEditsButton({
  sheet,
  sheetPath,
  busy,
  onReplaceEntries,
  onApply,
  onRevealEntry,
}: {
  sheet: JsonTranslationSheet | null;
  sheetPath: string;
  busy: string | null;
  onReplaceEntries: (updates: Array<{ index: number; value: string }>, label?: string) => void;
  onApply: () => void;
  onRevealEntry: (key: string) => void;
}) {
  const [edits, setEdits] = React.useState<BridgeEdits | null>(null);
  const [open, setOpen] = React.useState(false);
  const [error, setError] = React.useState("");
  // Apply waits one render after the imported values land in the sheet, so
  // the save that apply starts with already contains them.
  const [pendingApply, setPendingApply] = React.useState(false);
  const sheetRef = React.useRef(sheet);
  sheetRef.current = sheet;
  const idle = !busy;

  const refresh = React.useCallback(async () => {
    const current = sheetRef.current;
    if (!current || isPreviewRuntime()) {
      return;
    }
    try {
      setEdits(await invokeCommand("bridge_edits_preview", { sheet: current }));
    } catch {
      // Sheets the bridge does not carry (DLL strings, no manifest id) have no in-game edits.
      setEdits(null);
    }
  }, []);

  React.useEffect(() => {
    if (sheetPath && idle) {
      void refresh();
    }
  }, [sheetPath, idle, refresh]);

  React.useEffect(() => {
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  React.useEffect(() => {
    if (pendingApply) {
      setPendingApply(false);
      onApply();
    }
  }, [pendingApply, sheet, onApply]);

  const editCount = edits?.items.length ?? 0;
  const reportCount = edits?.reports.length ?? 0;
  if (!edits || editCount + reportCount === 0) {
    return null;
  }
  const conflictCount = edits.items.filter((item) => item.kind === "conflict").length;

  async function resolveItems(gameItems: BridgeEditItem[]) {
    const current = sheetRef.current;
    if (!current || !edits?.ack_id) {
      return;
    }
    setError("");
    try {
      const indexByKey = new Map(current.entries.map((entry, index) => [entry.key, index]));
      const updates = gameItems.flatMap((item) => {
        const index = indexByKey.get(item.sheet_key);
        return index === undefined ? [] : [{ index, value: item.game_value }];
      });
      setEdits(await invokeCommand("acknowledge_bridge_edits", { sheet: current, ackId: edits.ack_id }));
      if (updates.length > 0) {
        onReplaceEntries(updates, `게임 안 수정 가져오기 ${updates.length}개`);
      }
      setOpen(false);
      // Choosing only sheet values changes nothing to apply; the ack alone drops the game edits.
      setPendingApply(updates.length > 0);
    } catch (caught) {
      setError(`가져오지 못했습니다: ${formatError(caught)}`);
    }
  }

  async function resolveReport(report: BridgeReport) {
    const current = sheetRef.current;
    if (!current) {
      return;
    }
    try {
      setEdits(await invokeCommand("resolve_bridge_report", { sheet: current, id: report.id }));
    } catch (caught) {
      setError(`처리하지 못했습니다: ${formatError(caught)}`);
    }
  }

  const label = [editCount > 0 ? `게임 안 수정 ${editCount}` : "", reportCount > 0 ? `오역 신고 ${reportCount}` : ""]
    .filter(Boolean)
    .join(" · ");
  return (
    <>
      <button
        type="button"
        className={`bridge-edits-badge${conflictCount > 0 ? " conflict" : ""}`}
        data-tooltip={
          conflictCount > 0
            ? `시트와 게임 양쪽에서 바뀐 항목 ${conflictCount}개는 골라야 합니다`
            : "게임 안(F8)에서 고친 값과 오역 신고를 확인합니다"
        }
        onClick={() => {
          setError("");
          setOpen(true);
          void refresh();
        }}
        disabled={Boolean(busy)}
      >
        {label}
      </button>
      {open && (
        <BridgeEditsModal
          edits={edits}
          error={error}
          onCancel={() => setOpen(false)}
          onConfirm={(gameItems) => void resolveItems(gameItems)}
          onResolveReport={(report) => void resolveReport(report)}
          onRevealEntry={(key) => {
            setOpen(false);
            onRevealEntry(key);
          }}
        />
      )}
    </>
  );
}

function BridgeEditsModal({
  edits,
  error,
  onCancel,
  onConfirm,
  onResolveReport,
  onRevealEntry,
}: {
  edits: BridgeEdits;
  error: string;
  onCancel: () => void;
  onConfirm: (gameItems: BridgeEditItem[]) => void;
  onResolveReport: (report: BridgeReport) => void;
  onRevealEntry: (key: string) => void;
}) {
  // Edits made only in game default to being imported; conflicts need a choice.
  const [choices, setChoices] = React.useState<Record<string, Choice | undefined>>(() =>
    Object.fromEntries(edits.items.filter((item) => item.kind === "game_changed").map((item) => [item.sheet_key, "game"])),
  );
  const unresolved = edits.items.filter((item) => !choices[item.sheet_key]).length;
  const setAll = (choice: Choice) => setChoices(Object.fromEntries(edits.items.map((item) => [item.sheet_key, choice])));
  const choose = (item: BridgeEditItem, choice: Choice) => setChoices((current) => ({ ...current, [item.sheet_key]: choice }));

  return (
    <div className="modal-backdrop">
      <section className="modal bridge-edits-modal" role="dialog" aria-modal="true" aria-label="게임 안 수정 확인">
        <header>
          <h2>게임 안 수정 확인</h2>
          <p>
            {edits.mod_id} / {edits.language}: 게임 안(F8)에서 고친 값과 오역 신고입니다. 게임 값을 고르면 시트에 들어가고(되돌리기 가능), 시트 값을 고르면 게임 안 수정은
            버려집니다.
          </p>
        </header>
        {edits.items.length > 0 && (
          <>
            <div className="bridge-edits-bulk">
              <strong>수정 {edits.items.length}개</strong>
              <button type="button" onClick={() => setAll("game")}>
                모두 게임 값
              </button>
              <button type="button" onClick={() => setAll("sheet")}>
                모두 시트 값
              </button>
              {unresolved > 0 && <span>고르지 않은 항목 {unresolved}개</span>}
            </div>
            <div className="bridge-edits-list">
              {edits.items.map((item) => (
                <div className={`bridge-edits-item ${item.kind}`} key={item.sheet_key}>
                  <div className="bridge-edits-key">
                    <span className="bridge-edits-kind">{item.kind === "conflict" ? "양쪽 변경" : "게임 안 수정"}</span>
                    <code>
                      {item.table} / {item.key}
                    </code>
                  </div>
                  {item.source && <p className="bridge-edits-source">{item.source}</p>}
                  <label className={choices[item.sheet_key] === "sheet" ? "selected" : undefined}>
                    <input type="radio" name={item.sheet_key} checked={choices[item.sheet_key] === "sheet"} onChange={() => choose(item, "sheet")} />
                    <span>시트</span>
                    <em>{item.sheet_value || "(비어 있음)"}</em>
                  </label>
                  <label className={choices[item.sheet_key] === "game" ? "selected" : undefined}>
                    <input type="radio" name={item.sheet_key} checked={choices[item.sheet_key] === "game"} onChange={() => choose(item, "game")} />
                    <span>게임</span>
                    <em>{item.game_value}</em>
                  </label>
                </div>
              ))}
            </div>
          </>
        )}
        {edits.reports.length > 0 && (
          <>
            <div className="bridge-edits-bulk">
              <strong>
                <MessageSquareWarning size={14} /> 오역 신고 {edits.reports.length}개
              </strong>
            </div>
            <div className="bridge-edits-list">
              {edits.reports.map((report) => (
                <div className="bridge-edits-item report" key={report.id}>
                  <div className="bridge-edits-key">
                    <code>
                      {report.table} / {report.key}
                    </code>
                  </div>
                  {report.source && <p className="bridge-edits-source">{report.source}</p>}
                  {report.shown && <p>게임 표시: {report.shown}</p>}
                  {report.note && <p className="bridge-edits-note">메모: {report.note}</p>}
                  <div className="bridge-edits-report-actions">
                    <button type="button" disabled={!report.sheet_key} onClick={() => report.sheet_key && onRevealEntry(report.sheet_key)}>
                      <Crosshair size={13} />
                      시트에서 보기
                    </button>
                    <button type="button" onClick={() => onResolveReport(report)}>
                      <Check size={13} />
                      처리 완료
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </>
        )}
        {error && <p className="inline-warning">{error}</p>}
        <footer>
          <button type="button" className="icon-button-text compact" onClick={onCancel}>
            <X size={15} />
            닫기
          </button>
          {edits.items.length > 0 && (
            <button
              type="button"
              className="primary icon-button-text compact"
              onClick={() => onConfirm(edits.items.filter((item) => choices[item.sheet_key] === "game"))}
              disabled={unresolved > 0}
            >
              <Check size={15} />
              선택대로 시트에 반영하고 적용
            </button>
          )}
        </footer>
      </section>
    </div>
  );
}
