import { useEffect, useRef, useState } from "react";
import { useStore } from "../store";
import { invoke } from "@tauri-apps/api/core";

// ── Shortcut helpers ─────────────────────────────────────────────────────────

// Maps browser KeyboardEvent.key to X11 keysym (same values the evdev hook emits).
const KEYSYM_MAP: Record<string, number> = {
  Escape: 0xFF1B, Enter: 0xFF0D, Tab: 0xFF09, Backspace: 0xFF08,
  Delete: 0xFFFF, Home: 0xFF50, End: 0xFF57, Insert: 0xFF63,
  PageUp: 0xFF55, PageDown: 0xFF56,
  ArrowLeft: 0xFF51, ArrowUp: 0xFF52, ArrowRight: 0xFF53, ArrowDown: 0xFF54,
  F1: 0xFFBE, F2: 0xFFBF, F3: 0xFFC0, F4: 0xFFC1,
  F5: 0xFFC2, F6: 0xFFC3, F7: 0xFFC4, F8: 0xFFC5,
  F9: 0xFFC6, F10: 0xFFC7, F11: 0xFFC8, F12: 0xFFC9,
};

function keyToKeysym(key: string): number | null {
  if (key.length === 1) {
    const c = key.toLowerCase().charCodeAt(0);
    if ((c >= 0x61 && c <= 0x7a) || (c >= 0x30 && c <= 0x39)) return c;
  }
  return KEYSYM_MAP[key] ?? null;
}

function keysymToName(ks: number): string {
  if (ks >= 0x61 && ks <= 0x7a) return String.fromCharCode(ks).toUpperCase();
  if (ks >= 0x30 && ks <= 0x39) return String.fromCharCode(ks);
  const m: Record<number, string> = {
    0xFF1B: "Esc", 0xFF0D: "Enter", 0xFF09: "Tab", 0xFF08: "Bksp",
    0xFFFF: "Del", 0xFF50: "Home", 0xFF57: "End", 0xFF63: "Ins",
    0xFF55: "PgUp", 0xFF56: "PgDn",
    0xFF51: "←", 0xFF52: "↑", 0xFF53: "→", 0xFF54: "↓",
    0xFFBE: "F1", 0xFFBF: "F2", 0xFFC0: "F3", 0xFFC1: "F4",
    0xFFC2: "F5", 0xFFC3: "F6", 0xFFC4: "F7", 0xFFC5: "F8",
    0xFFC6: "F9", 0xFFC7: "F10", 0xFFC8: "F11", 0xFFC9: "F12",
  };
  return m[ks] ?? `?`;
}

// Modifier bit flags matching contixis_proto::modifiers (SHIFT=1, CTRL=2, ALT=4, META=8).
function formatShortcut(keysym: number, mods: number): string {
  const parts: string[] = [];
  if (mods & 2) parts.push("Ctrl");
  if (mods & 4) parts.push("Alt");
  if (mods & 1) parts.push("Shift");
  if (mods & 8) parts.push("Meta");
  parts.push(keysymToName(keysym));
  return parts.join("+");
}

// ── Sub-components ───────────────────────────────────────────────────────────

function Toggle({ checked, onChange }: { checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <button
      onClick={() => onChange(!checked)}
      style={{
        width: 40, height: 22, borderRadius: 11,
        background: checked ? "var(--accent)" : "var(--border)",
        border: "none", padding: 0, cursor: "pointer",
        position: "relative", transition: "background 0.2s", flexShrink: 0,
      }}
    >
      <span style={{
        position: "absolute", top: 3, left: checked ? 21 : 3,
        width: 16, height: 16, borderRadius: "50%",
        background: "#fff", transition: "left 0.2s", display: "block",
      }} />
    </button>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{
      display: "flex", alignItems: "center", justifyContent: "space-between",
      gap: 12, padding: "10px 0", borderBottom: "1px solid var(--border)",
    }}>
      <span style={{ fontSize: 13, color: "var(--text)" }}>{label}</span>
      {children}
    </div>
  );
}

function ShortcutRecorder({
  keysym, modifiers, onSave,
}: {
  keysym: number;
  modifiers: number;
  onSave: (ks: number, mods: number) => void;
}) {
  const [recording, setRecording] = useState(false);
  const recorderRef = useRef<HTMLDivElement>(null);

  const handleKeyDown = (e: React.KeyboardEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;
    const ks = keyToKeysym(e.key);
    if (ks === null) return;
    const mods =
      (e.ctrlKey  ? 2 : 0) |
      (e.altKey   ? 4 : 0) |
      (e.shiftKey ? 1 : 0) |
      (e.metaKey  ? 8 : 0);
    onSave(ks, mods);
    setRecording(false);
  };

  if (recording) {
    return (
      <div
        ref={recorderRef}
        tabIndex={0}
        onKeyDown={handleKeyDown}
        onBlur={() => setRecording(false)}
        autoFocus
        style={{
          fontSize: 11, fontFamily: "monospace",
          background: "var(--accent-dim)", color: "var(--accent)",
          border: "1px dashed var(--accent)",
          borderRadius: 4, padding: "3px 8px",
          outline: "none", cursor: "default", userSelect: "none",
          animation: "pulse 1s ease-in-out infinite",
        }}
      >
        Press key combo…
      </div>
    );
  }

  return (
    <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
      <span style={{
        fontSize: 11, fontFamily: "monospace",
        background: "var(--border)", color: "var(--text)",
        padding: "2px 8px", borderRadius: 4,
      }}>
        {formatShortcut(keysym, modifiers)}
      </span>
      <button
        onClick={() => setRecording(true)}
        style={{
          fontSize: 11, padding: "2px 8px", borderRadius: 4,
          background: "transparent", color: "var(--text-dim)",
          border: "1px solid var(--border)", cursor: "pointer",
        }}
      >
        Change
      </button>
    </div>
  );
}

