import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowDown, ArrowUp, ArrowUpDown, Check, Clock, Database } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { refresh } from "../api/queries";
import type { Overview } from "../api/types";
import { deviceStatus, nowSeconds, plural } from "../lib/format";
import {
  type CellMode,
  cellKey,
  cellMode,
  cycleCell,
  type Draft,
  draftChanges,
  isWaiting,
  MODE_LABEL,
  MODE_LONG,
  matrixDevices,
  offlineChanges,
} from "../lib/matrix";
import { cn } from "../lib/utils";
import { DeviceIcon, StatusDot } from "./device-icon";
import { Button } from "./ui/button";
import { Card } from "./ui/card";

const MODE_ICON = { sendReceive: ArrowUpDown, sendOnly: ArrowUp, receiveOnly: ArrowDown };

const CELL_STYLE: Record<CellMode, string> = {
  sendReceive: "border-primary bg-primary text-primary-foreground",
  sendOnly: "border-secondary bg-secondary text-secondary-foreground",
  receiveOnly: "border-border-strong bg-background text-foreground",
  off: "border-dashed border-border-strong bg-transparent text-muted-foreground",
};

function ModeIcon({ mode }: { mode: CellMode }) {
  if (mode === "off") return null;
  const Icon = MODE_ICON[mode];
  return <Icon className="size-3.5" strokeWidth={2.4} aria-hidden="true" />;
}

export function Legend() {
  return (
    <div className="flex flex-wrap gap-3 text-xs text-zinc-700 dark:text-zinc-300">
      {(["sendReceive", "sendOnly", "receiveOnly", "off"] as const).map((mode) => (
        <span key={mode} className="inline-flex items-center gap-1.5">
          <span
            className={cn(
              "grid size-[22px] place-items-center rounded-[5px] border",
              CELL_STYLE[mode],
            )}
          >
            <ModeIcon mode={mode} />
          </span>
          {mode === "sendOnly"
            ? "Nur senden"
            : mode === "receiveOnly"
              ? "Nur empfangen"
              : MODE_LABEL[mode]}
        </span>
      ))}
    </div>
  );
}

