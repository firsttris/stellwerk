import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Database, Folder as FolderIcon } from "lucide-react";
import { type FormEvent, useState } from "react";
import { api } from "../../../api";
import { overviewQuery, refresh, statusQuery } from "../../../api/queries";
import type { Device, Folder, FolderPatch, Live, Overview, ShareChange } from "../../../api/types";
import { ConflictCard } from "../../../components/conflict-card";
import { StatusDot } from "../../../components/device-icon";
import { Button } from "../../../components/ui/button";
import { Card } from "../../../components/ui/card";
import { Field, Input, Select } from "../../../components/ui/input";
import { VersionsList } from "../../../components/versions-list";
import { deviceStatus, nowSeconds, plural } from "../../../lib/format";
import { completion, folderSize, formatCount } from "../../../lib/live";
import { type CellMode, isWaiting, matrixDevices, storedMode } from "../../../lib/matrix";
import { cn } from "../../../lib/utils";

const TABS = ["devices", "versions", "conflicts", "settings"] as const;
type Tab = (typeof TABS)[number];
const TAB_LABEL: Record<Tab, string> = {
  devices: "Geräte",
  versions: "Versionen",
  conflicts: "Konflikte",
  settings: "Einstellungen",
};

export const Route = createFileRoute("/_app/folders/$folderId")({
  validateSearch: (search: Record<string, unknown>): { tab?: Tab } => ({
    tab: TABS.find((t) => t === search.tab),
  }),
  component: FolderPage,
});

const OPTIONS: { value: CellMode; label: string }[] = [
  { value: "sendReceive", label: "Senden & empfangen" },
  { value: "sendOnly", label: "Nur senden" },
  { value: "receiveOnly", label: "Nur empfangen" },
  { value: "off", label: "Aus" },
];

function FolderPage() {
  const { folderId } = Route.useParams();
  const { data: overview } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  const { tab = "devices" } = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const setTab = (next: Tab) => navigate({ search: { tab: next }, replace: true });
  const folder = overview?.folders.find((f) => f.id === folderId);

  if (!overview) return null;
  if (!folder) {
    return (
      <p className="text-muted-foreground">
        Diesen Ordner gibt es nicht mehr.{" "}
        <Link to="/folders" className="underline">
          Zu den Ordnern
        </Link>
      </p>
    );
  }
  const holders = overview.shares.filter((s) => s.folderId === folder.id).length;
  const conflicts = live?.conflicts[folder.id] ?? [];

  return (
    <>
      <nav aria-label="Pfad" className="flex items-center gap-2 text-[13px] text-muted-foreground">
        <Link to="/folders" className="hover:underline">
          Ordner
        </Link>
        <span>/</span>
        <span className="text-foreground">{folder.label}</span>
      </nav>
      <div className="flex items-center gap-3.5">
        <div className="grid size-11 place-items-center rounded-xl bg-muted">
          <FolderIcon className="size-5" aria-hidden="true" />
        </div>
        <div className="flex flex-col gap-0.5">
          <h1 className="text-2xl font-semibold tracking-tight">{folder.label}</h1>
          <span className="text-[13px] text-muted-foreground">
            auf {plural(holders, "Gerät", "Geräten")} und dem Server · Syncthing-ID{" "}
            <span className="font-mono">{folder.syncthingFolderId}</span>
          </span>
        </div>
      </div>
      <div role="tablist" aria-label="Ordnerbereiche" className="flex gap-1 border-b border-border">
        {TABS.map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            aria-selected={tab === t}
            onClick={() => setTab(t)}
            className={cn(
              "-mb-px h-10 cursor-pointer border-b-2 px-3 text-sm",
              tab === t ? "border-primary font-medium" : "border-transparent text-muted-foreground",
            )}
          >
            {TAB_LABEL[t]}
            {t === "conflicts" && conflicts.length > 0 && (
              <span className="ml-1.5 rounded-full bg-warn px-1.5 text-xs text-warn-foreground">
                {conflicts.length}
              </span>
            )}
          </button>
        ))}
      </div>
      {tab === "devices" && (
        <FolderDevices key={overview.revision} overview={overview} live={live} folder={folder} />
      )}
      {tab === "versions" && <VersionsList folderId={folder.id} />}
      {tab === "conflicts" &&
        (conflicts.length === 0 ? (
          <Card className="p-5 text-muted-foreground">
            Keine Konflikte. Wenn zwei Geräte dieselbe Datei ändern, bevor sie voneinander wissen,
            erscheint sie hier.
          </Card>
        ) : (
          conflicts.map((conflict) => (
            <ConflictCard
              key={conflict.path}
              folderId={folder.id}
              conflict={conflict}
              devices={overview.devices}
            />
          ))
        ))}
      {tab === "settings" && <FolderSettings folder={folder} />}
    </>
  );
}

