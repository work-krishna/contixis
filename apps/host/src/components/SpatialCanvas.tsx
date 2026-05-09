import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { useStore, type SpatialScreen } from "../store";
import { updateSpatialLayout } from "../lib/tauri";

const PADDING_FACTOR = 0.6;

interface Viewport {
  scale: number;
  offsetX: number;
  offsetY: number;
}

function computeViewport(screens: SpatialScreen[], canvasW: number, canvasH: number): Viewport {
  if (screens.length === 0 || canvasW === 0 || canvasH === 0) {
    return { scale: 0.1, offsetX: 0, offsetY: 0 };
  }
  const minX = Math.min(...screens.map(s => s.x));
  const minY = Math.min(...screens.map(s => s.y));
  const maxX = Math.max(...screens.map(s => s.x + s.widthPx));
  const maxY = Math.max(...screens.map(s => s.y + s.heightPx));
  const maxDim = Math.max(maxX - minX, maxY - minY, 1);
  const pad = maxDim * PADDING_FACTOR;
  const bboxW = maxX - minX + pad * 2;
  const bboxH = maxY - minY + pad * 2;
  const scale = Math.min(canvasW / bboxW, canvasH / bboxH) * 0.92;
  return {
    scale,
    offsetX: (canvasW - scale * bboxW) / 2 - (minX - pad) * scale,
    offsetY: (canvasH - scale * bboxH) / 2 - (minY - pad) * scale,
  };
}

// BFS: find all screens reachable by going right from the removed screen's right edge.
function findReflowIds(
  others: SpatialScreen[],
  rightX: number,
  fromY: number,
  fromH: number,
  tol = 4,
): Set<string> {
  const ids = new Set<string>();
  const queue: SpatialScreen[] = [];

  const addRight = (x: number, y: number, h: number) => {
    for (const s of others) {
      if (ids.has(s.deviceId)) continue;
      if (Math.abs(s.x - x) <= tol && s.y < y + h && s.y + s.heightPx > y) {
        ids.add(s.deviceId);
        queue.push(s);
      }
    }
  };

  addRight(rightX, fromY, fromH);
  while (queue.length) {
    const cur = queue.shift()!;
    addRight(cur.x + cur.widthPx, cur.y, cur.heightPx);
  }
  return ids;
}

// Shift right-adjacent screens left to fill the gap left by the removed screen.
function applyReflow(others: SpatialScreen[], removed: SpatialScreen): SpatialScreen[] {
  const ids = findReflowIds(others, removed.x + removed.widthPx, removed.y, removed.heightPx);
  return others.map(s => ids.has(s.deviceId) ? { ...s, x: s.x - removed.widthPx } : s);
}

// Generate all valid attachment points around every existing screen.
// All coordinates are rounded to integers to prevent float drift.
function snapCandidates(
  screens: SpatialScreen[],
  dw: number,
  dh: number,
): Array<{ x: number; y: number }> {
  const out: Array<{ x: number; y: number }> = [];
  for (const s of screens) {
    const rx = s.x + s.widthPx;
    const lx = s.x - dw;
    const by = s.y + s.heightPx;
    const ty = s.y - dh;
    // Right side – top, bottom, center Y alignments
    out.push({ x: rx, y: s.y });
    out.push({ x: rx, y: s.y + s.heightPx - dh });
    out.push({ x: rx, y: Math.round(s.y + (s.heightPx - dh) / 2) });
    // Left side
    out.push({ x: lx, y: s.y });
    out.push({ x: lx, y: s.y + s.heightPx - dh });
    out.push({ x: lx, y: Math.round(s.y + (s.heightPx - dh) / 2) });
    // Bottom – left, right, center X alignments
    out.push({ x: s.x, y: by });
    out.push({ x: s.x + s.widthPx - dw, y: by });
    out.push({ x: Math.round(s.x + (s.widthPx - dw) / 2), y: by });
    // Top
    out.push({ x: s.x, y: ty });
    out.push({ x: s.x + s.widthPx - dw, y: ty });
    out.push({ x: Math.round(s.x + (s.widthPx - dw) / 2), y: ty });
  }
  return out;
}

// Filter candidates that would strictly overlap (not merely touch) any existing screen.
// tol=1: a candidate at exactly the edge passes; ≥2px of real overlap is rejected.
function validSnapCandidates(
  candidates: Array<{ x: number; y: number }>,
  screens: SpatialScreen[],
  dw: number,
  dh: number,
): Array<{ x: number; y: number }> {
  return candidates.filter(c => {
    const x1 = c.x + 1, y1 = c.y + 1;
    const x2 = c.x + dw - 1, y2 = c.y + dh - 1;
    return !screens.some(s => x1 < s.x + s.widthPx && x2 > s.x && y1 < s.y + s.heightPx && y2 > s.y);
  });
}

