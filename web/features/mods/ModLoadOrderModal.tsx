import React from "react";
import { ArrowDown, ArrowUp, GripVertical, Lock, RotateCcw, Save, X } from "lucide-react";
import { invokeCommand } from "../../api/tauri";
import type { ModLoadOrder, ModLoadOrderEntry, ModOrderKey, ModRow } from "../../types";
import { isPreviewRuntime } from "../../utils/runtime";

/**
 * Edits the order the game loads mods in (`settings.save` mod_list). The game
 * still loads a mod's dependencies before it, so every change is previewed
 * against the game's own sorting before it is saved.
 */
export function ModLoadOrderModal({ mods, onClose }: { mods: ModRow[]; onClose: () => void }) {
  const modsByKey = React.useMemo(() => new Map(mods.map((mod) => [mod.key, mod])), [mods]);
  const [order, setOrder] = React.useState<ModLoadOrder | null>(null);
  const [dirty, setDirty] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [dragIndex, setDragIndex] = React.useState<number | null>(null);
  const [dropIndex, setDropIndex] = React.useState<number | null>(null);
  const requestRef = React.useRef(0);

  const load = React.useCallback(async (draft: ModOrderKey[] | null) => {
    const request = ++requestRef.current;
    try {
      const next = isPreviewRuntime()
        ? previewLoadOrder(mods, draft)
        : await invokeCommand("load_mod_load_order", { draft });
      if (request !== requestRef.current) return;
      setOrder(next);
      setError(null);
    } catch (reason) {
      if (request === requestRef.current) setError(String(reason));
    }
  }, [mods]);

  React.useEffect(() => {
    void load(null);
  }, [load]);

  React.useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const entries = order?.entries ?? [];
  const firstMovable = entries.findIndex((entry) => !entry.pinned);

  function move(from: number, to: number) {
    if (!order || from === to || to < firstMovable || entries[from]?.pinned) return;
    const next = [...entries];
    const [moved] = next.splice(from, 1);
    next.splice(to, 0, moved);
    setOrder({ ...order, entries: next });
    setDirty(true);
    void load(next.map(orderKey));
  }

  async function save() {
    if (!order) return;
    setBusy(true);
    try {
      const keys = entries.map(orderKey);
      const saved = isPreviewRuntime()
        ? previewLoadOrder(mods, keys)
        : await invokeCommand("save_mod_load_order", { order: keys });
      requestRef.current++;
      setOrder(saved);
      setDirty(false);
      setError(null);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setBusy(false);
    }
  }

  function reset() {
    setDirty(false);
    void load(null);
  }

  const reordered = entries.some((entry, index) => entry.load_index !== index);
  const canSave = Boolean(order?.settings_found) && !order?.game_running && dirty && !busy;

  return (
    <div className="modal-backdrop" role="presentation" onClick={onClose}>
      <section
        className="modal load-order-modal"
        role="dialog"
        aria-modal="true"
        aria-label="모드 로드 순서"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="load-order-header">
          <div>
            <h2>모드 로드 순서</h2>
            <p>
              게임은 위에서부터 차례로 모드를 불러옵니다. 같은 번역 키나 리소스를 가진 모드끼리는 아래쪽(나중) 모드가 이깁니다.
              필요한 모드(의존성)는 순서와 상관없이 먼저 불러옵니다.
            </p>
          </div>
          <button type="button" className="icon-only-button" aria-label="닫기" data-tooltip="닫기" onClick={onClose}>
            <X size={16} />
          </button>
        </header>
        {order?.game_running && <p className="load-order-notice warn">게임이 실행 중입니다. 게임을 종료한 뒤 저장할 수 있습니다.</p>}
        {order && !order.settings_found && (
          <p className="load-order-notice warn">게임 설정 파일이 아직 없습니다. 게임을 한 번 실행한 뒤 순서를 저장할 수 있습니다.</p>
        )}
        {reordered && (
          <p className="load-order-notice">의존성 때문에 실제 로드 위치가 다른 모드가 있습니다. 오른쪽 "실제" 번호를 확인하세요.</p>
        )}
        {error && <p className="load-order-notice danger">{error}</p>}
        <ol className="load-order-list">
          {!order && !error && <li className="load-order-empty">불러오는 중…</li>}
          {order && entries.length === 0 && <li className="load-order-empty">활성화된 모드가 없습니다.</li>}
          {entries.map((entry, index) => (
            <LoadOrderRow
              key={`${entry.source}:${entry.id}`}
              entry={entry}
              index={index}
              mod={modsByKey.get(entry.key)}
              canMoveUp={!entry.pinned && index > firstMovable}
              canMoveDown={!entry.pinned && index < entries.length - 1}
              dragging={dragIndex === index}
              dropTarget={dropIndex === index && dragIndex !== index}
              onMove={move}
              onDragStart={() => setDragIndex(index)}
              onDragOver={() => {
                if (dragIndex !== null && !entry.pinned) setDropIndex(index);
              }}
              onDrop={() => {
                if (dragIndex !== null) move(dragIndex, index);
                setDragIndex(null);
                setDropIndex(null);
              }}
              onDragEnd={() => {
                setDragIndex(null);
                setDropIndex(null);
              }}
            />
          ))}
        </ol>
        <footer className="load-order-footer">
          <span className="load-order-muted">
            {dirty ? "저장하지 않은 변경이 있습니다." : "저장하면 게임 설정(settings.save)에 바로 반영됩니다."}
          </span>
          <button type="button" className="icon-button-text compact" onClick={reset} disabled={!dirty || busy}>
            <RotateCcw size={15} />
            되돌리기
          </button>
          <button type="button" className="primary icon-button-text compact" onClick={() => void save()} disabled={!canSave}>
            <Save size={15} />
            저장
          </button>
        </footer>
      </section>
    </div>
  );
}

