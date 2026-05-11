const fs = require('fs');
let c = fs.readFileSync('z:/game/sts2/modmanager/web/features/settings/SettingsPage.tsx', 'utf8');

c = c.replace(
  /{pathRows\.map\(\(\[label, value\]\) => \([\s\S]*?\)\)}/m,
  `<details className="settings-details" style={{ marginTop: "16px", padding: "8px 0" }}>
          <summary style={{ cursor: "pointer", fontWeight: "bold" }}>고급 경로 정보 표시</summary>
          <div className="details-content" style={{ marginTop: "8px" }}>
            {pathRows.map(([label, value]) => (
              <div className="path-row" key={label}>
                <span>{label}</span>
                <code>{value}</code>
              </div>
            ))}
          </div>
        </details>`
);

// Tools section
c = c.replace(
  /<section className="settings-section">\s*<h2>필수 내장 도구<\/h2>[\s\S]*?<\/section>/m,
  `<details className="settings-section settings-details">
          <summary style={{ cursor: "pointer", outline: "none" }}><h2 style={{ display: "inline-block", margin: 0 }}>필수 내장 도구</h2></summary>
          <div style={{ marginTop: 12 }}>
            <p>{t.optional7z}</p>
            {dashboard.tools.map((tool) => (
              <div className="tool-row" key={tool.name}>
                <Pill tone={tool.available ? "good" : "warn"}>{tool.available ? "OK" : "Missing"}</Pill>
                <div>
                  <strong>{tool.name}</strong>
                  <small>{tool.expected_path}</small>
                </div>
              </div>
            ))}
          </div>
        </details>`
);

// Logs
c = c.replace(
  /<section className="settings-section settings-log-section">\s*<h2>\{t\.logs\}<\/h2>\s*<LogsPanel labels=\{t\} logs=\{logs\} \/>\s*<\/section>/m,
  `<details className="settings-section settings-log-section settings-details">
          <summary style={{ cursor: "pointer", outline: "none" }}><h2 style={{ display: "inline-block", margin: 0 }}>{t.logs}</h2></summary>
          <div style={{ marginTop: 12 }}>
            <LogsPanel labels={t} logs={logs} />
          </div>
        </details>`
);

// Game Logs
c = c.replace(
  /<section className="settings-section settings-log-section">\s*<div className="settings-title-row">\s*<h2>게임 로그<\/h2>\s*<button className="icon-button-text compact" type="button" onClick=\{onRefreshGameLogs\} disabled=\{gameLogsLoading\}>\s*<RefreshCw size=\{14\} \/>\s*\{gameLogsLoading \? "확인 중" : "새로고침"\}\s*<\/button>\s*<\/div>\s*<GameLogsPanel logs=\{gameLogs\} onOpenPath=\{onOpenPath\} \/>\s*<\/section>/m,
  `<details className="settings-section settings-log-section settings-details">
          <summary style={{ cursor: "pointer", outline: "none" }}>
            <div className="settings-title-row" style={{ display: "inline-flex", width: "calc(100% - 24px)", alignItems: "center" }}>
              <h2 style={{ margin: 0 }}>게임 로그</h2>
              <button className="icon-button-text compact" type="button" onClick={onRefreshGameLogs} disabled={gameLogsLoading} style={{ marginLeft: "auto" }}>
                <RefreshCw size={14} />
                {gameLogsLoading ? "확인 중" : "새로고침"}
              </button>
            </div>
          </summary>
          <div style={{ marginTop: 12 }}>
            <GameLogsPanel logs={gameLogs} onOpenPath={onOpenPath} />
          </div>
        </details>`
);

fs.writeFileSync('z:/game/sts2/modmanager/web/features/settings/SettingsPage.tsx', c);
