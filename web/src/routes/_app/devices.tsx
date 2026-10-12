import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import {
  type ColumnDef,
  flexRender,
  getCoreRowModel,
  getFilteredRowModel,
  useReactTable,
} from "@tanstack/react-table";
import { Database, Ellipsis, Search } from "lucide-react";
import { DropdownMenu } from "radix-ui";
import { useMemo, useState } from "react";
import { api } from "../../api";
import { overviewQuery, refresh, statusQuery } from "../../api/queries";
import type { Device, Live, Overview } from "../../api/types";
import { DeviceIcon, StatusDot } from "../../components/device-icon";
import { PageHeader } from "../../components/layout";
import { PairDialog } from "../../components/pair-dialog";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ago, nowSeconds, osLabel } from "../../lib/format";
import { deviceSummary, waitingChanges } from "../../lib/live";
import { isWaiting } from "../../lib/matrix";
import { cn } from "../../lib/utils";

export const Route = createFileRoute("/_app/devices")({ component: Devices });

type Filter = "all" | "online" | "offline";

function Devices() {
  const { data: overview } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const devices = useMemo(
    () =>
      (overview?.devices ?? []).filter((d) =>
        filter === "all" ? true : filter === "online" ? d.online : !d.online,
      ),
    [overview, filter],
  );
  const counts = {
    all: overview?.devices.length ?? 0,
    online: overview?.devices.filter((d) => d.online).length ?? 0,
    offline: overview?.devices.filter((d) => !d.online).length ?? 0,
  };

  return (
    <>
      <PageHeader
        title="Geräte"
        description="Jedes Gerät führt nur einen kleinen Agent aus. Alles andere stellst du hier ein."
        actions={<PairDialog variant="default" />}
      />
      <div className="flex flex-wrap items-center gap-3">
        <label className="relative flex min-w-[240px] flex-1 items-center">
          <span className="sr-only">Geräte durchsuchen</span>
          <Search
            className="pointer-events-none absolute left-3 size-4 text-muted-foreground"
            aria-hidden="true"
          />
          <input
            type="search"
            placeholder="Geräte durchsuchen"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-10 w-full rounded-md border border-border bg-background pr-3 pl-9"
          />
        </label>
        <fieldset className="flex gap-1 rounded-lg bg-muted p-1">
          <legend className="sr-only">Filter</legend>
          {(["all", "online", "offline"] as const).map((f) => (
            <button
              key={f}
              type="button"
              aria-pressed={filter === f}
              onClick={() => setFilter(f)}
              className={cn(
                "h-8 cursor-pointer rounded-md px-3 text-sm",
                filter === f
                  ? "bg-background font-medium shadow-sm"
                  : "text-zinc-700 dark:text-zinc-300",
              )}
            >
              {{ all: "Alle", online: "Online", offline: "Offline" }[f]} {counts[f]}
            </button>
          ))}
        </fieldset>
      </div>
      {overview && (
        <DeviceTable overview={overview} live={live} devices={devices} search={search} />
      )}
    </>
  );
}

function folderCount(overview: Overview, device: Device) {
  return overview.shares.filter((s) => s.deviceId === device.id).length;
}

