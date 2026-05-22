import React from "react";

export function useAppLogs() {
  const [logs, setLogs] = React.useState<string[]>([]);

  const appendLog = React.useCallback((message: string) => {
    const time = new Date().toLocaleTimeString();
    setLogs((items) => [`[${time}] ${message}`, ...items].slice(0, 30));
  }, []);

  return { logs, setLogs, appendLog };
}
