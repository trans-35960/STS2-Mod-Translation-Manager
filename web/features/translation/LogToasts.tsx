import type { ApplyResultState } from "../../types";
import { logTone, shouldToastLog } from "../../utils/logging";

function ApplyStatusToast({ result, onDismiss }: { result: ApplyResultState; onDismiss: () => void }) {
  const packed = Boolean(result.packed_pck_path || result.installed_mod_path);
  const failed = Boolean(result.error) || result.applied_entries === 0;
  return (
    <article className={`log-toast apply-toast ${packed && !failed ? "good" : "warn"}`}>
      <div>
        <strong>{result.message}</strong>
        {result.language_output_path && <p>언어 폴더: {result.language_output_path}</p>}
        {result.packed_pck_path && <p>PCK: {result.packed_pck_path}</p>}
        {result.installed_mod_path && <p>활성 모드: {result.installed_mod_path}</p>}
        {failed && <small>번역값이 없으면 적용된 PCK를 만들지 않습니다. translated_value를 채운 뒤 다시 실행해 주세요.</small>}
        {!failed && !packed && <small>PCK 원본을 찾지 못해 JSON만 저장했습니다. 경로/고급 설정의 출력 경로와 원본 PCK를 확인해 주세요.</small>}
        {!failed && packed && result.installed_mod_path && <small>패치된 PCK가 활성 모드 위치에 반영되었습니다. 새로고침하면 실제 파일트리에 대상 언어가 보여야 합니다.</small>}
        {!failed && packed && !result.installed_mod_path && <small>생성된 PCK는 출력 경로에 있습니다. 모드 폴더에 등록되지 않은 형식이라 직접 배치가 필요합니다.</small>}
      </div>
      <button type="button" onClick={onDismiss} aria-label="적용 결과 닫기">
        닫기
      </button>
    </article>
  );
}

function LogToasts({
  logs,
  onDismiss,
  applyResult,
  onDismissApply,
}: {
  logs: string[];
  onDismiss: (index: number) => void;
  applyResult: ApplyResultState | null;
  onDismissApply: () => void;
}) {
  const toastLogs = logs
    .map((log, index) => ({ log, index }))
    .filter(({ log }) => shouldToastLog(log));
  if (toastLogs.length === 0 && !applyResult) {
    return null;
  }
  return (
    <aside className="log-toasts" aria-live="polite">
      {applyResult && <ApplyStatusToast result={applyResult} onDismiss={onDismissApply} />}
      {toastLogs.map(({ log, index }) => (
        <article className={`log-toast ${logTone(log)}`} key={`${index}-${log}`}>
          <p>{log}</p>
          <button type="button" onClick={() => onDismiss(index)} aria-label="로그 닫기">
            닫기
          </button>
        </article>
      ))}
    </aside>
  );
}

function isAlertApplyResult(result: ApplyResultState) {
  return Boolean(result.error) || result.applied_entries === 0 || !(result.packed_pck_path || result.installed_mod_path);
}

function isAlertLog(log: string) {
  return shouldToastLog(log);
}

export { LogToasts, isAlertApplyResult, isAlertLog };
