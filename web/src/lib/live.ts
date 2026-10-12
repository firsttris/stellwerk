import type { Device, Live, Overview } from "../api/types";
import { m } from "../paraglide/messages.js";
import { byCount, intlLocale } from "./i18n";

// What the live state means for a folder or a device, in the UI's words. Pure functions.

export type Tone = "neutral" | "ok" | "info" | "warn";
export type Badge = { tone: Tone; text: string };

export const formatCount = (n: number) => new Intl.NumberFormat(intlLocale()).format(n);

/** "18,4 GB", "512 KB", "0 B". */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit++;
  }
  const decimal = new Intl.NumberFormat(intlLocale(), { maximumFractionDigits: 1 });
  return `${unit === 0 ? value : decimal.format(value)} ${units[unit]}`;
}

/** How much of a folder a device has, from the hub's view; `undefined` if unknown. */
export function completion(live: Live | undefined, folderId: string, deviceId: string) {
  return live?.hub.completion[folderId]?.[deviceId];
}

/** Devices holding the folder. */
function holders(overview: Overview, folderId: string): Device[] {
  const ids = new Set(
    overview.shares.filter((s) => s.folderId === folderId).map((s) => s.deviceId),
  );
  return overview.devices.filter((d) => d.approved && ids.has(d.id));
}

/** Errors in a folder anywhere: on the hub or on a device. */
export function folderErrors(overview: Overview, live: Live | undefined, folderId: string) {
  const found: { device: string; message: string }[] = [];
  const hub = live?.hub.folders[folderId];
  if (hub?.error) found.push({ device: m.server(), message: hub.error });
  for (const device of holders(overview, folderId)) {
    const status = live?.devices[device.id]?.folders[folderId];
    if (!device.online || !status) continue;
    if (status.error) found.push({ device: device.name, message: status.error });
    else if (status.errors > 0)
      found.push({ device: device.name, message: m.error_files_failed({ count: status.errors }) });
  }
  return found;
}

/** The badge next to a folder: conflicts, error, running with the lowest progress, in sync. */
export function folderBadge(overview: Overview, live: Live | undefined, folderId: string): Badge {
  if (!live) return { tone: "neutral", text: "…" };
  const conflicts = live.conflicts[folderId]?.length ?? 0;
  if (conflicts > 0) {
    return {
      tone: "warn",
      text: byCount(conflicts, m.badge_conflicts_one, () =>
        m.badge_conflicts_other({ count: conflicts }),
      ),
    };
  }
  if (folderErrors(overview, live, folderId).length > 0) {
    return { tone: "warn", text: m.badge_error() };
  }
  const running = holders(overview, folderId)
    .filter((d) => d.online)
    .map((d) => completion(live, folderId, d.id)?.percent ?? 100)
    .filter((p) => p < 100);
  const hub = live.hub.folders[folderId];
  if (running.length > 0) {
    return {
      tone: "info",
      text: m.badge_running_percent({ percent: Math.floor(Math.min(...running)) }),
    };
  }
  if (hub && (hub.state === "syncing" || hub.state === "scanning")) {
    return { tone: "info", text: m.badge_running() };
  }
  if (holders(overview, folderId).length === 0) {
    return { tone: "neutral", text: m.badge_server_only() };
  }
  return { tone: "ok", text: m.badge_in_sync() };
}

/** "48.210 Dateien · 18,4 GB" from the hub, which has every folder in full. */
export function folderSize(live: Live | undefined, folderId: string): string | undefined {
  const hub = live?.hub.folders[folderId];
  if (!hub) return undefined;
  const size = formatBytes(hub.globalBytes);
  return byCount(
    hub.globalFiles,
    () => m.folder_size_one({ size }),
    () => m.folder_size_other({ count: formatCount(hub.globalFiles), size }),
  );
}

/** Changes the hub holds for a device that it does not have yet, over all its folders. */
export function waitingChanges(overview: Overview, live: Live | undefined, deviceId: string) {
  return overview.shares
    .filter((s) => s.deviceId === deviceId)
    .reduce((sum, s) => sum + (completion(live, s.folderId, deviceId)?.needItems ?? 0), 0);
}

/** Lowest progress of a device over its folders, `undefined` when all are complete. */
export function deviceProgress(overview: Overview, live: Live | undefined, deviceId: string) {
  const percents = overview.shares
    .filter((s) => s.deviceId === deviceId)
    .map((s) => completion(live, s.folderId, deviceId)?.percent ?? 100)
    .filter((p) => p < 100);
  return percents.length > 0 ? Math.floor(Math.min(...percents)) : undefined;
}

/** The second line of a device's status. */
export function deviceSummary(overview: Overview, live: Live | undefined, device: Device): string {
  if (!device.approved) return m.device_gets_no_folders();
  const waiting = waitingChanges(overview, live, device.id);
  const count = formatCount(waiting);
  if (!device.online) {
    if (waiting === 0) return m.device_all_on_server();
    return byCount(waiting, m.device_waiting_one, () => m.device_waiting_other({ count }));
  }
  const percent = deviceProgress(overview, live, device.id);
  if (percent === undefined) return m.device_in_sync();
  return byCount(
    waiting,
    () => m.device_loading_one({ percent }),
    () => m.device_loading_other({ count, percent }),
  );
}