type Row = { mode: CellMode; path: string };

function FolderDevices({
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
  const [rows, setRows] = useState<Record<string, Row>>(() =>
    Object.fromEntries(devices.map((d) => [d.id, stored(d.id)])),
  );
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
    onSuccess: () => refresh(queryClient),
  });
  const set = (deviceId: string, row: Partial<Row>) =>
    setRows((current) => ({
      ...current,
      [deviceId]: { ...stored(deviceId), ...current[deviceId], ...row },
    }));

  return (
    <Card aria-label="Geräte in diesem Ordner" className="overflow-x-auto">
      <div className="min-w-[760px]">
        <div className="grid grid-cols-[minmax(160px,1fr)_220px_minmax(240px,2fr)_minmax(160px,1fr)] gap-3 border-b border-border bg-subtle px-5 py-3 text-xs font-medium text-muted-foreground">
          <div>Gerät</div>
          <div>Modus</div>
          <div>Lokaler Pfad</div>
          <div>Status</div>
        </div>
        <div className="grid grid-cols-[minmax(160px,1fr)_220px_minmax(240px,2fr)_minmax(160px,1fr)] items-center gap-3 border-b border-muted px-5 py-3">
          <div className="flex flex-col">
            <span className="inline-flex items-center gap-1.5 font-mono font-medium">
              <Database className="size-3.5" aria-hidden="true" />
              Server
            </span>
            <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
              <StatusDot online /> Online
            </span>
          </div>
          <Select disabled aria-label="Modus für Server" value="hub" onChange={() => undefined}>
            <option value="hub">Speichert (Hub)</option>
          </Select>
          <Input
            disabled
            aria-label="Pfad auf Server"
            value={`folders/${folder.syncthingFolderId}`}
          />
          <span className="text-[13px] text-muted-foreground">
            Vollständige Kopie mit Versionen
            {folderSize(live, folder.id) ? ` · ${folderSize(live, folder.id)}` : ""}
          </span>
        </div>
        {devices.map((device) => {
          const row = rows[device.id] ?? stored(device.id);
          const off = row.mode === "off";
          return (
            <div
              key={device.id}
              className="grid grid-cols-[minmax(160px,1fr)_220px_minmax(240px,2fr)_minmax(160px,1fr)] items-center gap-3 border-b border-muted px-5 py-3"
            >
              <div className="flex min-w-0 flex-col">
                <span className="truncate font-mono font-medium">{device.name}</span>
                <span className="inline-flex items-center gap-1.5 text-xs text-muted-foreground">
                  <StatusDot online={device.online} /> {deviceStatus(device, now)}
                </span>
              </div>
              <Select
                aria-label={`Modus für ${device.name}`}
                value={row.mode}
                onChange={(e) => set(device.id, { mode: e.target.value as CellMode })}
              >
                {OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </Select>
              <Input
                aria-label={`Pfad auf ${device.name}`}
                value={off ? "" : row.path}
                placeholder={off ? "Nicht auf diesem Gerät" : `~/Stellwerk/${folder.label}`}
                disabled={off}
                onChange={(e) => set(device.id, { path: e.target.value })}
              />
              <span className="text-[13px] text-muted-foreground">
                {off
                  ? "Nicht auf diesem Gerät"
                  : shareStatus(device, live, folder.id, overview.revision)}
              </span>
            </div>
          );
        })}
        {devices.length === 0 && (
          <p className="px-5 py-6 text-muted-foreground">Noch keine Geräte gekoppelt.</p>
        )}
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3 bg-subtle px-5 py-3 text-xs text-muted-foreground">
        <span>
          Ein neuer Pfad richtet den Ordner auf dem Gerät neu ein; vorhandene Dateien am neuen Ort
          werden abgeglichen.
        </span>
        <div className="flex items-center gap-3">
          {save.error && <span className="text-danger">{save.error.message}</span>}
          <Button
            size="sm"
            disabled={changes.length === 0 || save.isPending}
            onClick={() => save.mutate()}
          >
            Speichern
          </Button>
        </div>
      </div>
    </Card>
  );
}