// ── Main panel ───────────────────────────────────────────────────────────────

export function SettingsPanel() {
  const { showSettings, setShowSettings, theme, setTheme } = useStore();
  const [autostart, setAutostart]   = useState(false);
  const [autostartBusy, setAutostartBusy] = useState(false);
  const [relKs,  setRelKs]  = useState(0x0068); // default: h
  const [relMods, setRelMods] = useState(0x06);  // default: Ctrl+Alt

  useEffect(() => {
    if (!showSettings) return;
    invoke<boolean>("get_autostart").then(setAutostart).catch(() => {});
    invoke<[number, number]>("get_release_shortcut")
      .then((pair) => { setRelKs(pair[0]); setRelMods(pair[1]); })
      .catch(() => {});
  }, [showSettings]);

  const handleAutostart = async (v: boolean) => {
    setAutostartBusy(true);
    try {
      await invoke("toggle_autostart", { enable: v });
      setAutostart(v);
    } catch {
      // silently ignore on platforms where autostart isn't supported
    } finally {
      setAutostartBusy(false);
    }
  };

  const handleShortcutSave = async (ks: number, mods: number) => {
    try {
      await invoke("set_release_shortcut", { keysym: ks, modifiers: mods });
      setRelKs(ks);
      setRelMods(mods);
    } catch (e) {
      console.error("set_release_shortcut failed:", e);
    }
  };

  if (!showSettings) return null;

  return (
    <>
      {/* Backdrop */}
      <div
        onClick={() => setShowSettings(false)}
        style={{ position: "fixed", inset: 0, zIndex: 90 }}
      />

      {/* Panel */}
      <div
        style={{
          position: "fixed", top: 40, right: 0, width: 300,
          background: "var(--surface)",
          borderLeft: "1px solid var(--border)",
          borderBottom: "1px solid var(--border)",
          borderBottomLeftRadius: 8,
          zIndex: 100, padding: "12px 16px 16px",
          boxShadow: "-4px 4px 20px rgba(0,0,0,0.3)",
        }}
      >
        <div style={{ fontWeight: 700, fontSize: 12, color: "var(--text-dim)", letterSpacing: 0.5, textTransform: "uppercase", marginBottom: 8 }}>
          Settings
        </div>

        <Row label="Theme">
          <div style={{ display: "flex", gap: 6 }}>
            {(["dark", "light"] as const).map(t => (
              <button
                key={t}
                onClick={() => setTheme(t)}
                style={{
                  padding: "3px 10px", borderRadius: 6, fontSize: 12,
                  background: theme === t ? "var(--accent)" : "transparent",
                  color: theme === t ? "#fff" : "var(--text-dim)",
                  border: `1px solid ${theme === t ? "var(--accent)" : "var(--border)"}`,
                  cursor: "pointer", textTransform: "capitalize",
                }}
              >
                {t}
              </button>
            ))}
          </div>
        </Row>

        <Row label="Auto-start on login">
          <Toggle checked={autostart} onChange={autostartBusy ? () => {} : handleAutostart} />
        </Row>

        <div style={{ marginTop: 16, marginBottom: 4, fontWeight: 700, fontSize: 12, color: "var(--text-dim)", letterSpacing: 0.5, textTransform: "uppercase" }}>
          Keyboard Shortcuts
        </div>

        <div style={{ display: "flex", flexDirection: "column", gap: 2 }}>
          <div style={{ padding: "8px 0", borderBottom: "1px solid var(--border)" }}>
            <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 6 }}>
              Release focus to host
            </div>
            <ShortcutRecorder
              keysym={relKs}
              modifiers={relMods}
              onSave={handleShortcutSave}
            />
          </div>

          <div style={{ display: "flex", justifyContent: "space-between", padding: "8px 0" }}>
            <span style={{ fontSize: 12, color: "var(--text-dim)" }}>Switch screen</span>
            <span style={{ fontSize: 11, fontFamily: "monospace", background: "var(--border)", color: "var(--text)", padding: "2px 6px", borderRadius: 4 }}>
              Move cursor to edge
            </span>
          </div>
        </div>
      </div>
    </>
  );
}
