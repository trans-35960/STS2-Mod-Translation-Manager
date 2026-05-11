import React from "react";
import { Pill } from "../../components/Common";
import type { TranslationProjectNode } from "./translationUtils";

export function TranslationProjectTree({
  tree,
  selectedPath,
  onSelect,
  onCopyJson,
}: {
  tree: TranslationProjectNode | null;
  selectedPath: string | null;
  onSelect: (path: string | null) => void;
  onCopyJson: (node: TranslationProjectNode) => void;
}) {
  const [contextMenu, setContextMenu] = React.useState<{
    x: number;
    y: number;
    node: TranslationProjectNode;
  } | null>(null);

  React.useEffect(() => {
    if (!contextMenu) {
      return;
    }
    const close = () => setContextMenu(null);
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setContextMenu(null);
      }
    };
    window.addEventListener("click", close);
    window.addEventListener("keydown", closeOnEscape);
    return () => {
      window.removeEventListener("click", close);
      window.removeEventListener("keydown", closeOnEscape);
    };
  }, [contextMenu]);

  if (!tree) {
    return (
      <section className="project-tree-card">
        <h3>프로젝트</h3>
        <p>번역 시트를 생성하거나 불러오면 파일별 진행률이 표시됩니다.</p>
      </section>
    );
  }
  return (
    <section className="project-tree-card">
      <div className="project-tree-header">
        <h3>프로젝트 파일</h3>
        <span>{tree.ready}/{tree.total}</span>
      </div>
      <div className="progress-track" aria-label="translation progress">
        <span style={{ width: `${tree.total ? Math.round((tree.ready / tree.total) * 100) : 0}%` }} />
      </div>
      <ul className="project-tree">
        <TranslationProjectTreeNode
          node={tree}
          depth={0}
          selectedPath={selectedPath}
          onSelect={onSelect}
          onCopyJson={onCopyJson}
          onOpenContextMenu={(node, event) => {
            event.preventDefault();
            event.stopPropagation();
            setContextMenu({ x: event.clientX, y: event.clientY, node });
          }}
        />
      </ul>
      {contextMenu && (
        <div
          className="tree-context-menu project-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <strong>{contextMenu.node.filterPath || contextMenu.node.name}</strong>
          <button
            type="button"
            onClick={() => {
              onSelect(contextMenu.node.filterPath || null);
              setContextMenu(null);
            }}
          >
            이 범위 보기
          </button>
          <button
            type="button"
            onClick={() => {
              onCopyJson(contextMenu.node);
              setContextMenu(null);
            }}
          >
            JSON 구조로 복사
          </button>
        </div>
      )}
    </section>
  );
}

function TranslationProjectTreeNode({
  node,
  depth,
  selectedPath,
  onSelect,
  onCopyJson,
  onOpenContextMenu,
}: {
  node: TranslationProjectNode;
  depth: number;
  selectedPath: string | null;
  onSelect: (path: string | null) => void;
  onCopyJson: (node: TranslationProjectNode) => void;
  onOpenContextMenu: (node: TranslationProjectNode, event: React.MouseEvent) => void;
}) {
  const [open, setOpen] = React.useState(depth < 2);
  const isDirectory = node.children.length > 0;
  const complete = node.total > 0 && node.ready === node.total;
  const selected = selectedPath === node.filterPath || (!selectedPath && depth === 0);
  return (
    <li>
      <button
        className={`project-tree-node${selected ? " selected" : ""}`}
        type="button"
        title="우클릭: 복사 메뉴 열기"
        onContextMenu={(event) => onOpenContextMenu(node, event)}
        onClick={() => {
          onSelect(depth === 0 ? null : node.filterPath);
          if (isDirectory) {
            setOpen((value) => !value);
          }
        }}
      >
        <span className="tree-icon">{isDirectory ? (open ? "▾" : "▸") : "·"}</span>
        <span title={node.path}>{node.name}</span>
        <Pill tone={complete ? "good" : node.attention ? "warn" : undefined}>{node.ready}/{node.total}</Pill>
      </button>
      {isDirectory && open && (
        <ul className="project-tree nested">
          {node.children.map((child) => (
            <TranslationProjectTreeNode
              node={child}
              depth={depth + 1}
              selectedPath={selectedPath}
              onSelect={onSelect}
              onCopyJson={onCopyJson}
              onOpenContextMenu={onOpenContextMenu}
              key={child.path}
            />
          ))}
        </ul>
      )}
    </li>
  );
}
