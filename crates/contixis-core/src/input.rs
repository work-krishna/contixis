use crate::grid::{GridPosition, VirtualGrid};
use parking_lot::RwLock;
use std::sync::Arc;

/// Tracks which device currently owns keyboard/mouse focus and routes
/// input events to the correct destination.
pub struct InputRouter {
    grid: Arc<RwLock<VirtualGrid>>,
    focused: Arc<RwLock<Option<FocusTarget>>>,
}

#[derive(Debug, Clone)]
pub struct FocusTarget {
    pub device_id: String,
    pub position: GridPosition,
}

impl InputRouter {
    pub fn new(grid: Arc<RwLock<VirtualGrid>>) -> Self {
        Self {
            grid,
            focused: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns the device that currently holds focus, if any.
    pub fn focused_device(&self) -> Option<FocusTarget> {
        self.focused.read().clone()
    }

    /// Explicitly grant focus to a device.
    pub fn grant_focus(&self, device_id: String, position: GridPosition) {
        *self.focused.write() = Some(FocusTarget { device_id, position });
    }

    /// Release focus (no device active — events go to the host).
    pub fn release_focus(&self) {
        *self.focused.write() = None;
    }

    /// Called on every host mouse move.  If the cursor crosses a grid edge,
    /// returns the neighbour device to receive focus and the re-mapped coords.
    /// Returns None if still within the current focused device's bounds.
    pub fn maybe_transfer(
        &self,
        nx: f32,
        ny: f32,
    ) -> Option<TransferDecision> {
        let focused = self.focused.read().clone()?;
        let grid = self.grid.read();

        if let Some((screen, new_pos, new_nx, new_ny)) =
            grid.edge_transition(focused.position, nx, ny)
        {
            Some(TransferDecision {
                from_device: focused.device_id.clone(),
                to_device: screen.device_id.clone(),
                to_position: new_pos,
                nx: new_nx,
                ny: new_ny,
            })
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferDecision {
    pub from_device: String,
    pub to_device: String,
    pub to_position: GridPosition,
    pub nx: f32,
    pub ny: f32,
}
