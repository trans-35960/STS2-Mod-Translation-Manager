import React from "react";
import { TriangleAlert } from "lucide-react";
import type { ModRow } from "../../types";
import { modRelativePath } from "./modUtils";

/**
 * Asks before opening a mod DLL's strings in the translation tool. DLL
 * strings are the mod's code: some are names, keys or text the code compares,
 * and translating those breaks the mod, so the risk is spelled out every time.
 */
export function DllTranslateModal({
  mod,
  onCancel,
  onOpen,
}: {
  mod: ModRow;
  onCancel: () => void;
  /** Path of the chosen DLL relative to the mod folder. */
  onOpen: (dllPath: string) => void;
}) {
  const dlls = React.useMemo(() => (mod.dll_files ?? []).map((path) => modRelativePath(mod.path, path)), [mod]);
  const [selected, setSelected] = React.useState(dlls[0] ?? "");
  const [understood, setUnderstood] = React.useState(false);

  React.useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onCancel();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="modal-backdrop" role="presentation" onClick={onCancel}>
      <section className="modal dll-translate-modal" role="dialog" aria-modal="true" aria-label="DLL 문자열 번역" onClick={(event) => event.stopPropagation()}>
        <header>
          <h2>
            <TriangleAlert size={18} />
            DLL 문자열 번역
          </h2>
          <p>{mod.workshop_title?.trim() || mod.name}</p>
        </header>
        <ul className="dll-translate-risks">
          <li>
            DLL 문자열은 모드의 <strong>코드 안에 들어 있는 글자</strong>입니다. 화면에 보이는 문장 말고도 이름, 키, 비교용 값이 섞여 있어서
            잘못 바꾸면 <strong>모드가 오작동하거나 게임이 튕길 수 있습니다.</strong>
          </li>
          <li>번역 도구는 번역할 문장만 먼저 보여 주고, 키·로그·경로·정규식처럼 바꾸면 안 되는 항목은 적용할 때 자동으로 뺍니다. 그래도 완벽하지 않습니다.</li>
          <li>모드 파일은 그대로 두고 번역 브리지가 게임 안에서 바꿉니다. 적용 후 게임을 다시 시작해야 반영됩니다.</li>
          <li>번역 후 게임이 이상하면 이 모드의 DLL 번역부터 의심하세요.</li>
        </ul>
        {dlls.length > 1 && (
          <label className="dll-translate-pick">
            <span>DLL 선택</span>
            <select value={selected} onChange={(event) => setSelected(event.target.value)}>
              {dlls.map((path) => (
                <option value={path} key={path}>
                  {path}
                </option>
              ))}
            </select>
          </label>
        )}
        {dlls.length === 1 && <code className="dll-translate-path">{selected}</code>}
        <label className="dll-translate-confirm">
          <input type="checkbox" checked={understood} onChange={(event) => setUnderstood(event.target.checked)} />
          위험을 이해했고 번역할 문장만 신중하게 바꾸겠습니다
        </label>
        <footer>
          <button type="button" className="icon-button-text compact" onClick={onCancel}>
            취소
          </button>
          <button type="button" className="primary icon-button-text compact" onClick={() => onOpen(selected)} disabled={!understood || !selected}>
            번역 도구에서 열기
          </button>
        </footer>
      </section>
    </div>
  );
}
