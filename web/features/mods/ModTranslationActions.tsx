import React from "react";
import type { ExtractionTreeNode, ModRow } from "../../types";
import { hasLocalizationBranch, joinResourcePath } from "./modUtils";

function ModTranslationActions({
  mod,
  busy,
  onStartModTranslation,
}: {
  mod: ModRow;
  busy: string | null;
  onStartModTranslation: (mod: ModRow, resourcePath?: string) => void;
}) {
  const [manualPath, setManualPath] = React.useState("");
  const [showTree, setShowTree] = React.useState(false);

  if (mod.language_preview.length > 0) {
    return null;
  }

  return (
    <div className="mod-manual-translation">
      <div className="manual-picker-actions">
        <button type="button" onClick={() => setShowTree((current) => !current)} disabled={Boolean(busy)}>
          {showTree ? "트리 닫기" : "파일 트리 선택"}
        </button>
        <button type="button" onClick={() => onStartModTranslation(mod, manualPath)} disabled={Boolean(busy) || !manualPath.trim()}>
          선택 경로 번역
        </button>
      </div>
      {manualPath && <code className="selected-resource-path">{manualPath}</code>}
      {showTree && mod.extraction_tree.length > 0 && (
        <LanguageCandidateTree nodes={mod.extraction_tree} selectedPath={manualPath} onSelect={setManualPath} />
      )}
      {showTree && mod.extraction_tree.length === 0 && <p className="tree-empty">표시할 파일 트리가 없습니다. 먼저 추출을 눌러 파일 트리를 갱신하세요.</p>}
    </div>
  );
}

function LanguageCandidateTree({
  nodes,
  selectedPath,
  onSelect,
}: {
  nodes: ExtractionTreeNode[];
  selectedPath: string;
  onSelect: (path: string) => void;
}) {
  return (
    <div className="language-candidate-tree">
      <ul className="file-tree">
        {nodes.map((node) => (
          <LanguageCandidateNode node={node} depth={0} basePath="" selectedPath={selectedPath} onSelect={onSelect} key={`${node.kind}-${node.name}-${node.path}`} />
        ))}
      </ul>
    </div>
  );
}

function LanguageCandidateNode({
  node,
  depth,
  basePath,
  selectedPath,
  onSelect,
}: {
  node: ExtractionTreeNode;
  depth: number;
  basePath: string;
  selectedPath: string;
  onSelect: (path: string) => void;
}) {
  const isDirectory = node.children.length > 0 || node.kind === "dir";
  const nodePath = node.path || joinResourcePath(basePath, node.name);
  const selectable = isDirectory || node.kind === "language" || node.kind === "hardcoded";
  const [isOpen, setIsOpen] = React.useState(depth < 2 || hasLocalizationBranch(node));
  const isSelected = selectedPath === nodePath;

  return (
    <li>
      <button
        className={isSelected ? `tree-node tree-select ${node.kind} selected` : `tree-node tree-select ${node.kind}`}
        type="button"
        title={nodePath || node.name}
        onClick={() => {
          if (isDirectory) {
            setIsOpen((current) => !current);
          }
          if (selectable) {
            onSelect(nodePath);
          }
        }}
      >
        <span className="tree-icon">{isDirectory ? (isOpen ? "v" : ">") : node.kind === "language" ? "T" : node.kind === "hardcoded" ? "H" : "."}</span>
        <span>{node.name}</span>
        {selectable && (
          <small>
            {isSelected ? "선택됨" : "선택"}
          </small>
        )}
      </button>
      {isDirectory && isOpen && node.children.length > 0 && (
        <ul className="file-tree nested">
          {node.children.map((child) => (
            <LanguageCandidateNode
              node={child}
              depth={depth + 1}
              basePath={nodePath}
              selectedPath={selectedPath}
              onSelect={onSelect}
              key={`${child.kind}-${child.name}-${child.path}`}
            />
          ))}
        </ul>
      )}
    </li>
  );
}

export { ModTranslationActions };
