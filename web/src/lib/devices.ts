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
