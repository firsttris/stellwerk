import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { Clock, TriangleAlert } from "lucide-react";
import { eventsQuery, overviewQuery, statusQuery } from "../../api/queries";
import type { Live, Overview as OverviewData } from "../../api/types";
import { PageHeader } from "../../components/layout";
import { Matrix } from "../../components/matrix";
import { NewFolderDialog } from "../../components/new-folder-dialog";
import { PairDialog } from "../../components/pair-dialog";
import { Button } from "../../components/ui/button";
import { Card } from "../../components/ui/card";
import { ago, describeEvent, nowSeconds, plural, since } from "../../lib/format";
import {
  folderBadge,
  folderErrors,
  formatBytes,
  formatCount,
  waitingChanges,
} from "../../lib/live";
import { isWaiting } from "../../lib/matrix";

export const Route = createFileRoute("/_app/")({ component: Overview });

function Overview() {
  const { data: overview, error } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  return (
    <>
      <PageHeader
        title="Übersicht"
        description="Wer synchronisiert was mit wem – zentral festgelegt, auf allen Geräten automatisch angewendet."
        actions={
          <>
            <PairDialog />
            <NewFolderDialog />
          </>
        }
      />
      {error && <p className="text-danger">{error.message}</p>}
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
        label="Geräte"
        value={
          <>
            {online}{" "}
            <span className="text-sm font-normal text-muted-foreground">
              von {devices.length} online
            </span>
          </>
        }
        note={
          longestOffline
            ? `${longestOffline.name} ${since(now - (longestOffline.lastSeen ?? now))} offline`
            : "Alle Geräte erreichbar"
        }
      />
      <Stat
        label="Ordner"
        value={
          <>
            {overview.folders.length}{" "}
            <span className="text-sm font-normal text-muted-foreground">verwaltet</span>
          </>
        }
        note={
          live
            ? `${count("ok") + count("neutral")} synchron · ${count("info")} läuft · ${count("warn")} mit Fehler`
            : "Status wird geladen …"
        }
      />
      <Stat
        label="Auf dem Server"
        value={formatBytes(bytes)}
        note={`${formatCount(files)} Dateien, mit Versionen`}
      />
      <Stat
        label="Konfiguration"
        value={`Revision ${overview.revision}`}
        note={
          waiting === 0
            ? "Alle Geräte sind auf dem neuesten Stand"
            : `${plural(waiting, "Gerät wartet", "Geräte warten")} auf die neue Konfiguration`
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
        Braucht Aufmerksamkeit
      </h2>
      {conflicts.map(({ folder, conflict }) => (
        <div key={`${folder.id}-${conflict.path}`} className="flex items-start gap-3">
          <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-warn text-warn-foreground">
            <TriangleAlert className="size-4" aria-hidden="true" />
          </div>
          <div className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="font-medium">Konflikt in {folder.label}</span>
            <span className="text-[13px] text-muted-foreground">
              <span className="font-mono">{conflict.original}</span> wurde auf zwei Geräten
              gleichzeitig geändert.
            </span>
          </div>
          <Button variant="outline" size="sm" asChild>
            <Link
              to="/folders/$folderId"
              params={{ folderId: folder.id }}
              search={{ tab: "conflicts" }}
            >
              Lösen
            </Link>
          </Button>
        </div>
      ))}
      {offline.length === 0 && errors.length === 0 && conflicts.length === 0 && (
        <p className="text-muted-foreground">
          Nichts zu tun. Alle Geräte sind online und synchron.
        </p>
      )}
      {errors.map(({ folder, device, message }) => (
        <div key={`${folder.id}-${device}`} className="flex items-start gap-3">
          <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-warn text-warn-foreground">
            <TriangleAlert className="size-4" aria-hidden="true" />
          </div>
          <div className="flex min-w-0 flex-col gap-0.5">
            <span className="font-medium">
              Fehler in {folder.label} auf {device}
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
                {device.name}{" "}
                {device.lastSeen !== null
                  ? `${since(now - device.lastSeen)} offline`
                  : "noch nie verbunden"}
              </span>
              <span className="text-[13px] text-muted-foreground">
                {waiting > 0
                  ? `${formatCount(waiting)} ${waiting === 1 ? "Änderung liegt" : "Änderungen liegen"} auf dem Server bereit und ${waiting === 1 ? "wird" : "werden"} übertragen, sobald das Gerät wieder da ist. Nichts zu tun.`
                  : "Alles liegt auf dem Server bereit. Nichts zu tun."}
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
        Aktivität
      </h2>
      {events?.length === 0 && <p className="text-muted-foreground">Noch nichts passiert.</p>}
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
