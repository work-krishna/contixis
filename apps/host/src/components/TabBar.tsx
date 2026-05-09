type Tab = "view" | "help" | "about";

interface TabBarProps {
  active: Tab;
  onChange: (t: Tab) => void;
}

const TABS: { id: Tab; label: string }[] = [
  { id: "view",  label: "Layout" },
  { id: "help",  label: "Help"  },
  { id: "about", label: "About" },
];

export function TabBar({ active, onChange }: TabBarProps) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "stretch",
        height: 34,
        background: "var(--surface)",
        borderBottom: "1px solid var(--border)",
        padding: "0 8px",
        gap: 2,
        flexShrink: 0,
      }}
    >
      {TABS.map(t => (
        <button
          key={t.id}
          onClick={() => onChange(t.id)}
          style={{
            padding: "0 14px",
            height: "100%",
            borderRadius: 0,
            background: "transparent",
            color: active === t.id ? "var(--accent)" : "var(--text-dim)",
            fontWeight: active === t.id ? 600 : 400,
            fontSize: 13,
            border: "none",
            borderBottom: active === t.id ? "2px solid var(--accent)" : "2px solid transparent",
            cursor: "pointer",
            transition: "color 0.15s, border-color 0.15s",
          }}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}
