import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Clock, Radar, TriangleAlert } from "lucide-react";
import { eventsQuery, overviewQuery, statusQuery } from "../../api/queries";
import type { Live, Overview as OverviewData } from "../../api/types";
import { PageHeader } from "../../components/layout";
import { Matrix } from "../../components/matrix";
import { NewFolderDialog } from "../../components/new-folder-dialog";
import { PairDialog } from "../../components/pair-dialog";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { errorMessage } from "../../lib/errors";
import { ago, describeEvent, nowSeconds, since } from "../../lib/format";
import { byCount } from "../../lib/i18n";
import {
  folderBadge,
  folderErrors,
  formatBytes,
  formatCount,
  waitingChanges,
} from "../../lib/live";
import { isWaiting } from "../../lib/matrix";
import { m } from "../../paraglide/messages.js";

export const Route = createFileRoute("/_app/")({ component: Overview });

function Overview() {
  const { data: overview, error } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  return (
    <>
      <PageHeader
        title={m.overview_title()}
        description={m.overview_description()}
        actions={
          <>
            <PairDialog />
            <NewFolderDialog />
          </>
        }
      />
      {error && <p className="text-danger">{errorMessage(error)}</p>}
      {overview && (
        <>
          <Stats overview={overview} live={live} />
          <Matrix overview={overview} live={live} />
          <div className="grid grid-cols-[repeat(auto-fit,minmax(min(360px,100%),1fr))] gap-4">
            <Attention overview={overview} live={live} />
            <Activity overview={overview} />
          </div>
        </>
      )}
    </>
  );
}

function Stat({ label, value, note }: { label: string; value: React.ReactNode; note: string }) {
  return (
    <div className="flex flex-col gap-1.5 rounded-xl border border-border p-4">
      <span className="text-[13px] text-muted-foreground">{label}</span>
      <span className="text-2xl font-semibold tracking-tight tabular-nums">{value}</span>
      <span className="text-xs text-muted-foreground">{note}</span>
    </div>
  );
}

function Stats({ overview, live }: { overview: OverviewData; live: Live | undefined }) {
  const now = nowSeconds();
  const devices = overview.devices.filter((d) => d.approved);
  const online = devices.filter((d) => d.online).length;
  const longestOffline = devices
    .filter((d) => !d.online && d.lastSeen !== null)
    .sort((a, b) => (a.lastSeen ?? 0) - (b.lastSeen ?? 0))[0];
  const badges = overview.folders.map((f) => folderBadge(overview, live, f.id));
  const count = (tone: string) => badges.filter((b) => b.tone === tone).length;
  const onHub = Object.values(live?.hub.folders ?? {});
  const bytes = onHub.reduce((sum, f) => sum + f.globalBytes, 0);
  const files = onHub.reduce((sum, f) => sum + f.globalFiles, 0);
  const waiting = devices.filter((d) => isWaiting(d, overview.revision)).length;
  return (
    <div className="grid grid-cols-[repeat(auto-fit,minmax(200px,1fr))] gap-3">
      <Stat
        label={m.stat_devices()}
        value={
          <>
            {online}{" "}
            <span className="text-sm font-normal text-muted-foreground">
              {m.stat_devices_online({ count: devices.length })}
            </span>
          </>
        }
        note={
          longestOffline
            ? m.stat_offline_since({
                device: longestOffline.name,
                since: since(now - (longestOffline.lastSeen ?? now)),
              })
            : m.stat_all_reachable()
        }
      />
      <Stat
        label={m.stat_folders()}
        value={
          <>
            {overview.folders.length}{" "}
            <span className="text-sm font-normal text-muted-foreground">
              {m.stat_folders_managed()}
            </span>
          </>
        }
        note={
          live
            ? m.stat_folders_note({
                ok: count("ok") + count("neutral"),
                running: count("info"),
                errors: count("warn"),
              })
            : m.stat_loading()
        }
      />
      <Stat
        label={m.stat_server()}
        value={formatBytes(bytes)}
        note={m.stat_server_files({ count: formatCount(files) })}
      />
      <Stat
        label={m.stat_configuration()}
        value={m.stat_revision({ revision: overview.revision })}
        note={
          waiting === 0
            ? m.stat_all_current()
            : byCount(waiting, m.stat_waiting_one, () => m.stat_waiting_other({ count: waiting }))
        }
      />
    </div>
  );
}