/** One device's progress in this folder, from the hub's view of it. */
function shareStatus(device: Device, live: Live | undefined, folderId: string, revision: number) {
  const done = completion(live, folderId, device.id);
  const missing = done?.needItems ?? 0;
  const changes = `${formatCount(missing)} ${missing === 1 ? "Änderung" : "Änderungen"}`;
  if (!device.online) {
    if (missing > 0) return `Offline · ${changes} warten auf dem Server`;
    return isWaiting(device, revision)
      ? "Wartet, bis das Gerät online ist"
      : "Offline · vollständig";
  }
  const error = live?.devices[device.id]?.folders[folderId]?.error;
  if (error) return `Fehler: ${error}`;
  if (done && done.percent < 100) return `Lädt ${changes} · ${Math.floor(done.percent)} %`;
  return "Synchron";
}

function FolderSettings({ folder }: { folder: Folder }) {
  const [label, setLabel] = useState(folder.label);
  const [ignores, setIgnores] = useState(folder.ignorePatterns.join("\n"));
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const update = useMutation({
    mutationFn: (patch: FolderPatch) => api.updateFolder(folder.id, patch),
    onSuccess: () => refresh(queryClient),
  });
  const remove = useMutation({
    mutationFn: () => api.deleteFolder(folder.id),
    onSuccess: async () => {
      await refresh(queryClient);
      await navigate({ to: "/folders" });
    },
  });
  const rename = (event: FormEvent) => {
    event.preventDefault();
    update.mutate({ label });
  };
  const ignoreLines = ignores
    .split("\n")
    .map((line) => line.trimEnd())
    .filter(Boolean);
  const ignoresChanged = ignoreLines.join("\n") !== folder.ignorePatterns.join("\n");
  const keep =
    folder.versioning.type === "staggered"
      ? `${folder.versioning.max_age_days} Tage, gestaffelt`
      : "keine";

  return (
    <div className="flex flex-col gap-4">
      <Card className="flex flex-col gap-4 p-5">
        <h2 className="text-base font-semibold">Name</h2>
        <form onSubmit={rename} className="flex flex-wrap gap-2">
          <Input
            aria-label="Name des Ordners"
            className="max-w-sm"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
          <Button
            type="submit"
            variant="outline"
            disabled={!label.trim() || label === folder.label}
          >
            Umbenennen
          </Button>
        </form>
        <p className="text-[13px] text-muted-foreground">Versionen auf dem Server: {keep}.</p>
      </Card>
      <Card className="flex flex-col gap-3 p-5">
        <h2 className="text-base font-semibold">Ignorieren</h2>
        <Field label="Ein Muster pro Zeile, gilt auf allen Geräten und dem Server.">
          {(id) => (
            <textarea
              id={id}
              rows={6}
              value={ignores}
              onChange={(e) => setIgnores(e.target.value)}
              placeholder={"node_modules/\n*.log"}
              className="w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-[13px]"
            />
          )}
        </Field>
        <div className="flex items-center justify-between gap-3">
          <a
            href="https://docs.syncthing.net/users/ignoring.html"
            target="_blank"
            rel="noreferrer"
            className="text-[13px] text-muted-foreground underline"
          >
            Syntax der Muster
          </a>
          <Button
            variant="outline"
            disabled={!ignoresChanged || update.isPending}
            onClick={() => update.mutate({ ignorePatterns: ignoreLines })}
          >
            Muster speichern
          </Button>
        </div>
        {update.error && <p className="text-sm text-danger">{update.error.message}</p>}
      </Card>
      <Card className="flex flex-wrap items-center justify-between gap-4 border-danger/30 p-5">
        <div className="flex flex-col gap-0.5">
          <span className="font-medium">Ordner entfernen</span>
          <span className="text-[13px] text-muted-foreground">
            Stoppt den Sync überall. Dateien auf den Geräten und dem Server bleiben erhalten.
          </span>
        </div>
        <Button
          variant="destructive"
          onClick={() => {
            if (
              window.confirm(`„${folder.label}“ überall entfernen? Die Dateien bleiben erhalten.`)
            ) {
              remove.mutate();
            }
          }}
        >
          Entfernen …
        </Button>
      </Card>
    </div>
  );
}
