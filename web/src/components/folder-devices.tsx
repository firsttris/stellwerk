import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Database } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { refresh } from "../api/queries";
import type { Device, Folder, Live, Overview, ShareChange } from "../api/types";
import { errorMessage } from "../lib/errors";
import { deviceStatus, nowSeconds } from "../lib/format";
import { folderSize, shareStatus } from "../lib/live";
import { type CellMode, matrixDevices, storedMode } from "../lib/matrix";
import { cn } from "../lib/utils";
import { m } from "../paraglide/messages.js";
import { StatusDot } from "./device-icon";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { Input, Select } from "./ui/input";

const options = (): { value: CellMode; label: string }[] => [
  { value: "sendReceive", label: m.folder_option_both() },
  { value: "sendOnly", label: m.mode_legend_send() },
  { value: "receiveOnly", label: m.mode_legend_receive() },
  { value: "off", label: m.mode_off() },
];

type Row = { mode: CellMode; path: string };

/** Device, mode, path, status: the same columns for the header and every row. */
const GRID =
  "grid grid-cols-[minmax(160px,1fr)_220px_minmax(240px,2fr)_minmax(160px,1fr)] gap-3 border-b px-5 py-3";

/** Mode and path of a folder per device. */
export function FolderDevices({
  overview,
  live,
  folder,
}: {
  overview: Overview;
  live: Live | undefined;
  folder: Folder;
}) {
  const devices = matrixDevices(overview);
  const now = nowSeconds();
  const stored = (deviceId: string): Row => ({
    mode: storedMode(overview, folder.id, deviceId),
    path:
      overview.shares.find((s) => s.folderId === folder.id && s.deviceId === deviceId)?.path ?? "",
  });
  // Only the rows edited here; the others follow the stored state, also when another tab or
  // the matrix changes it meanwhile.
  const [rows, setRows] = useState<Record<string, Row>>({});
  const queryClient = useQueryClient();

  const changes: ShareChange[] = devices.flatMap((d) => {
    const row = rows[d.id];
    const before = stored(d.id);
    if (!row || (row.mode === before.mode && (row.mode === "off" || row.path === before.path))) {
      return [];
    }
    return [
      {
        folderId: folder.id,
        deviceId: d.id,
        mode: row.mode === "off" ? null : row.mode,
        path: row.mode === "off" ? undefined : row.path.trim() || undefined,
      },
    ];
  });
  const save = useMutation({
    mutationFn: () => api.applyShares(changes),
    onSuccess: async () => {
      await refresh(queryClient);
      setRows({});
    },
  });
  const set = (deviceId: string, row: Partial<Row>) =>
    setRows((current) => ({
      ...current,
      [deviceId]: { ...stored(deviceId), ...current[deviceId], ...row },
    }));

  return (
    <Card aria-label={m.folder_devices_label()} className="overflow-x-auto">
      <div className="min-w-[760px]">
        <div
          className={cn(GRID, "border-border bg-subtle text-xs font-medium text-muted-foreground")}
        >
          <div>{m.folder_column_device()}</div>
          <div>{m.folder_column_mode()}</div>
          <div>{m.folder_column_path()}</div>
          <div>{m.folder_column_status()}</div>
        </div>
        <HubRow folder={folder} live={live} />
        {devices.map((device) => (
          <DeviceRow
            key={device.id}
            device={device}
            row={rows[device.id] ?? stored(device.id)}
            onChange={(row) => set(device.id, row)}
            status={shareStatus(device, live, folder.id, overview.revision)}
            placeholder={`~/Stellwerk/${folder.label}`}
            now={now}
          />
        ))}
        {devices.length === 0 && (
          <p className="px-5 py-6 text-muted-foreground">{m.folder_no_devices()}</p>
        )}
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3 bg-subtle px-5 py-3 text-xs text-muted-foreground">
        <span>{m.folder_new_path_hint()}</span>
        <div className="flex items-center gap-3">
          {save.error && <span className="text-danger">{errorMessage(save.error)}</span>}
          <Button
            size="sm"
            disabled={changes.length === 0 || save.isPending}
            onClick={() => save.mutate()}
          >
            {m.save()}
          </Button>
        </div>
      </div>
    </Card>
  );
}

function HubRow({ folder, live }: { folder: Folder; live: Live | undefined }) {
  const size = folderSize(live, folder.id);
  return (
    <div className={cn(GRID, "items-center border-muted")}>
      <div className="flex flex-col">
        <span className="inline-flex items-center gap-1.5 font-mono font-medium">
          <Database className="size-3.5" aria-hidden="true" />
          {m.server()}
        </span>
        <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
          <StatusDot online /> {m.devices_online()}
        </span>
      </div>
      <Select
        disabled
        aria-label={m.folder_mode_for({ device: m.server() })}
        value="hub"
        onChange={() => undefined}
      >
        <option value="hub">{m.folder_hub_mode()}</option>
      </Select>
      <Input
        disabled
        aria-label={m.folder_path_for({ device: m.server() })}
        value={`folders/${folder.syncthingFolderId}`}
      />
      <span className="text-[13px] text-muted-foreground">
        {m.folder_hub_copy()}
        {size ? ` · ${size}` : ""}
      </span>
    </div>
  );
}

function DeviceRow({
  device,
  row,
  onChange,
  status,
  placeholder,
  now,
}: {
  device: Device;
  row: Row;
  onChange: (row: Partial<Row>) => void;
  status: string;
  placeholder: string;
  now: number;
}) {
  const off = row.mode === "off";
  return (
    <div className={cn(GRID, "items-center border-muted")}>
      <div className="flex min-w-0 flex-col">
        <span className="truncate font-mono font-medium">{device.name}</span>
        <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
          <StatusDot online={device.online} /> {deviceStatus(device, now)}
        </span>
      </div>
      <Select
        aria-label={m.folder_mode_for({ device: device.name })}
        value={row.mode}
        onChange={(e) => onChange({ mode: e.target.value as CellMode })}
      >
        {options().map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </Select>
      <Input
        aria-label={m.folder_path_for({ device: device.name })}
        value={off ? "" : row.path}
        placeholder={off ? m.folder_not_here() : placeholder}
        disabled={off}
        onChange={(e) => onChange({ path: e.target.value })}
      />
      <span className="text-[13px] text-muted-foreground">
        {off ? m.folder_not_here() : status}
      </span>
    </div>
  );
}
