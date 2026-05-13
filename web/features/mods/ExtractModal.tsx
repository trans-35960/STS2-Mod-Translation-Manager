import React from "react";
import { labels } from "../../i18n";
import type { ExtractionTreeNode, ModRow } from "../../types";
import { hasLocalizationBranch, joinResourcePath } from "./modUtils";

function ExtractConfirmModal(props: {
  labels: typeof labels.ko;
  mod: ModRow;
  busy: string | null;
  outputDir: string;
  setOutputDir: (value: string) => void;
  onChooseOutputDir: () => void;
  onCancel: () => void;
  onConfirm: (force: boolean) => void;
  onClearCache: () => void;
  onExtractNode: (node: ExtractionTreeNode) => void;
  onOpenNodeTools: (node: ExtractionTreeNode) => void;
}) {
  const [isTreeOpen, setIsTreeOpen] = React.useState(true);
  const [contextMenu, setContextMenu] = React.useState<{
    x: number;
    y: number;
    node: ExtractionTreeNode;
  } | null>(null);
  const [selectedNode, setSelectedNode] = React.useState<ExtractionTreeNode | null>(null);
  const selectedPath = selectedNode?.path || selectedNode?.name || "";

  return (
    <div className="modal-backdrop" role="presentation" onClick={() => setContextMenu(null)}>
      <section className="modal extract-modal" role="dialog" aria-modal="true" aria-label="Extract language files">
        <header>
          <h2>파일 추출 확인</h2>
          <p>{props.mod.name}</p>
        </header>
        <div className="modal-detail">
          <span>추출 방식</span>
          <strong>{selectedNode ? `선택 항목: ${selectedPath}` : "전체 모드 파일"}</strong>
        </div>
        <div className="modal-detail">
          <span>원본 경로</span>
          <code>{props.mod.extraction_source_path}</code>
        </div>
        <div className="modal-detail">
          <span>현재 상태</span>
          <strong>{props.mod.translation_state} / {props.mod.extraction_hint}</strong>
        </div>
        <label className="modal-field">
          <span>추출 대상 폴더</span>
          <div>
            <input value={props.outputDir} onChange={(event) => props.setOutputDir(event.target.value)} />
            <button type="button" onClick={props.onChooseOutputDir} disabled={Boolean(props.busy)}>
              선택
            </button>
          </div>
        </label>
        {props.mod.language_preview.length > 0 && (
          <div className="language-preview modal-languages">
            {props.mod.language_preview.map((language) => (
              <span title={`${language.files} files: ${language.sample_path}`} key={language.code}>
                {language.label} {language.files}
              </span>
            ))}
          </div>
        )}
        <div className="modal-cache-controls">
          <span>파일 트리가 예전 추출 결과처럼 보이면 분석 캐시를 지우고 다시 읽으세요.</span>
          <button type="button" onClick={props.onClearCache} disabled={Boolean(props.busy)}>
            캐시 삭제 후 다시 읽기
          </button>
        </div>
        <div className="modal-tree-section">
          <button
            className="modal-tree-title"
            type="button"
            aria-expanded={isTreeOpen}
            onClick={() => setIsTreeOpen((current) => !current)}
          >
            <span>{props.labels.fileTree}</span>
            <span>{isTreeOpen ? props.labels.collapse : props.labels.expand}</span>
          </button>
          {isTreeOpen && (
            <div className="modal-tree-body">
              {props.mod.extraction_tree.length > 0 ? (
                <FileTree
                  nodes={props.mod.extraction_tree}
                  selectedPath={selectedPath}
                  onSelect={setSelectedNode}
                  onContextMenu={(node, event) => {
                    event.preventDefault();
                    event.stopPropagation();
                    setContextMenu({ x: event.clientX, y: event.clientY, node });
                  }}
                />
              ) : (
                <p className="tree-empty">{props.labels.noFileTree}</p>
              )}
            </div>
          )}
        </div>
        <footer>
          <button onClick={props.onCancel}>취소</button>
          <button type="button" onClick={() => selectedNode && props.onExtractNode(selectedNode)} disabled={Boolean(props.busy) || !props.outputDir.trim() || !selectedNode}>
            선택 추출
          </button>
          <button type="button" onClick={() => props.onConfirm(true)} disabled={Boolean(props.busy) || !props.outputDir.trim()}>
            강제 전체 추출
          </button>
          <button className="primary" onClick={() => props.onConfirm(false)} disabled={Boolean(props.busy) || !props.outputDir.trim()}>
            전체 추출
          </button>
        </footer>
      </section>
      {contextMenu && (
        <div
          className="tree-context-menu"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={(event) => event.stopPropagation()}
        >
          <strong>{contextMenu.node.path || contextMenu.node.name}</strong>
          <button
            type="button"
            onClick={() => {
              setSelectedNode(contextMenu.node);
              props.onExtractNode(contextMenu.node);
              setContextMenu(null);
            }}
          >
            선택 항목만 추출
          </button>
          <button
            type="button"
            onClick={() => {
              props.onOpenNodeTools(contextMenu.node);
              setContextMenu(null);
            }}
          >
            번역 도구에서 작업
          </button>
        </div>
      )}
    </div>
  );
}