function nearestSnap(
  candidates: Array<{ x: number; y: number }>,
  rawX: number,
  rawY: number,
  dw: number,
  dh: number,
): { x: number; y: number } | null {
  if (!candidates.length) return null;
  const cx = rawX + dw / 2, cy = rawY + dh / 2;
  let best = candidates[0];
  let bd = Infinity;
  for (const c of candidates) {
    const d = Math.hypot(c.x + dw / 2 - cx, c.y + dh / 2 - cy);
    if (d < bd) { bd = d; best = c; }
  }
  return best;
}

// Compute the best snap position for the dragged screen at (rawX, rawY).
function computeSnap(
  screens: SpatialScreen[],
  rawX: number,
  rawY: number,
  dw: number,
  dh: number,
): { x: number; y: number } | null {
  const cands = snapCandidates(screens, dw, dh);
  const valid = validSnapCandidates(cands, screens, dw, dh);
  return nearestSnap(valid, rawX, rawY, dw, dh);
}

interface DragState {
  deviceId: string;
  startMouseX: number;
  startMouseY: number;
  origX: number;
  origY: number;
  screen: SpatialScreen;
  // Scale captured from the post-reflow viewport so delta math is consistent.
  startScale: number;
}

export function SpatialCanvas() {
  const { spatialScreens } = useStore();
  const containerRef = useRef<HTMLDivElement>(null);
  const canvasSizeRef = useRef({ w: 0, h: 0 });
  const [canvasSize, setCanvasSize] = useState({ w: 0, h: 0 });
  const [drag, setDrag] = useState<DragState | null>(null);
  const [dragPos, setDragPos] = useState<{ x: number; y: number } | null>(null);
  const [snapPos, setSnapPos] = useState<{ x: number; y: number } | null>(null);

  useLayoutEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      const size = { w: el.clientWidth, h: el.clientHeight };
      canvasSizeRef.current = size;
      setCanvasSize(size);
    });
    ro.observe(el);
    const size = { w: el.clientWidth, h: el.clientHeight };
    canvasSizeRef.current = size;
    setCanvasSize(size);
    return () => ro.disconnect();
  }, []);

  // Viewport computed from settled screens only (never from the floating dragged screen).
  // This prevents the zoom-feedback loop: drag right → scale drops → delta grows → jumps.
  const vp = computeViewport(spatialScreens, canvasSize.w, canvasSize.h);

  const toCanvas = (wx: number, wy: number) => ({
    cx: wx * vp.scale + vp.offsetX,
    cy: wy * vp.scale + vp.offsetY,
  });

  const handleMouseDown = useCallback((e: React.MouseEvent, screen: SpatialScreen) => {
    if (screen.isHost) return;
    e.preventDefault();
    const state = useStore.getState();
    const others = state.spatialScreens.filter(s => s.deviceId !== screen.deviceId);
    const reflowed = applyReflow(others, screen);
    state.setSpatialScreens(reflowed);

    // Compute scale from the post-reflow viewport so mouse→workspace mapping
    // is correct from the very first mousemove.
    const { w, h } = canvasSizeRef.current;
    const postVP = computeViewport(reflowed, w, h);

    setDrag({
      deviceId: screen.deviceId,
      startMouseX: e.clientX,
      startMouseY: e.clientY,
      origX: screen.x,
      origY: screen.y,
      screen,
      startScale: postVP.scale,
    });
    setDragPos({ x: screen.x, y: screen.y });
    setSnapPos(null);
  }, []);

  useEffect(() => {
    if (!drag) return;

    const onMove = (e: MouseEvent) => {
      const rawX = Math.round(drag.origX + (e.clientX - drag.startMouseX) / drag.startScale);
      const rawY = Math.round(drag.origY + (e.clientY - drag.startMouseY) / drag.startScale);
      setDragPos({ x: rawX, y: rawY });

      const screens = useStore.getState().spatialScreens;
      setSnapPos(computeSnap(screens, rawX, rawY, drag.screen.widthPx, drag.screen.heightPx));
    };

    const onUp = async (e: MouseEvent) => {
      // Compute snap synchronously at drop time to avoid stale state from async renders.
      const rawX = Math.round(drag.origX + (e.clientX - drag.startMouseX) / drag.startScale);
      const rawY = Math.round(drag.origY + (e.clientY - drag.startMouseY) / drag.startScale);
      const screens = useStore.getState().spatialScreens;
      const snap = computeSnap(screens, rawX, rawY, drag.screen.widthPx, drag.screen.heightPx);
      const finalPos = snap ?? { x: rawX, y: rawY };

      const placed = { ...drag.screen, x: finalPos.x, y: finalPos.y };
      const all = [...screens, placed];
      useStore.getState().setSpatialScreens(all);

      setDrag(null);
      setDragPos(null);
      setSnapPos(null);

      await updateSpatialLayout(
        all.filter(s => !s.isHost).map(s => ({ deviceId: s.deviceId, x: s.x, y: s.y }))
      ).catch(console.error);
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [drag]);

  // Build render list: settled screens + ghost + dragged screen
  const renderedElements: React.ReactNode[] = [];

  for (const screen of spatialScreens) {
    const { cx, cy } = toCanvas(screen.x, screen.y);
    const cw = screen.widthPx * vp.scale;
    const ch = screen.heightPx * vp.scale;
    renderedElements.push(
      <div
        key={screen.deviceId}
        onMouseDown={(e) => handleMouseDown(e, screen)}
        style={{
          position: "absolute",
          left: cx, top: cy, width: cw, height: ch,
          background: screen.isHost ? "var(--accent-dim)" : "var(--cell-filled)",
          border: `2px solid ${screen.isHost ? "var(--accent)" : "var(--border)"}`,
          borderRadius: 4,
          boxSizing: "border-box",
          cursor: screen.isHost ? "default" : "grab",
          userSelect: "none",
          display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center",
          overflow: "hidden",
          transition: drag ? "left 0.12s ease-out, top 0.12s ease-out" : "none",
          zIndex: 1,
        }}
      >
        {screen.isHost && (
          <span style={{ fontSize: Math.max(9, cw * 0.045), color: "var(--accent)", fontWeight: 700, letterSpacing: 1, textTransform: "uppercase", lineHeight: 1.2 }}>HOST</span>
        )}
        <span style={{ fontSize: Math.max(10, cw * 0.055), fontWeight: 600, color: "var(--text)", textAlign: "center", padding: "0 4px", lineHeight: 1.3, maxWidth: "90%", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {screen.displayName}
        </span>
        <span style={{ fontSize: Math.max(8, cw * 0.035), color: "var(--text-dim)", marginTop: 2 }}>
          {screen.widthPx}×{screen.heightPx}
        </span>
        {!screen.isHost && (
          <span style={{ fontSize: Math.max(8, cw * 0.03), color: "var(--text-dim)", marginTop: 1, fontStyle: "italic" }}>drag to reposition</span>
        )}
      </div>
    );
  }

  // Ghost: dashed outline at the snap target position
  if (drag && snapPos) {
    const { cx, cy } = toCanvas(snapPos.x, snapPos.y);
    const cw = drag.screen.widthPx * vp.scale;
    const ch = drag.screen.heightPx * vp.scale;
    renderedElements.push(
      <div
        key="snap-ghost"
        style={{
          position: "absolute",
          left: cx, top: cy, width: cw, height: ch,
          border: "2px dashed #7fb3ff",
          borderRadius: 4,
          boxSizing: "border-box",
          pointerEvents: "none",
          zIndex: 8,
          opacity: 0.5,
        }}
      />
    );
  }

  // Dragged screen: follows cursor
  if (drag && dragPos) {
    const { cx, cy } = toCanvas(dragPos.x, dragPos.y);
    const cw = drag.screen.widthPx * vp.scale;
    const ch = drag.screen.heightPx * vp.scale;
    renderedElements.push(
      <div
        key={`drag-${drag.deviceId}`}
        style={{
          position: "absolute",
          left: cx, top: cy, width: cw, height: ch,
          background: "var(--cell-filled)",
          border: "2px solid #7fb3ff",
          borderRadius: 4,
          boxSizing: "border-box",
          cursor: "grabbing",
          userSelect: "none",
          display: "flex", flexDirection: "column", alignItems: "center", justifyContent: "center",
          overflow: "hidden",
          boxShadow: "0 8px 24px rgba(0,0,0,0.5)",
          zIndex: 10,
          opacity: 0.85,
        }}
      >
        <span style={{ fontSize: Math.max(10, cw * 0.055), fontWeight: 600, color: "var(--text)", textAlign: "center", padding: "0 4px", lineHeight: 1.3, maxWidth: "90%", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {drag.screen.displayName}
        </span>
        <span style={{ fontSize: Math.max(8, cw * 0.035), color: "var(--text-dim)", marginTop: 2 }}>
          {drag.screen.widthPx}×{drag.screen.heightPx}
        </span>
      </div>
    );
  }

  const totalCount = drag ? spatialScreens.length + 1 : spatialScreens.length;
  const dotSize = Math.max(16, 32 * vp.scale);

  return (
    <div
      ref={containerRef}
      style={{
        flex: 1,
        position: "relative",
        overflow: "hidden",
        background: "var(--bg)",
        backgroundImage: "radial-gradient(circle, #2a2d3a 1px, transparent 1px)",
        backgroundSize: `${dotSize}px ${dotSize}px`,
        backgroundPosition: `${vp.offsetX % dotSize}px ${vp.offsetY % dotSize}px`,
      }}
    >
      {totalCount === 0 ? (
        <div style={{ position: "absolute", inset: 0, display: "flex", alignItems: "center", justifyContent: "center", color: "var(--text-dim)", fontSize: 13, flexDirection: "column", gap: 8 }}>
          <span style={{ fontSize: 32 }}>⬜</span>
          <span>Start the server to see screens</span>
        </div>
      ) : (
        renderedElements
      )}
      {totalCount > 0 && (
        <div style={{ position: "absolute", bottom: 10, right: 14, fontSize: 10, color: "var(--text-dim)", fontFamily: "monospace" }}>
          {Math.round(vp.scale * 100)}% · {totalCount} screen{totalCount !== 1 ? "s" : ""}
        </div>
      )}
    </div>
  );
}
