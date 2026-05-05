use serde::{Deserialize, Serialize};

/// Logical position of a device in the virtual grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct GridPosition {
    pub row: u8,
    pub col: u8,
}

/// Physical screen geometry reported by an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInfo {
    pub device_id: String,
    pub width_px: u32,
    pub height_px: u32,
    pub scale_factor: f32,
}

/// One cell in the virtual grid — may be occupied or empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridCell {
    pub position: GridPosition,
    pub screen: Option<ScreenInfo>,
}

/// NxM virtual display grid.
///
/// Coordinates exposed to the rest of the system are normalised [0.0, 1.0]
/// within each cell's physical bounds, so that mouse positions remain
/// meaningful even after a device changes resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualGrid {
    pub rows: u8,
    pub cols: u8,
    cells: Vec<GridCell>,
}

impl VirtualGrid {
    pub fn new(rows: u8, cols: u8) -> Self {
        let mut cells = Vec::with_capacity((rows * cols) as usize);
        for r in 0..rows {
            for c in 0..cols {
                cells.push(GridCell {
                    position: GridPosition { row: r, col: c },
                    screen: None,
                });
            }
        }
        Self { rows, cols, cells }
    }

    fn idx(&self, pos: GridPosition) -> Option<usize> {
        if pos.row < self.rows && pos.col < self.cols {
            Some(pos.row as usize * self.cols as usize + pos.col as usize)
        } else {
            None
        }
    }

    pub fn place_screen(&mut self, pos: GridPosition, screen: ScreenInfo) -> bool {
        if let Some(i) = self.idx(pos) {
            self.cells[i].screen = Some(screen);
            true
        } else {
            false
        }
    }

    pub fn remove_screen(&mut self, device_id: &str) {
        for cell in &mut self.cells {
            if let Some(s) = &cell.screen {
                if s.device_id == device_id {
                    cell.screen = None;
                    break;
                }
            }
        }
    }

    pub fn get_cell(&self, pos: GridPosition) -> Option<&GridCell> {
        self.idx(pos).map(|i| &self.cells[i])
    }

    pub fn find_device(&self, device_id: &str) -> Option<GridPosition> {
        self.cells.iter().find_map(|c| {
            c.screen.as_ref().and_then(|s| {
                if s.device_id == device_id { Some(c.position) } else { None }
            })
        })
    }

    /// Given a cursor leaving `from_pos` at normalised coordinates (nx, ny),
    /// return the device and re-mapped normalised coordinates on the neighbour,
    /// or None if the edge is off-grid.
    pub fn edge_transition(
        &self,
        from_pos: GridPosition,
        nx: f32,
        ny: f32,
    ) -> Option<(&ScreenInfo, GridPosition, f32, f32)> {
        let (dr, dc, mapped_x, mapped_y): (i16, i16, f32, f32) = if nx < 0.0 {
            (-1, 0, 1.0 + nx, ny)  // wait, left edge: nx < 0 means col - 1
        } else if nx > 1.0 {
            (0, 1, nx - 1.0, ny)
        } else if ny < 0.0 {
            (-1, 0, nx, 1.0 + ny)  // actually row - 1
        } else if ny > 1.0 {
            (1, 0, nx, ny - 1.0)
        } else {
            return None;
        };

        // Recalculate correctly: left/right changes col, up/down changes row.
        let (dr, dc, mapped_x, mapped_y): (i16, i16, f32, f32) = if nx < 0.0 {
            (0, -1, 1.0 + nx, ny)
        } else if nx > 1.0 {
            (0, 1, nx - 1.0, ny)
        } else if ny < 0.0 {
            (-1, 0, nx, 1.0 + ny)
        } else {
            (1, 0, nx, ny - 1.0)
        };

        let new_row = from_pos.row as i16 + dr;
        let new_col = from_pos.col as i16 + dc;
        if new_row < 0 || new_col < 0 {
            return None;
        }
        let new_pos = GridPosition { row: new_row as u8, col: new_col as u8 };
        let cell = self.get_cell(new_pos)?;
        let screen = cell.screen.as_ref()?;
        Some((screen, new_pos, mapped_x.clamp(0.0, 1.0), mapped_y.clamp(0.0, 1.0)))
    }

    pub fn cells(&self) -> &[GridCell] { &self.cells }
}
