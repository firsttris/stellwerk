import { useQuery } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { Search } from "lucide-react";
import { useState } from "react";
import { overviewQuery, statusQuery } from "../../api/queries";
import { DeviceTable } from "../../components/device-table";
import { PageHeader } from "../../components/layout";
import { PairDialog } from "../../components/pair-dialog";
import { WaitingDevices } from "../../components/waiting-devices";
import { type DeviceFilter, filterDevices } from "../../lib/devices";
import { cn } from "../../lib/utils";
import { m } from "../../paraglide/messages.js";

export const Route = createFileRoute("/_app/devices")({ component: Devices });

function Devices() {
  const { data: overview } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<DeviceFilter>("all");
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
        <DeviceTable
          overview={overview}
          live={live}
          devices={filterDevices(overview.devices, filter, search)}
        />
      )}
    </>
  );
}
