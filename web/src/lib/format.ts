import type { Device } from "../api/types";

// Texts the UI builds from data. German for now; English comes with Paraglide.

const OS_LABEL: Record<string, string> = {
  linux: "Linux",
  windows: "Windows",
  macos: "macOS",
  android: "Android",
};

export const osLabel = (os: string) => OS_LABEL[os] ?? os;

/** "gerade eben", "vor 5 Min.", "vor 3 Std.", "vor 2 Tagen". */
export function ago(at: number, now: number): string {
  const seconds = Math.max(0, now - at);
  if (seconds < 60) return "gerade eben";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `vor ${minutes} Min.`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `vor ${hours} Std.`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "vor 1 Tag" : `vor ${days} Tagen`;
}

/** "3 Tage", "5 Std.", for "offline for …". */
export function duration(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${Math.max(1, minutes)} Min.`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} Std.`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "1 Tag" : `${days} Tage`;
}

/** "seit 3 Tagen", "seit 1 Tag", "seit 5 Std.": dative after "seit". */
export function since(seconds: number): string {
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `seit ${Math.max(1, minutes)} Min.`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `seit ${hours} Std.`;
  const days = Math.floor(hours / 24);
  return days === 1 ? "seit 1 Tag" : `seit ${days} Tagen`;
}

/** The short status under a device's name. */
export function deviceStatus(device: Device, now: number): string {
  if (!device.approved) return "Gesperrt";
  if (device.online) return `Online · ${osLabel(device.os)}`;
  if (device.lastSeen === null) return "Offline";
  return `Offline · ${duration(now - device.lastSeen)}`;
}

export const nowSeconds = () => Math.floor(Date.now() / 1000);

export const plural = (count: number, one: string, many: string) =>
  `${count} ${count === 1 ? one : many}`;

/** An activity entry in the UI's language; the stored text is the fallback. */
export function describeEvent(
  event: { kind: string; text: string; deviceId: string | null; folderId: string | null },
  devices: readonly Device[],
  folders: readonly { id: string; label: string }[],
): string {
  const name = devices.find((d) => d.id === event.deviceId)?.name ?? "Ein Gerät";
  const folder = folders.find((f) => f.id === event.folderId)?.label ?? "einem Ordner";
  switch (event.kind) {
    case "paired":
      return `${name} wurde gekoppelt`;
    case "online":
      return `${name} ist online`;
    case "offline":
      return `${name} ist offline`;
    case "changes": {
      const count = Number(event.text);
      return `${name} hat ${count === 1 ? "1 Datei" : `${count} Dateien`} in ${folder} geändert`;
    }
    case "error":
      return `Fehler auf ${name}: ${event.text}`;
    default:
      return event.text;
  }
}
