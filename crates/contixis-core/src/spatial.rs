use serde::{Deserialize, Serialize};

/// Pixel-coordinate placement of a screen in the shared workspace.
/// All coordinates are in actual display pixels (same units as XRandR output).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenPlacement {
    pub device_id: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Free-form spatial layout: screens are placed at arbitrary pixel coordinates.
/// Replaces the grid system for cursor-transition logic on the host.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SpatialLayout {
    pub screens: Vec<ScreenPlacement>,
}

/// Maximum pixel gap between two screens that still allows a cursor transition.
const GAP_MAX: i32 = 300;

impl SpatialLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn place(&mut self, screen: ScreenPlacement) {
        if let Some(existing) = self.screens.iter_mut().find(|s| s.device_id == screen.device_id) {
            *existing = screen;
        } else {
            self.screens.push(screen);
        }
    }

    pub fn remove(&mut self, device_id: &str) {
        self.screens.retain(|s| s.device_id != device_id);
    }

    pub fn find(&self, device_id: &str) -> Option<&ScreenPlacement> {
        self.screens.iter().find(|s| s.device_id == device_id)
    }

    pub fn find_mut(&mut self, device_id: &str) -> Option<&mut ScreenPlacement> {
        self.screens.iter_mut().find(|s| s.device_id == device_id)
    }

    /// X coordinate of the right edge of the rightmost screen.
    pub fn rightmost_x(&self) -> i32 {
        self.screens.iter()
            .map(|s| s.x + s.width as i32)
            .max()
            .unwrap_or(0)
    }

    /// Y coordinate of the topmost screen (smallest y).
    pub fn top_y(&self) -> i32 {
        self.screens.iter().map(|s| s.y).min().unwrap_or(0)
    }

    /// Find the screen a cursor transitions to when leaving `from_id` at normalised
    /// coordinates (exit_norm_x, exit_norm_y).  Returns (target, entry_norm_x, entry_norm_y).
    ///
    /// Algorithm: determine the primary exit direction from which axis is more out of
    /// [0,1].  Then find the geometrically nearest screen in that direction whose
    /// perpendicular span contains the exit pixel, within GAP_MAX pixels.
    ///
    /// Corner fallback: if no neighbor exists in the primary direction and the cursor is
    /// in the outer quarter of the screen along the perpendicular axis (e.g. bottom-left
    /// corner), also check the perpendicular direction.  This lets a cursor that hits a
    /// dead bottom/top edge while sliding toward a left/right neighbor still transition.
    pub fn edge_transition(
        &self,
        from_id: &str,
        exit_norm_x: f32,
        exit_norm_y: f32,
    ) -> Option<(&ScreenPlacement, f32, f32)> {
        let from = self.find(from_id)?;
        let fw = from.width as i32;
        let fh = from.height as i32;

        // How far each axis is outside [0,1].
        let dx_oob = if exit_norm_x < 0.0 { -exit_norm_x }
                     else if exit_norm_x > 1.0 { exit_norm_x - 1.0 }
                     else { 0.0 };
        let dy_oob = if exit_norm_y < 0.0 { -exit_norm_y }
                     else if exit_norm_y > 1.0 { exit_norm_y - 1.0 }
                     else { 0.0 };

        // Primary direction: 0=left 1=right 2=up 3=down
        let dir: u8 = if dx_oob >= dy_oob {
            if exit_norm_x < 0.0 { 0 } else { 1 }
        } else {
            if exit_norm_y < 0.0 { 2 } else { 3 }
        };

        // Absolute exit pixel, clamped to the screen edge on the primary axis.
        let exit_px_x = (from.x as f32 + exit_norm_x * fw as f32)
            .clamp(from.x as f32, (from.x + fw) as f32);
        let exit_px_y = (from.y as f32 + exit_norm_y * fh as f32)
            .clamp(from.y as f32, (from.y + fh) as f32);

        if let Some(r) = self.search_direction(from, fw, fh, dir, exit_px_x, exit_px_y) {
            return Some(r);
        }

        // Dead-edge fallback: try both perpendicular directions, closer half first.
        // This handles the common case where the cursor sweeps diagonally and hits
        // a top/bottom edge before reaching the left/right edge where the agent sits.
        let (perp_a, perp_b): (u8, u8) = match dir {
            // Top/bottom dead edge — try left/right
            2 | 3 => if exit_norm_x <= 0.5 { (0, 1) } else { (1, 0) },
            // Left/right dead edge — try up/down
            _ => if exit_norm_y <= 0.5 { (2, 3) } else { (3, 2) },
        };

        if let Some(r) = self.search_direction(from, fw, fh, perp_a, exit_px_x, exit_px_y) {
            return Some(r);
        }
        self.search_direction(from, fw, fh, perp_b, exit_px_x, exit_px_y)
    }

    /// Search for the nearest neighbor of `from` in the given direction `dir`.
    /// `exit_px_x/y` are the absolute cursor coordinates clamped to the screen boundary.
    fn search_direction<'a>(
        &'a self,
        from: &ScreenPlacement,
        fw: i32,
        fh: i32,
        dir: u8,
        exit_px_x: f32,
        exit_px_y: f32,
    ) -> Option<(&'a ScreenPlacement, f32, f32)> {
        let mut best: Option<(&ScreenPlacement, f32, f32, i32)> = None;

        for screen in &self.screens {
            if screen.device_id == from.device_id { continue; }

            let sx = screen.x;
            let sy = screen.y;
            let sw = screen.width as i32;
            let sh = screen.height as i32;

            let (gap, perp_ok, enx, eny) = match dir {
                // Left: target's right edge must be ≤ from's left edge
                0 => {
                    let gap = from.x - (sx + sw);
                    let ok  = (sy + sh) > from.y && sy < (from.y + fh);
                    let eny = ((exit_px_y - sy as f32) / sh as f32).clamp(0.0, 1.0);
                    (gap, ok, 1.0f32, eny)
                }
                // Right: target's left edge must be ≥ from's right edge
                1 => {
                    let gap = sx - (from.x + fw);
                    let ok  = (sy + sh) > from.y && sy < (from.y + fh);
                    let eny = ((exit_px_y - sy as f32) / sh as f32).clamp(0.0, 1.0);
                    (gap, ok, 0.0f32, eny)
                }
                // Up: target's bottom edge must be ≤ from's top edge
                2 => {
                    let gap = from.y - (sy + sh);
                    let ok  = (sx + sw) > from.x && sx < (from.x + fw);
                    let enx = ((exit_px_x - sx as f32) / sw as f32).clamp(0.0, 1.0);
                    (gap, ok, enx, 1.0f32)
                }
                // Down: target's top edge must be ≥ from's bottom edge
                _ => {
                    let gap = sy - (from.y + fh);
                    let ok  = (sx + sw) > from.x && sx < (from.x + fw);
                    let enx = ((exit_px_x - sx as f32) / sw as f32).clamp(0.0, 1.0);
                    (gap, ok, enx, 0.0f32)
                }
            };

            if gap >= 0 && gap <= GAP_MAX && perp_ok {
                if best.map_or(true, |(_, _, _, bg)| gap < bg) {
                    best = Some((screen, enx, eny, gap));
                }
            }
        }

        best.map(|(s, ex, ey, _)| (s, ex, ey))
    }
}