/** Folders × devices. Clicks collect in a draft; "Anwenden" sends it as one revision. */
export function Matrix({ overview }: { overview: Overview }) {
  const [draft, setDraft] = useState<Draft>(new Map());
  const [toast, setToast] = useState("");
  const queryClient = useQueryClient();
  const devices = matrixDevices(overview);
  const now = nowSeconds();
  const changes = draftChanges(overview, draft);
  const offline = offlineChanges(overview, draft);

  const apply = useMutation({
    mutationFn: () => api.applyShares(changes),
    onSuccess: async () => {
      setToast(
        offline > 0
          ? `Angewendet. ${changes.length - offline} sofort übertragen, ${offline} ${offline === 1 ? "wartet" : "warten"} auf Offline-Geräte.`
          : "Angewendet. Alle Geräte haben die neue Konfiguration.",
      );
      await refresh(queryClient);
      setDraft(new Map());
    },
  });

  return (
    <Card aria-labelledby="matrix-title" className="overflow-hidden">
      <div className="flex flex-wrap items-start justify-between gap-3 border-b border-border px-5 py-4">
        <div className="flex flex-col gap-1">
          <h2 id="matrix-title" className="text-base font-semibold">
            Freigaben
          </h2>
          <p className="text-[13px] text-muted-foreground">
            Klick auf eine Zelle wechselt den Modus. Offline-Geräte bekommen die Änderung, sobald
            sie wieder online sind.
          </p>
        </div>
        <Legend />
      </div>

      {changes.length > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-info-border bg-info px-5 py-3 text-info-foreground">
          <div className="flex items-center gap-2.5">
            <span className="size-2 rounded-full bg-info-accent" />
            <span>
              <strong className="font-semibold">
                {changes.length === 1
                  ? "1 ungespeicherte Änderung."
                  : `${changes.length} ungespeicherte Änderungen.`}
              </strong>{" "}
              {offline === 0
                ? "Wird sofort auf allen betroffenen Geräten angewendet."
                : offline === 1
                  ? "1 davon betrifft ein Offline-Gerät und wird nachgeholt."
                  : `${offline} davon betreffen Offline-Geräte und werden nachgeholt.`}
            </span>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => setDraft(new Map())}>
              Verwerfen
            </Button>
            <Button size="sm" onClick={() => apply.mutate()} disabled={apply.isPending}>
              Anwenden
            </Button>
          </div>
        </div>
      )}
      {apply.error && (
        <p className="border-b border-border bg-danger-surface px-5 py-3 text-danger">
          {apply.error.message}
        </p>
      )}
      {toast && changes.length === 0 && (
        <div className="flex items-center gap-2.5 border-b border-ok-border bg-ok-surface px-5 py-3 text-ok-foreground">
          <Check className="size-4" strokeWidth={2.4} aria-hidden="true" />
          <span>{toast}</span>
        </div>
      )}

      {overview.folders.length === 0 ? (
        <p className="px-5 py-10 text-center text-muted-foreground">
          Noch keine Ordner. Leg mit „Neuer Ordner“ den ersten an.
        </p>
      ) : (
        <div className="overflow-x-auto">
          <table
            className="w-full min-w-fit border-collapse text-left"
            aria-label="Freigabe-Matrix"
          >
            <colgroup>
              <col className="min-w-[220px]" />
              <col className="w-[136px] min-w-[136px]" />
              {devices.map((device) => (
                <col key={device.id} className="w-[136px] min-w-[136px]" />
              ))}
            </colgroup>
            <thead className="border-b border-border bg-subtle">
              <tr>
                <th
                  scope="col"
                  className="px-5 py-3 align-bottom text-xs font-medium text-muted-foreground"
                >
                  Ordner
                </th>
                <th scope="col" className="border-l border-muted px-2.5 py-3 font-normal">
                  <div className="flex items-center gap-1.5 font-medium">
                    <Database className="size-3.5" aria-hidden="true" />
                    <span className="font-mono text-[13px]">Server</span>
                  </div>
                  <div className="flex items-center gap-1.5 text-xs whitespace-nowrap text-muted-foreground">
                    <StatusDot online />
                    Hub · immer online
                  </div>
                </th>
                {devices.map((device) => (
                  <th
                    key={device.id}
                    scope="col"
                    className="border-l border-muted px-2.5 py-3 font-normal"
                  >
                    <div className="flex items-center gap-1.5 font-medium">
                      <DeviceIcon kind={device.kind} />
                      <span className="truncate font-mono text-[13px]">{device.name}</span>
                    </div>
                    <div className="flex items-center gap-1.5 text-xs whitespace-nowrap text-muted-foreground">
                      <StatusDot online={device.online} />
                      {deviceStatus(device, now)}
                    </div>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {overview.folders.map((folder) => {
                const holders = overview.shares.filter((s) => s.folderId === folder.id).length;
                return (
                  <tr key={folder.id} className="border-b border-muted hover:bg-subtle">
                    <th scope="row" className="px-5 py-3 font-normal">
                      <div className="flex flex-col gap-0.5">
                        <Link
                          to="/folders/$folderId"
                          params={{ folderId: folder.id }}
                          className="font-medium hover:underline"
                        >
                          {folder.label}
                        </Link>
                        <span className="text-xs text-muted-foreground">
                          auf {plural(holders, "Gerät", "Geräten")} und dem Server
                        </span>
                      </div>
                    </th>
                    <td className="border-l border-muted p-2">
                      <div
                        className="flex h-11 w-full items-center justify-center gap-1.5 rounded-md border border-border bg-muted text-[13px] font-medium"
                        title={`${folder.label} auf dem Server: immer gespeichert`}
                      >
                        <Database className="size-3.5" aria-hidden="true" />
                        Speichert
                      </div>
                    </td>
                    {devices.map((device) => {
                      const mode = cellMode(overview, draft, folder.id, device.id);
                      const changed = draft.has(cellKey(folder.id, device.id));
                      const waiting =
                        !changed && mode !== "off" && isWaiting(device, overview.revision);
                      const label = `${folder.label} auf ${device.name}: ${MODE_LONG[mode]}. Klicken zum Ändern.`;
                      return (
                        <td key={device.id} className="border-l border-muted p-2">
                          <button
                            type="button"
                            aria-label={label}
                            title={label}
                            onClick={() => {
                              setToast("");
                              setDraft(cycleCell(overview, draft, folder.id, device.id));
                            }}
                            className={cn(
                              "relative flex h-11 w-full cursor-pointer items-center justify-center gap-1.5 rounded-md border text-[13px] font-medium transition-transform hover:-translate-y-px",
                              CELL_STYLE[mode],
                              changed &&
                                "ring-2 ring-info-accent ring-offset-2 ring-offset-background",
                            )}
                          >
                            <ModeIcon mode={mode} />
                            <span>{MODE_LABEL[mode]}</span>
                            {waiting && (
                              <span className="absolute -top-2 -right-1.5 inline-flex items-center gap-0.5 rounded-full border border-info-border bg-background px-1.5 text-[10px] font-medium text-info-foreground dark:text-info-accent">
                                <Clock className="size-2.5" strokeWidth={2.6} aria-hidden="true" />
                                wartet
                              </span>
                            )}
                          </button>
                        </td>
                      );
                    })}
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
      <div className="flex items-center gap-2 bg-subtle px-5 py-3 text-xs text-muted-foreground">
        <Database className="size-3.5" aria-hidden="true" />
        Der Server hält von jedem Ordner eine vollständige Kopie mit Versionen. Darüber laufen
        Geräte, die nie gleichzeitig online sind.
      </div>
    </Card>
  );
}
