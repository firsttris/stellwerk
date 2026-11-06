import type { Device, Live, Overview } from "../api/types";
import { m } from "../paraglide/messages.js";
import { byCount, intlLocale } from "./i18n";
import { isWaiting } from "./matrix";

// What the live state means for a folder or a device, in the UI's words. Pure functions.

export type Tone = "neutral" | "ok" | "info" | "warn";
export type Badge = { tone: Tone; text: string };

export const formatCount = (n: number) => new Intl.NumberFormat(intlLocale()).format(n);

/** "18,4 GB", "512 KB", "0 B". */
export function formatBytes(bytes: number): string {
  const units = ["B", "KB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  // Compare what will be shown: 999 990 B rounds to "1.000 KB", which reads as "1 MB".
  const shown = (v: number) => Math.round(v * 10) / 10;
  while (shown(value) >= 1000 && unit < units.length - 1) {
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

/**
 * What a device really still has to fetch. The hub counts every item changed locally in a
 * receive-only folder as missing (not the ones only the device has, which it never heard of),
 * so a device reporting such changes corrects its numbers:
 * what it lacks itself, or what is left of the hub's count without them, whichever is more
 * (the hub may hold changes the device has not heard of yet). `undefined` if unknown.
 */
export function syncProgress(live: Live | undefined, folderId: string, deviceId: string) {
  const done = completion(live, folderId, deviceId);
  if (!done) return undefined;
  const own = live?.devices[deviceId]?.folders[folderId];
  const localChanges = own?.localChanges ?? 0;
  if (!own || localChanges === 0) {
    return { percent: done.percent, missing: done.needItems, localChanges };
  }
  const counted = own.localChangesOnHub ?? localChanges;
  const missing = Math.max(own.needItems ?? 0, done.needItems - counted);
  if (missing === 0) return { percent: 100, missing, localChanges };
  const have = own.globalBytes > 0 ? 100 * (1 - own.needBytes / own.globalBytes) : done.percent;
  return { percent: Math.min(have, 99), missing, localChanges };
}

/** Devices holding the folder; a blocked device keeps its shares but gets nothing. */
export function holders(overview: Overview, folderId: string): Device[] {
  const ids = new Set(
    overview.shares.filter((s) => s.folderId === folderId).map((s) => s.deviceId),
  );
  return overview.devices.filter((d) => d.approved && ids.has(d.id));
}

/** How many folders a device gets: none while it is blocked. */
export function deviceFolderCount(overview: Overview, device: Device): number {
  return device.approved ? overview.shares.filter((s) => s.deviceId === device.id).length : 0;
}

/** Errors in a folder anywhere: on the hub (`deviceId` null) or on a device. */
export function folderErrors(overview: Overview, live: Live | undefined, folderId: string) {
  const found: { deviceId: string | null; device: string; message: string }[] = [];
  const hub = live?.hub.folders[folderId];
  if (hub?.error) found.push({ deviceId: null, device: m.server(), message: hub.error });
  for (const device of holders(overview, folderId)) {
    const status = live?.devices[device.id]?.folders[folderId];
    if (!device.online || !status) continue;
    if (status.error)
      found.push({ deviceId: device.id, device: device.name, message: status.error });
    else if (status.errors > 0) {
      const count = status.errors;
      const message = byCount(count, m.error_files_failed_one, () =>
        m.error_files_failed_other({ count: formatCount(count) }),
      );
      found.push({ deviceId: device.id, device: device.name, message });
    }
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
    .map((d) => syncProgress(live, folderId, d.id)?.percent ?? 100)
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
    .reduce((sum, s) => sum + (syncProgress(live, s.folderId, deviceId)?.missing ?? 0), 0);
}

/** Lowest progress of a device over its folders, `undefined` when all are complete. */
export function deviceProgress(overview: Overview, live: Live | undefined, deviceId: string) {
  const percents = overview.shares
    .filter((s) => s.deviceId === deviceId)
    .map((s) => syncProgress(live, s.folderId, deviceId)?.percent ?? 100)
    .filter((p) => p < 100);
  return percents.length > 0 ? Math.floor(Math.min(...percents)) : undefined;
}

/** "1.903 lokale Änderungen", `undefined` without any. */
function localChangesText(count: number) {
  if (count === 0) return undefined;
  return byCount(count, m.local_changes_one, () =>
    m.local_changes_other({ count: formatCount(count) }),
  );
}

/** A status line with the local changes, which are no reason to call a device unfinished. */
const inSyncWith = (inSync: string, local: string | undefined) =>
  local ? `${inSync}, ${local}` : inSync;
const loadingWith = (loading: string, local: string | undefined) =>
  local ? `${loading} · ${local}` : loading;

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
  const local = localChangesText(
    overview.shares
      .filter((s) => s.deviceId === device.id)
      .reduce((sum, s) => sum + (syncProgress(live, s.folderId, device.id)?.localChanges ?? 0), 0),
  );
  // Below 100 % with nothing to fetch: only deletions are left, which is not "loading 0".
  if (percent === undefined || waiting === 0) return inSyncWith(m.device_in_sync(), local);
  return loadingWith(
    byCount(
      waiting,
      () => m.device_loading_one({ percent }),
      () => m.device_loading_other({ count, percent }),
    ),
    local,
  );
}

/** One device's progress in this folder, from the hub's view of it. */
export function shareStatus(
  device: Device,
  live: Live | undefined,
  folderId: string,
  revision: number,
) {
  const done = syncProgress(live, folderId, device.id);
  const missing = done?.missing ?? 0;
  const count = formatCount(missing);
  if (!device.online) {
    if (missing > 0) {
      return byCount(missing, m.share_offline_waiting_one, () =>
        m.share_offline_waiting_other({ count }),
      );
    }
    return isWaiting(device, revision) ? m.share_waiting_online() : m.share_offline_complete();
  }
  const error = live?.devices[device.id]?.folders[folderId]?.error;
  if (error) return m.share_error({ error });
  // As in deviceSummary: below 100 % with nothing to fetch means only deletions are left.
  const local = localChangesText(done?.localChanges ?? 0);
  if (done && done.percent < 100 && missing > 0) {
    const percent = Math.floor(done.percent);
    return loadingWith(
      byCount(
        missing,
        () => m.share_loading_one({ percent }),
        () => m.share_loading_other({ count, percent }),
      ),
      local,
    );
  }
  return inSyncWith(m.share_in_sync(), local);
}
