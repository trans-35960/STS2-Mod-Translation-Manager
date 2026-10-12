import React from "react";
import { Check, X } from "lucide-react";
import type { TranslatorSyncItem, TranslatorSyncPreview } from "../../types";

type Choice = "game" | "sheet";

/**
 * Lists override values the player changed in game since the last save and
 * keys changed on both sides, and lets the user pick which side wins.
 */
export function TranslatorSyncModal({
  preview,
  onCancel,
  onConfirm,
}: {
  preview: TranslatorSyncPreview;
  onCancel: () => void;
  onConfirm: (gameItems: TranslatorSyncItem[], sheetItems: TranslatorSyncItem[]) => void;
}) {
  // Edits made only in game default to being imported; conflicts need a choice.
  const [choices, setChoices] = React.useState<Record<string, Choice | undefined>>(() =>
    Object.fromEntries(preview.items.filter((item) => item.kind === "game_changed").map((item) => [itemId(item), "game"])),
  );
  const unresolved = preview.items.filter((item) => !choices[itemId(item)]).length;
  const setAll = (choice: Choice) => setChoices(Object.fromEntries(preview.items.map((item) => [itemId(item), choice])));

  function confirm() {
    const gameItems = preview.items.filter((item) => choices[itemId(item)] === "game");
    const sheetItems = preview.items.filter((item) => choices[itemId(item)] === "sheet");
    onConfirm(gameItems, sheetItems);
  }

  return (
    <div className="modal-backdrop">
      <section className="modal translator-sync-modal" role="dialog" aria-modal="true" aria-label="게임 안 변경 확인">
        <header>
          <h2>게임 안 변경 확인</h2>
          <p>
            {preview.mod_id} / {preview.language}: Sts2ModTranslator에서 마지막 저장 이후 바뀐 항목입니다. 게임 값을 고르면 시트에 반영되고(되돌리기 가능),
            시트 값을 고르면 게임 값을 덮어씁니다.
          </p>
        </header>
        <div className="translator-sync-bulk">
          <button type="button" onClick={() => setAll("game")}>모두 게임 값</button>
          <button type="button" onClick={() => setAll("sheet")}>모두 시트 값</button>
          {unresolved > 0 && <span>선택 안 한 충돌 {unresolved}개</span>}
        </div>
        <div className="translator-sync-list">
          {preview.items.map((item) => {
            const id = itemId(item);
            return (
              <div className={`translator-sync-item ${item.kind}`} key={id}>
                <div className="translator-sync-key">
                  <span className="translator-sync-kind">{item.kind === "conflict" ? "양쪽 변경" : "게임 안 변경"}</span>
                  <code>
                    {item.table} / {item.key}
                  </code>
                </div>
                <label className={choices[id] === "sheet" ? "selected" : undefined}>
                  <input type="radio" name={id} checked={choices[id] === "sheet"} onChange={() => setChoices((current) => ({ ...current, [id]: "sheet" }))} />
                  <span>시트</span>
                  <em>{item.ours || "(비어 있음)"}</em>
                </label>
                <label className={choices[id] === "game" ? "selected" : undefined}>
                  <input type="radio" name={id} checked={choices[id] === "game"} onChange={() => setChoices((current) => ({ ...current, [id]: "game" }))} />
                  <span>게임</span>
                  <em>{item.theirs || "(비어 있음)"}</em>
                </label>
              </div>
            );
          })}
        </div>
        <footer>
          <button type="button" className="icon-button-text compact" onClick={onCancel}>
            <X size={15} />
            취소
          </button>
          <button type="button" className="primary icon-button-text compact" onClick={confirm} disabled={unresolved > 0}>
            <Check size={15} />
            선택대로 반영하고 적용
          </button>
        </footer>
      </section>
    </div>
  );
}

function itemId(item: TranslatorSyncItem) {
  return `${item.table}\u0000${item.key}`;
}
