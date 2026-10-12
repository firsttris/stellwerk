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
import { WaitingDevices } from "../../components/waiting-devices";
import { ago, nowSeconds, osLabel } from "../../lib/format";
import { deviceFolderCount, deviceSummary, waitingChanges } from "../../lib/live";
import { isWaiting } from "../../lib/matrix";
import { cn } from "../../lib/utils";
import { m } from "../../paraglide/messages.js";

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
        title={m.devices_title()}
        description={m.devices_description()}
        actions={<PairDialog variant="default" />}
      />
      <div className="flex flex-wrap items-center gap-3">
        <label className="relative flex min-w-[240px] flex-1 items-center">
          <span className="sr-only">{m.devices_search()}</span>
          <Search
            className="pointer-events-none absolute left-3 size-4 text-muted-foreground"
            aria-hidden="true"
          />
          <input
            type="search"
            placeholder={m.devices_search()}
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="h-10 w-full rounded-md border border-border bg-background pr-3 pl-9"
          />
        </label>
        <fieldset className="flex gap-1 rounded-lg bg-muted p-1">
          <legend className="sr-only">{m.devices_filter()}</legend>
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
              {{
                all: m.devices_filter_all,
                online: m.devices_filter_online,
                offline: m.devices_filter_offline,
              }[f]({ count: counts[f] })}
            </button>
          ))}
        </fieldset>
      </div>
      <WaitingDevices waiting={live?.waiting ?? []} />
      {overview && (
        <DeviceTable overview={overview} live={live} devices={devices} search={search} />
      )}
    </>
  );
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
        header: () => m.devices_column_device(),
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
        header: () => m.devices_column_status(),
        cell: ({ row: { original: d } }) => (
          <div className="flex flex-col gap-0.5">
            <span className="inline-flex items-center gap-2 font-medium">
              <StatusDot online={d.online} />
              {!d.approved
                ? m.device_blocked()
                : d.online
                  ? m.devices_online()
                  : m.device_offline()}
            </span>
            <span className="text-xs text-muted-foreground">
              {isWaiting(d, overview.revision) && waitingChanges(overview, live, d.id) === 0
                ? m.devices_config_waiting()
                : deviceSummary(overview, live, d)}
            </span>
          </div>
        ),
      },
      {
        id: "folders",
        header: () => m.devices_column_folders(),
        cell: ({ row: { original: d } }) => deviceFolderCount(overview, d),
      },
      {
        id: "seen",
        header: () => m.devices_column_seen(),
        cell: ({ row: { original: d } }) =>
          d.online
            ? m.time_ago_just_now()
            : d.lastSeen === null
              ? m.devices_never()
              : ago(d.lastSeen, now),
      },
      {
        id: "agent",
        header: () => m.devices_column_agent(),
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
    <Card aria-label={m.devices_list()} className="overflow-x-auto">
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
                  <span className="font-mono font-medium">{m.server()}</span>
                  <span className="text-xs text-muted-foreground">{overview.hub.address}</span>
                </div>
              </div>
            </td>
            <td className="px-4 py-3">
              <span className="inline-flex items-center gap-2 font-medium">
                <StatusDot online />
                {m.devices_online()}
              </span>
              <div className="text-xs text-muted-foreground">{m.devices_hub_holds_all()}</div>
            </td>
            <td className="px-4 py-3">{overview.folders.length}</td>
            <td className="px-4 py-3">{m.time_ago_just_now()}</td>
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
    const name = window.prompt(m.devices_new_name(), device.name)?.trim();
    if (name && name !== device.name) update.mutate({ name });
  };
  const confirmRemove = () => {
    if (window.confirm(m.devices_remove_confirm({ device: device.name }))) {
      remove.mutate();
    }
  };

  const item =
    "flex h-9 cursor-pointer items-center rounded px-2.5 text-sm outline-none data-[highlighted]:bg-muted";
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="ghost" size="icon" aria-label={m.devices_actions({ device: device.name })}>
          <Ellipsis aria-hidden="true" />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          className="z-50 min-w-44 rounded-md border border-border bg-background p-1 shadow-lg"
        >
          <DropdownMenu.Item className={item} onSelect={rename}>
            {m.devices_rename()}
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className={item}
            onSelect={() => update.mutate({ approved: !device.approved })}
          >
            {device.approved ? m.devices_block() : m.devices_unblock()}
          </DropdownMenu.Item>
          <DropdownMenu.Item className={cn(item, "text-danger")} onSelect={confirmRemove}>
            {m.remove()}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