function DeviceTable({
  overview,
  live,
  devices,
  search,
}: {
  overview: Overview;
  live: Live | undefined;
  devices: Device[];
  search: string;
}) {
  const now = nowSeconds();
  const columns = useMemo<ColumnDef<Device>[]>(
    () => [
      {
        id: "device",
        header: "Gerät",
        accessorFn: (d) => `${d.name} ${osLabel(d.os)}`,
        cell: ({ row: { original: d } }) => (
          <div className="flex items-center gap-3">
            <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-muted">
              <DeviceIcon kind={d.kind} className="size-[18px]" />
            </div>
            <div className="flex min-w-0 flex-col">
              <span className="truncate font-mono font-medium">{d.name}</span>
              <span className="text-xs text-muted-foreground">{osLabel(d.os)}</span>
            </div>
          </div>
        ),
      },
      {
        id: "status",
        header: "Status",
        cell: ({ row: { original: d } }) => (
          <div className="flex flex-col gap-0.5">
            <span className="inline-flex items-center gap-2 font-medium">
              <StatusDot online={d.online} />
              {!d.approved ? "Gesperrt" : d.online ? "Online" : "Offline"}
            </span>
            <span className="text-xs text-muted-foreground">
              {isWaiting(d, overview.revision) && waitingChanges(overview, live, d.id) === 0
                ? "Neue Konfiguration wartet"
                : deviceSummary(overview, live, d)}
            </span>
          </div>
        ),
      },
      {
        id: "folders",
        header: "Ordner",
        cell: ({ row: { original: d } }) => folderCount(overview, d),
      },
      {
        id: "seen",
        header: "Zuletzt gesehen",
        cell: ({ row: { original: d } }) =>
          d.online ? "gerade eben" : d.lastSeen === null ? "nie" : ago(d.lastSeen, now),
      },
      {
        id: "agent",
        header: "Agent",
        cell: ({ row: { original: d } }) => (
          <span className="rounded bg-muted px-1.5 py-0.5 font-mono text-xs">{d.agentVersion}</span>
        ),
      },
      {
        id: "actions",
        header: "",
        cell: ({ row: { original: d } }) => <DeviceActions device={d} />,
      },
    ],
    [overview, live, now],
  );
  const table = useReactTable({
    data: devices,
    columns,
    state: { globalFilter: search },
    getCoreRowModel: getCoreRowModel(),
    getFilteredRowModel: getFilteredRowModel(),
  });

  return (
    <Card aria-label="Geräteliste" className="overflow-x-auto">
      <table className="w-full min-w-[760px] text-left">
        <thead className="border-b border-border bg-subtle text-xs text-muted-foreground">
          {table.getHeaderGroups().map((group) => (
            <tr key={group.id}>
              {group.headers.map((header) => (
                <th key={header.id} className="px-4 py-3 font-medium">
                  {flexRender(header.column.columnDef.header, header.getContext())}
                </th>
              ))}
            </tr>
          ))}
        </thead>
        <tbody>
          <tr className="border-b border-muted">
            <td className="px-4 py-3">
              <div className="flex items-center gap-3">
                <div className="grid size-9 place-items-center rounded-lg bg-muted">
                  <Database className="size-[18px]" aria-hidden="true" />
                </div>
                <div className="flex flex-col">
                  <span className="font-mono font-medium">Server</span>
                  <span className="text-xs text-muted-foreground">{overview.hub.address}</span>
                </div>
              </div>
            </td>
            <td className="px-4 py-3">
              <span className="inline-flex items-center gap-2 font-medium">
                <StatusDot online />
                Online
              </span>
              <div className="text-xs text-muted-foreground">Hub, hält alle Ordner</div>
            </td>
            <td className="px-4 py-3">{overview.folders.length}</td>
            <td className="px-4 py-3">gerade eben</td>
            <td className="px-4 py-3" />
            <td className="px-4 py-3" />
          </tr>
          {table.getRowModel().rows.map((row) => (
            <tr key={row.id} className="border-b border-muted hover:bg-subtle">
              {row.getVisibleCells().map((cell) => (
                <td key={cell.id} className="px-4 py-3">
                  {flexRender(cell.column.columnDef.cell, cell.getContext())}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

function DeviceActions({ device }: { device: Device }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    mutationFn: (patch: Parameters<typeof api.updateDevice>[1]) =>
      api.updateDevice(device.id, patch),
    onSuccess: () => refresh(queryClient),
  });
  const remove = useMutation({
    mutationFn: () => api.deleteDevice(device.id),
    onSuccess: () => refresh(queryClient),
  });

  const rename = () => {
    const name = window.prompt("Neuer Name", device.name)?.trim();
    if (name && name !== device.name) update.mutate({ name });
  };
  const confirmRemove = () => {
    if (
      window.confirm(
        `${device.name} entfernen? Das Gerät verliert alle Ordner; seine Dateien bleiben erhalten.`,
      )
    ) {
      remove.mutate();
    }
  };

  const item =
    "flex h-9 cursor-pointer items-center rounded px-2.5 text-sm outline-none data-[highlighted]:bg-muted";
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="ghost" size="icon" aria-label={`Aktionen für ${device.name}`}>
          <Ellipsis aria-hidden="true" />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          className="z-50 min-w-44 rounded-md border border-border bg-background p-1 shadow-lg"
        >
          <DropdownMenu.Item className={item} onSelect={rename}>
            Umbenennen
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className={item}
            onSelect={() => update.mutate({ approved: !device.approved })}
          >
            {device.approved ? "Sperren" : "Entsperren"}
          </DropdownMenu.Item>
          <DropdownMenu.Item className={cn(item, "text-danger")} onSelect={confirmRemove}>
            Entfernen …
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
