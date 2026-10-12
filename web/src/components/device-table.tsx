import { Database } from "lucide-react";
import type { ReactNode } from "react";
import type { Device, Live, Overview } from "../api/types";
import { ago, nowSeconds, osLabel } from "../lib/format";
import { deviceFolderCount, deviceSummary, waitingChanges } from "../lib/live";
import { isWaiting } from "../lib/matrix";
import { m } from "../paraglide/messages.js";
import { DeviceActions } from "./device-actions";
import { DeviceIcon, StatusDot } from "./device-icon";
import { Card } from "./ui/card";

const cell = "px-4 py-3";

/** The hub and the devices, one row each. */
export function DeviceTable({
  overview,
  live,
  devices,
}: {
  overview: Overview;
  live: Live | undefined;
  devices: readonly Device[];
}) {
  const now = nowSeconds();
  const headers = [
    m.devices_column_device(),
    m.devices_column_status(),
    m.devices_column_folders(),
    m.devices_column_seen(),
    m.devices_column_agent(),
    "",
  ];
  return (
    <Card aria-label={m.devices_list()} className="overflow-x-auto">
      <table className="w-full min-w-[760px] text-left">
        <thead className="border-b border-border bg-subtle text-xs text-muted-foreground">
          <tr>
            {headers.map((header, index) => (
              // biome-ignore lint/suspicious/noArrayIndexKey: fixed columns; the last has no text
              <th key={index} scope="col" className={`${cell} font-medium`}>
                {header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          <HubRow overview={overview} />
          {devices.map((device) => (
            <DeviceRow key={device.id} overview={overview} live={live} device={device} now={now} />
          ))}
        </tbody>
      </table>
    </Card>
  );
}

function Name({ icon, name, detail }: { icon: ReactNode; name: string; detail: string }) {
  return (
    <div className="flex items-center gap-3">
      <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-muted">{icon}</div>
      <div className="flex min-w-0 flex-col">
        <span className="truncate font-mono font-medium">{name}</span>
        <span className="text-xs text-muted-foreground">{detail}</span>
      </div>
    </div>
  );
}

function HubRow({ overview }: { overview: Overview }) {
  return (
    <tr className="border-b border-muted">
      <td className={cell}>
        <Name
          icon={<Database className="size-[18px]" aria-hidden="true" />}
          name={m.server()}
          detail={overview.hub.address}
        />
      </td>
      <td className={cell}>
        <span className="inline-flex items-center gap-2 font-medium">
          <StatusDot online />
          {m.devices_online()}
        </span>
        <div className="text-xs text-muted-foreground">{m.devices_hub_holds_all()}</div>
      </td>
      <td className={cell}>{overview.folders.length}</td>
      <td className={cell}>{m.time_ago_just_now()}</td>
      <td className={cell} />
      <td className={cell} />
    </tr>
  );
}

function DeviceRow({
  overview,
  live,
  device: d,
  now,
}: {
  overview: Overview;
  live: Live | undefined;
  device: Device;
  now: number;
}) {
  return (
    <tr className="border-b border-muted hover:bg-subtle">
      <td className={cell}>
        <Name
          icon={<DeviceIcon kind={d.kind} className="size-[18px]" />}
          name={d.name}
          detail={osLabel(d.os)}
        />
      </td>
      <td className={cell}>
        <div className="flex flex-col gap-0.5">
          <span className="inline-flex items-center gap-2 font-medium">
            <StatusDot online={d.online} />
            {!d.approved ? m.device_blocked() : d.online ? m.devices_online() : m.device_offline()}
          </span>
          <span className="text-xs text-muted-foreground">
            {isWaiting(d, overview.revision) && waitingChanges(overview, live, d.id) === 0
              ? m.devices_config_waiting()
              : deviceSummary(overview, live, d)}
          </span>
        </div>
      </td>
      <td className={cell}>{deviceFolderCount(overview, d)}</td>
      <td className={cell}>
        {d.online
          ? m.time_ago_just_now()
          : d.lastSeen === null
            ? m.devices_never()
            : ago(d.lastSeen, now)}
      </td>
      <td className={cell}>
        <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">{d.agentVersion}</span>
      </td>
      <td className={cell}>
        <DeviceActions device={d} />
      </td>
    </tr>
  );
}