function FileTree({
  nodes,
  depth = 0,
  basePath = "",
  selectedPath,
  onSelect,
  onContextMenu,
}: {
  nodes: ExtractionTreeNode[];
  depth?: number;
  basePath?: string;
  selectedPath: string;
  onSelect: (node: ExtractionTreeNode) => void;
  onContextMenu: (node: ExtractionTreeNode, event: React.MouseEvent) => void;
}) {
  return (
    <ul className="file-tree">
      {nodes.map((node) => (
        <FileTreeNode
          node={node}
          depth={depth}
          basePath={basePath}
          selectedPath={selectedPath}
          onSelect={onSelect}
          onContextMenu={onContextMenu}
          key={`${node.kind}-${node.name}-${node.path}`}
        />
      ))}
    </ul>
  );
}

function FileTreeNode({
  node,
  depth,
  basePath,
  selectedPath,
  onSelect,
  onContextMenu,
}: {
  node: ExtractionTreeNode;
  depth: number;
  basePath: string;
  selectedPath: string;
  onSelect: (node: ExtractionTreeNode) => void;
  onContextMenu: (node: ExtractionTreeNode, event: React.MouseEvent) => void;
}) {
  const isDirectory = node.children.length > 0 || node.kind === "dir";
  const [isOpen, setIsOpen] = React.useState(depth === 0 || hasLocalizationBranch(node));
  const nodePath = node.path || joinResourcePath(basePath, node.name);
  const actionNode = { ...node, path: nodePath };
  const isSelected = selectedPath === nodePath;

  return (
    <li>
      {isDirectory ? (
        <button
          className={isSelected ? `tree-node tree-toggle ${node.kind} selected` : `tree-node tree-toggle ${node.kind}`}
          type="button"
          title={nodePath || node.name}
          aria-expanded={isOpen}
          onContextMenu={(event) => onContextMenu(actionNode, event)}
          onClick={() => {
            onSelect(actionNode);
            setIsOpen((current) => !current);
          }}
        >
          <span className="tree-icon">{isOpen ? "▾" : "▸"}</span>
          <span>{node.name}</span>
        </button>
      ) : (
        <div
          className={isSelected ? `tree-node ${node.kind} selected` : `tree-node ${node.kind}`}
          title={nodePath || node.name}
          onContextMenu={(event) => onContextMenu(actionNode, event)}
          onClick={() => onSelect(actionNode)}
        >
          <span className="tree-icon">{node.kind === "language" ? "T" : node.kind === "hardcoded" ? "H" : "·"}</span>
          <span>{node.name}</span>
        </div>
      )}
      {isDirectory && isOpen && node.children.length > 0 && (
        <FileTree nodes={node.children} depth={depth + 1} basePath={nodePath} selectedPath={selectedPath} onSelect={onSelect} onContextMenu={onContextMenu} />
      )}
    </li>
  );
}

export { ExtractConfirmModal };
