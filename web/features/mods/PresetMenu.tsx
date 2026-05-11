import React from "react";
import { Archive } from "lucide-react";
import { labels } from "../../i18n";
import type { Preset } from "../../types";

function PresetMenu(props: {
  labels: typeof labels.ko;
  presets: Preset[];
  selectedPreset: string;
  presetName: string;
  archivePath: string;
  busy: string | null;
  setSelectedPreset: (value: string) => void;
  setPresetName: (value: string) => void;
  setArchivePath: (value: string) => void;
  onSave: () => void;
  onApply: () => void;
  onExport: () => void;
  onImport: () => void;
}) {
  const t = props.labels;
  const [open, setOpen] = React.useState(false);
  const selected = props.presets.find((preset) => preset.name === props.selectedPreset);
  return (
    <div className="preset-menu">
      <button className="toolbar-icon-button" type="button" onClick={() => setOpen((value) => !value)} aria-expanded={open} aria-label={t.presets} data-tooltip={t.presets}>
        <Archive size={16} />
      </button>
      {open && (
        <div className="preset-popover">
          <label>
            <span>{t.presetName}</span>
            <input value={props.presetName} onChange={(event) => props.setPresetName(event.target.value)} placeholder="balanced-korean" />
          </label>
          <button className="primary" onClick={props.onSave} disabled={Boolean(props.busy) || !props.presetName.trim()}>
            {t.savePreset}
          </button>
          <label>
            <span>{t.presets}</span>
            <select value={props.selectedPreset} onChange={(event) => props.setSelectedPreset(event.target.value)}>
              {props.presets.map((preset) => <option value={preset.name} key={preset.name}>{preset.name}</option>)}
            </select>
          </label>
          <button onClick={props.onApply} disabled={!props.selectedPreset || Boolean(props.busy)}>{t.apply}</button>
          {selected && (
            <div className="preset-preview">
              <strong>{selected.name}</strong>
              <small>{selected.mod_count} mods</small>
              <ul>
                {selected.mods.slice(0, 5).map((mod) => (
                  <li key={mod.key}>
                    <span>{mod.key}</span>
                    <small>{mod.version_hint ?? mod.file_name ?? "-"}</small>
                  </li>
                ))}
              </ul>
            </div>
          )}
          {props.presets.length === 0 && <div className="empty compact">{t.noPresets}</div>}
          <label>
            <span>{t.archivePath}</span>
            <input value={props.archivePath} onChange={(event) => props.setArchivePath(event.target.value)} placeholder="Z:/game/sts2/modmanager/exports/preset.zip" />
          </label>
          <div className="button-row">
            <button onClick={props.onExport} disabled={!props.selectedPreset || Boolean(props.busy)}>{t.exportZip}</button>
            <button onClick={props.onImport} disabled={Boolean(props.busy)}>{t.importZip}</button>
          </div>
        </div>
      )}
    </div>
  );
}

export { PresetMenu };
