import { useStore } from "../store";
import { GridCell } from "./GridCell";
import { updateGridLayout } from "../lib/tauri";

export function GridCanvas() {
  const { grid, gridRows, gridCols, devices, hostCells, placeDevice, removeFromGrid } =
    useStore();

  const getHostCell = (row: number, col: number) =>
    (hostCells ?? []).find((h) => h.row === row && h.col === col) ?? null;

  const handleDrop = async (deviceId: string, row: number, col: number) => {
    placeDevice(deviceId, row, col);
    await updateGridLayout(
      useStore.getState().grid.map((c) => ({
        row: c.row,
        col: c.col,
        deviceId: c.deviceId,
        screenId: c.screenId,
      }))
    ).catch(console.error);
  };

  const handleClear = (row: number, col: number) => {
    const cell = grid.find((c) => c.row === row && c.col === col);
    if (cell?.deviceId) {
      removeFromGrid(cell.deviceId);
    }
  };

  return (
    <div
      style={{
        display: "grid",
        gridTemplateColumns: `repeat(${gridCols}, 1fr)`,
        gridTemplateRows: `repeat(${gridRows}, 1fr)`,
        gap: 8,
        flex: 1,
        padding: 16,
      }}
    >
      {grid.map((cell) => (
        <GridCell
          key={`${cell.row}-${cell.col}`}
          cell={cell}
          device={cell.deviceId ? devices[cell.deviceId] : undefined}
          hostCell={getHostCell(cell.row, cell.col)}
          onDrop={handleDrop}
          onClear={handleClear}
        />
      ))}
    </div>
  );
}
