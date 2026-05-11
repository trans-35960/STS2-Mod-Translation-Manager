import React from "react";
import type { LucideIcon } from "lucide-react";

export function SourceTags({ value }: { value: string }) {
  const tags = value.split(" + ").filter(Boolean);
  if (tags.length === 0) {
    return null;
  }
  return (
    <div className="source-tags" title={value}>
      {tags.slice(0, 2).map((label) => (
        <span key={label}>{label}</span>
      ))}
      {tags.length > 2 && <span>+{tags.length - 2}</span>}
    </div>
  );
}

export function IconButton({
  label,
  icon,
  onClick,
  disabled,
  danger,
  active,
  loading,
}: {
  label: string;
  icon: LucideIcon;
  onClick: () => void;
  disabled?: boolean;
  danger?: boolean;
  active?: boolean;
  loading?: boolean;
}) {
  const Icon = icon;
  return (
    <button
      className={`icon-only-button${danger ? " danger" : ""}${active ? " active" : ""}`}
      type="button"
      aria-label={label}
      data-tooltip={label}
      onClick={onClick}
      disabled={disabled}
    >
      <Icon size={16} className={loading ? "spin-icon" : undefined} />
    </button>
  );
}

export function StatusBadge({ children, tone = "neutral" }: { children: React.ReactNode; tone?: "neutral" | "info" | "danger" }) {
  return <span className={`status-badge ${tone}`}>{children}</span>;
}

export function Pill({ children, tone }: { children: React.ReactNode; tone?: "good" | "warn" | "info" }) {
  return <span className={`pill ${tone ?? ""}`}>{children}</span>;
}