function Attention({ overview, live }: { overview: OverviewData; live: Live | undefined }) {
  const now = nowSeconds();
  const offline = overview.devices.filter((d) => d.approved && !d.online);
  const conflicts = overview.folders.flatMap((folder) =>
    (live?.conflicts[folder.id] ?? []).map((conflict) => ({ folder, conflict })),
  );
  const errors = overview.folders.flatMap((folder) =>
    folderErrors(overview, live, folder.id).map((error) => ({ folder, ...error })),
  );
  return (
    <Card aria-labelledby="attention-title" className="flex flex-col gap-3.5 px-5 py-4">
      <h2 id="attention-title" className="text-base font-semibold">
        {m.attention_title()}
      </h2>
      {conflicts.map(({ folder, conflict }) => (
        <div key={`${folder.id}-${conflict.path}`} className="flex items-start gap-3">
          <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-warn text-warn-foreground">
            <TriangleAlert className="size-4" aria-hidden="true" />
          </div>
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="font-medium">{m.attention_conflict({ folder: folder.label })}</span>
            <span className="text-[13px] text-muted-foreground">
              <span className="font-mono">{conflict.original}</span>
              {m.attention_conflict_text()}
            </span>
          </div>
          <Button variant="outline" size="sm" asChild>
            <Link
              to="/folders/$folderId"
              params={{ folderId: folder.id }}
              search={{ tab: "conflicts" }}
            >
              {m.attention_resolve()}
            </Link>
          </Button>
        </div>
      ))}
      {(live?.waiting ?? []).map((device) => (
        <div key={device.fingerprint} className="flex items-start gap-3">
          <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-muted">
            <Radar className="size-4" aria-hidden="true" />
          </div>
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="font-medium">{m.attention_waiting({ device: device.name })}</span>
            <span className="text-[13px] text-muted-foreground">
              {m.attention_waiting_text({ fingerprint: device.fingerprint })}
            </span>
          </div>
          <Button variant="outline" size="sm" asChild>
            <Link to="/devices">{m.attention_review()}</Link>
          </Button>
        </div>
      ))}
      {offline.length === 0 &&
        errors.length === 0 &&
        conflicts.length === 0 &&
        (live?.waiting ?? []).length === 0 && (
          <p className="text-muted-foreground">{m.attention_nothing()}</p>
        )}
      {errors.map(({ folder, device, message }) => (
        <div key={`${folder.id}-${device}`} className="flex items-start gap-3">
          <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-warn text-warn-foreground">
            <TriangleAlert className="size-4" aria-hidden="true" />
          </div>
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="font-medium">
              {m.attention_error({ folder: folder.label, device })}
            </span>
            <span className="text-[13px] text-muted-foreground">{message}</span>
          </div>
        </div>
      ))}
      {offline.map((device) => {
        const waiting = waitingChanges(overview, live, device.id);
        return (
          <div key={device.id} className="flex items-start gap-3">
            <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-muted">
              <Clock className="size-4" aria-hidden="true" />
            </div>
            <div className="flex min-w-0 flex-col gap-0.5">
              <span className="font-medium">
                {device.lastSeen !== null
                  ? m.attention_offline({
                      device: device.name,
                      since: since(now - device.lastSeen),
                    })
                  : m.attention_never_connected({ device: device.name })}
              </span>
              <span className="text-[13px] text-muted-foreground">
                {waiting > 0
                  ? byCount(waiting, m.attention_waiting_one, () =>
                      m.attention_waiting_other({ count: formatCount(waiting) }),
                    )
                  : m.attention_all_ready()}
              </span>
            </div>
          </div>
        );
      })}
    </Card>
  );
}

function Activity({ overview }: { overview: OverviewData }) {
  const { data: events } = useQuery(eventsQuery);
  const now = nowSeconds();
  return (
    <Card aria-labelledby="activity-title" className="flex flex-col gap-3 px-5 py-4">
      <h2 id="activity-title" className="text-base font-semibold">
        {m.activity_title()}
      </h2>
      {events?.length === 0 && <p className="text-muted-foreground">{m.activity_nothing()}</p>}
      {events?.map((event) => (
        <div key={event.id} className="grid grid-cols-[88px_minmax(0,1fr)] gap-2.5 text-[13px]">
          <span className="font-mono text-xs text-muted-foreground tabular-nums">
            {ago(event.at, now)}
          </span>
          <span>{describeEvent(event, overview.devices, overview.folders)}</span>
        </div>
      ))}
    </Card>
  );
}
