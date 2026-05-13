import React from "react";
import { Archive, DownloadCloud, FolderPlus, PackagePlus } from "lucide-react";

function ImportMenu(props: {
  busy: string | null;
  vortexDownloadCount: number;
  onImportFolder: () => void;
  onImportArchive: () => void;
  onImportVortexDownloads: () => void;
}) {
  const [open, setOpen] = React.useState(false);
  const busy = Boolean(props.busy);
  const hasVortexDownloads = props.vortexDownloadCount > 0;

  function run(action: () => void) {
    setOpen(false);
    action();
  }

  return (
    <div className="import-menu">
      <button
        className="toolbar-icon-button"
        type="button"
        onClick={() => setOpen((value) => !value)}
        aria-expanded={open}
        aria-label="모드 불러오기"
        data-tooltip="모드 불러오기"
        disabled={busy}
      >
        <PackagePlus size={16} />
      </button>
      {open && (
        <div className="import-popover">
          <button type="button" onClick={() => run(props.onImportFolder)} disabled={busy}>
            <FolderPlus size={15} />
            <span>폴더 불러오기</span>
          </button>
          <button type="button" onClick={() => run(props.onImportArchive)} disabled={busy}>
            <Archive size={15} />
            <span>압축파일 불러오기</span>
          </button>
          <button
            type="button"
            onClick={() => run(props.onImportVortexDownloads)}
            disabled={busy || !hasVortexDownloads}
            title={hasVortexDownloads ? "Nexus/Vortex 다운로드 폴더에서 감지된 항목을 불러옵니다." : "감지된 Vortex 다운로드 항목이 없습니다."}
          >
            <DownloadCloud size={15} />
            <span>Vortex 다운로드</span>
            {hasVortexDownloads && <small>{props.vortexDownloadCount}</small>}
          </button>
        </div>
      )}
    </div>
  );
}

export { ImportMenu };