function LoadOrderRow({
  entry,
  index,
  mod,
  canMoveUp,
  canMoveDown,
  dragging,
  dropTarget,
  onMove,
  onDragStart,
  onDragOver,
  onDrop,
  onDragEnd,
}: {
  entry: ModLoadOrderEntry;
  index: number;
  mod: ModRow | undefined;
  canMoveUp: boolean;
  canMoveDown: boolean;
  dragging: boolean;
  dropTarget: boolean;
  onMove: (from: number, to: number) => void;
  onDragStart: () => void;
  onDragOver: () => void;
  onDrop: () => void;
  onDragEnd: () => void;
}) {
  const title = mod?.workshop_title?.trim() || mod?.name || entry.id;
  const moved = entry.load_index !== index;
  const className = [
    "load-order-row",
    entry.pinned ? "pinned" : "",
    dragging ? "dragging" : "",
    dropTarget ? "drop-target" : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <li
      className={className}
      draggable={!entry.pinned}
      onDragStart={(event) => {
        event.dataTransfer.effectAllowed = "move";
        onDragStart();
      }}
      onDragOver={(event) => {
        if (entry.pinned) return;
        event.preventDefault();
        onDragOver();
      }}
      onDrop={(event) => {
        event.preventDefault();
        onDrop();
      }}
      onDragEnd={onDragEnd}
    >
      <span className="load-order-handle" aria-hidden="true">
        {entry.pinned ? <Lock size={14} /> : <GripVertical size={14} />}
      </span>
      <span className="load-order-index">{index + 1}</span>
      <span className="load-order-name">
        <strong title={title}>{title}</strong>
        <span className="load-order-muted">
          {entry.id} · {entry.source === "steam_workshop" ? "창작마당" : "로컬"}
          {entry.pinned && " · 번역 브리지는 항상 먼저 불러옵니다"}
        </span>
        <span className="load-order-tags">
          {entry.waits_for.length > 0 && (
            <span className="load-order-tag">{entry.waits_for.join(", ")} 다음에 로드</span>
          )}
          {entry.missing_dependencies.length > 0 && (
            <span className="load-order-tag danger">필요 모드 없음: {entry.missing_dependencies.join(", ")}</span>
          )}
          {entry.circular && <span className="load-order-tag danger">순환 의존성 — 로드 실패</span>}
          {!entry.listed && !entry.pinned && <span className="load-order-tag">순서 미지정 (저장하면 고정)</span>}
        </span>
      </span>
      <span className={moved ? "load-order-actual moved" : "load-order-actual"} title="게임이 실제로 불러오는 순서">
        실제 {entry.load_index + 1}
      </span>
      <span className="load-order-buttons">
        <button
          type="button"
          className="icon-only-button"
          aria-label="위로"
          data-tooltip="위로"
          disabled={!canMoveUp}
          onClick={() => onMove(index, index - 1)}
        >
          <ArrowUp size={14} />
        </button>
        <button
          type="button"
          className="icon-only-button"
          aria-label="아래로"
          data-tooltip="아래로"
          disabled={!canMoveDown}
          onClick={() => onMove(index, index + 1)}
        >
          <ArrowDown size={14} />
        </button>
      </span>
    </li>
  );
}

function orderKey(entry: ModLoadOrderEntry): ModOrderKey {
  return { id: entry.id, source: entry.source };
}

/** Browser preview: active mods in the given order, dependencies ignored. */
function previewLoadOrder(mods: ModRow[], draft: ModOrderKey[] | null): ModLoadOrder {
  const entries: ModLoadOrderEntry[] = mods
    .filter((mod) => mod.active && mod.manifest_id)
    .map((mod) => ({
      id: mod.manifest_id ?? mod.name,
      source: mod.workshop ? "steam_workshop" : "mods_directory",
      key: mod.key,
      load_index: 0,
      listed: true,
      pinned: Boolean(mod.system_mod),
      waits_for: [],
      missing_dependencies: [],
      circular: false,
    }));
  const position = (entry: ModLoadOrderEntry) => {
    if (entry.pinned) return -1;
    const index = draft?.findIndex((key) => key.id === entry.id && key.source === entry.source) ?? -1;
    return index < 0 ? Number.MAX_SAFE_INTEGER : index;
  };
  entries.sort((left, right) => position(left) - position(right));
  entries.forEach((entry, index) => {
    entry.load_index = index;
  });
  return { settings_found: true, game_running: false, entries };
}
