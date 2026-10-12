import type { Device } from "../api/types";
import { m } from "../paraglide/messages.js";
import { byCount } from "./i18n";

// Texts the UI builds from data, in the current language.

const OS_LABEL: Record<string, string> = {
  linux: "Linux",
  windows: "Windows",
  macos: "macOS",
  android: "Android",
};

export const osLabel = (os: string) => OS_LABEL[os] ?? os;

/** Splits seconds into the unit a person would use. */
function unit(seconds: number): { unit: "minutes" | "hours" | "days"; count: number } {
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return { unit: "minutes", count: Math.max(1, minutes) };
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return { unit: "hours", count: hours };
  return { unit: "days", count: Math.floor(hours / 24) };
}

/** "gerade eben", "vor 5 Min.", "vor 3 Std.", "vor 2 Tagen". */
export function ago(at: number, now: number): string {
  const seconds = Math.max(0, now - at);
  if (seconds < 60) return m.time_ago_just_now();
  const { unit: u, count } = unit(seconds);
  if (u === "minutes") return m.time_ago_minutes({ count });
  if (u === "hours") return m.time_ago_hours({ count });
  return byCount(count, m.time_ago_day, () => m.time_ago_days({ count }));
}

/** "3 Tage", "5 Std.", for "offline · …". */
export function duration(seconds: number): string {
  const { unit: u, count } = unit(seconds);
  if (u === "minutes") return m.duration_minutes({ count });
  if (u === "hours") return m.duration_hours({ count });
  return byCount(count, m.duration_day, () => m.duration_days({ count }));
}

/** "seit 3 Tagen", "seit 1 Tag", "seit 5 Std.". */
export function since(seconds: number): string {
  const { unit: u, count } = unit(seconds);
  if (u === "minutes") return m.since_minutes({ count });
  if (u === "hours") return m.since_hours({ count });
  return byCount(count, m.since_day, () => m.since_days({ count }));
}

/** The short status under a device's name. */
export function deviceStatus(device: Device, now: number): string {
  if (!device.approved) return m.device_blocked();
  if (device.online) return m.device_online_os({ os: osLabel(device.os) });
  if (device.lastSeen === null) return m.device_offline();
  return m.device_offline_for({ duration: duration(now - device.lastSeen) });
}

export const nowSeconds = () => Math.floor(Date.now() / 1000);

/** An activity entry in the UI's language; the stored text is the fallback. */
export function describeEvent(
  event: { kind: string; text: string; deviceId: string | null; folderId: string | null },
  devices: readonly Device[],
  folders: readonly { id: string; label: string }[],
): string {
  const device = devices.find((d) => d.id === event.deviceId)?.name ?? m.event_someone();
  const folder = folders.find((f) => f.id === event.folderId)?.label ?? m.event_some_folder();
  switch (event.kind) {
    case "paired":
      return m.event_paired({ device });
    case "online":
      return m.event_online({ device });
    case "offline":
      return m.event_offline({ device });
    case "changes": {
      const count = Number(event.text);
      return byCount(
        count,
        () => m.event_changes_one({ device, folder }),
        () => m.event_changes_other({ device, folder, count }),
      );
    }
    case "error":
      return m.event_error({ device, text: event.text });
    case "conflict":
      return m.event_conflict({ folder, text: event.text });
    case "resolved":
      return m.event_resolved({ folder });
    case "restored":
      return m.event_restored({ folder, path: event.text });
    default:
      return event.text;
  }
}

/**
 * `dir/config.ts` + `workbench` → `config.workbench.ts`, as "keep both" names the copy. Mirrors
 * `free_name` and `split_ext` in crates/engine/src/files.rs (without the numbered fallback the
 * engine adds when that name is taken).
 */
export function bothName(original: string, device: string): string {
  const file = original.split("/").pop() ?? original;
  const dot = file.lastIndexOf(".");
  const label = device.replace(/[^\p{L}\p{N}_-]/gu, "-");
  return dot > 0 ? `${file.slice(0, dot)}.${label}${file.slice(dot)}` : `${file}.${label}`;
}
