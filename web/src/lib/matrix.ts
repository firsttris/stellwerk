import type { Device, Mode, Overview, ShareChange } from "../api/types";
import { m } from "../paraglide/messages.js";

// The folder × device matrix as data: what is stored, what the draft changes, what "Apply"
// sends. Pure functions, tested without React.

export type CellMode = Mode | "off";

/** A click moves a cell one step on. */
export const CYCLE: readonly CellMode[] = ["off", "sendReceive", "sendOnly", "receiveOnly"];

export const modeLabel = (mode: CellMode): string =>
  ({
    off: m.mode_off,
    sendReceive: m.mode_both,
    sendOnly: m.mode_send,
    receiveOnly: m.mode_receive,
  })[mode]();

export const modeLong = (mode: CellMode): string =>
  ({
    off: m.mode_long_off,
    sendReceive: m.mode_long_both,
    sendOnly: m.mode_long_send,
    receiveOnly: m.mode_long_receive,
  })[mode]();

/** Changed cells only, by [`cellKey`]. */
export type Draft = ReadonlyMap<string, CellMode>;

export const cellKey = (folderId: string, deviceId: string) => `${folderId}\u0000${deviceId}`;

export function storedMode(overview: Overview, folderId: string, deviceId: string): CellMode {
  const share = overview.shares.find((s) => s.folderId === folderId && s.deviceId === deviceId);
  return share?.mode ?? "off";
}

export function cellMode(
  overview: Overview,
  draft: Draft,
  folderId: string,
  deviceId: string,
): CellMode {
  return draft.get(cellKey(folderId, deviceId)) ?? storedMode(overview, folderId, deviceId);
}

export function nextMode(mode: CellMode): CellMode {
  const index = CYCLE.indexOf(mode);
  return CYCLE[(index + 1) % CYCLE.length] ?? "off";
}

/** Sets a cell in the draft; a cell back at its stored mode leaves the draft. */
export function setCell(
  overview: Overview,
  draft: Draft,
  folderId: string,
  deviceId: string,
  mode: CellMode,
): Draft {
  const next = new Map(draft);
  const key = cellKey(folderId, deviceId);
  if (mode === storedMode(overview, folderId, deviceId)) next.delete(key);
  else next.set(key, mode);
  return next;
}

export function cycleCell(
  overview: Overview,
  draft: Draft,
  folderId: string,
  deviceId: string,
): Draft {
  const mode = cellMode(overview, draft, folderId, deviceId);
  return setCell(overview, draft, folderId, deviceId, nextMode(mode));
}

/** Paths typed or picked for new shares, by [`cellKey`]. */
export type DraftPaths = Readonly<Record<string, string>>;

/** What "Apply" sends. Cells of folders or devices that are gone are dropped; a new share takes
 * its path from `paths` when one is given there, else the Zentrale's default. */
export function draftChanges(
  overview: Overview,
  draft: Draft,
  paths: DraftPaths = {},
): ShareChange[] {
  return [...draft].flatMap(([key, mode]): ShareChange[] => {
    const [folderId = "", deviceId = ""] = key.split("\u0000");
    const known =
      overview.folders.some((f) => f.id === folderId) &&
      overview.devices.some((d) => d.id === deviceId);
    if (!known) return [];
    if (mode === "off") return [{ folderId, deviceId, mode: null }];
    const path = storedMode(overview, folderId, deviceId) === "off" ? paths[key]?.trim() : "";
    return [path ? { folderId, deviceId, mode, path } : { folderId, deviceId, mode }];
  });
}

/** Draft cells that put a folder on a device that does not hold it yet: they need a path. */
export function newShares(
  overview: Overview,
  draft: Draft,
): { folderId: string; deviceId: string }[] {
  return draftChanges(overview, draft)
    .filter((c) => c.mode && storedMode(overview, c.folderId, c.deviceId) === "off")
    .map(({ folderId, deviceId }) => ({ folderId, deviceId }));
}

/** How many draft changes touch devices that are offline now. */
export function offlineChanges(overview: Overview, draft: Draft): number {
  return draftChanges(overview, draft).filter(
    (change) => !overview.devices.find((d) => d.id === change.deviceId)?.online,
  ).length;
}

/** An offline device that has not yet applied the current revision. */
export function isWaiting(device: Device, revision: number): boolean {
  return !device.online && (device.appliedRev ?? -1) < revision;
}

/** Devices shown as columns: approved ones, in the order the Zentrale lists them. */
export function matrixDevices(overview: Overview): Device[] {
  return overview.devices.filter((d) => d.approved);
}
