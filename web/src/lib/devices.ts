import type { Device } from "../api/types";
import { osLabel } from "./format";

export type DeviceFilter = "all" | "online" | "offline";

/** The devices list after the online filter and the search, which matches name or system. */
export function filterDevices(
  devices: readonly Device[],
  filter: DeviceFilter,
  search: string,
): Device[] {
  const needle = search.trim().toLowerCase();
  return devices.filter(
    (d) =>
      (filter === "all" || d.online === (filter === "online")) &&
      `${d.name} ${osLabel(d.os)}`.toLowerCase().includes(needle),
  );
}

/** `x.y.z` as numbers; `undefined` for anything else, such as an agent that never connected. */
function versionParts(version: string): number[] | undefined {
  const parts = version.replace(/^v/, "").split(".");
  if (parts.length !== 3 || !parts.every((p) => /^\d+$/.test(p))) return undefined;
  return parts.map(Number);
}

/** Whether a device's agent is older than the Zentrale; versions that are not `x.y.z` never are. */
export function agentOutdated(device: Device, zentrale: string): boolean {
  const agent = versionParts(device.agentVersion);
  const wanted = versionParts(zentrale);
  if (!agent || !wanted) return false;
  const index = agent.findIndex((part, i) => part !== wanted[i]);
  return index >= 0 && (agent[index] ?? 0) < (wanted[index] ?? 0);
}
