import React from "react";
import { isAlertLog } from "../features/translation/LogToasts";

export function useAppLogs() {
  const [logs, setLogs] = React.useState<string[]>([]);

  const appendLog = React.useCallback((message: string) => {
    const time = new Date().toLocaleTimeString();
    setLogs((items) => [`[${time}] ${message}`, ...items].slice(0, 30));
  }, []);

  React.useEffect(() => {
    if (!logs.some((log) => !isAlertLog(log))) {
      return;
    }
    const timeoutId = window.setTimeout(() => {
      setLogs((items) => items.filter((log) => isAlertLog(log)));
    }, 4500);
    return () => window.clearTimeout(timeoutId);
  }, [logs]);

  return { logs, setLogs, appendLog };
}
