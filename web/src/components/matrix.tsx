import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Link } from "@tanstack/react-router";
import { ArrowDown, ArrowUp, ArrowUpDown, Check, Clock, Database } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { refresh } from "../api/queries";
import type { Live, Overview } from "../api/types";
import { errorMessage } from "../lib/errors";
import { deviceStatus, nowSeconds } from "../lib/format";
import { byCount } from "../lib/i18n";
import { completion, folderBadge, folderSize, holders } from "../lib/live";
import {
  type CellMode,
  cellKey,
  cellMode,
  cycleCell,
  type Draft,
  draftChanges,
  isWaiting,
  matrixDevices,
  modeLabel,
  modeLong,
  offlineChanges,
} from "../lib/matrix";
import { cn } from "../lib/utils";
import { m } from "../paraglide/messages.js";
import { DeviceIcon, StatusDot } from "./device-icon";
import { Button } from "./ui/button";
import { Badge, Card } from "./ui/card";

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
            ? m.mode_legend_send()
            : mode === "receiveOnly"
              ? m.mode_legend_receive()
              : modeLabel(mode)}
        </span>
      ))}
    </div>
  );
}

/** Folders × devices. Clicks collect in a draft; "Anwenden" sends it as one revision. */
export function Matrix({ overview, live }: { overview: Overview; live: Live | undefined }) {
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
          ? byCount(
              offline,
              () => m.matrix_applied_waiting_one({ now: changes.length - offline }),
              () =>
                m.matrix_applied_waiting_other({ now: changes.length - offline, waiting: offline }),
            )
          : m.matrix_applied_all(),
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
            {m.matrix_title()}
          </h2>
          <p className="text-[13px] text-muted-foreground">{m.matrix_hint()}</p>
        </div>
        <Legend />
      </div>

      {changes.length > 0 && (
        <div className="flex flex-wrap items-center justify-between gap-3 border-b border-info-border bg-info px-5 py-3 text-info-foreground">
          <div className="flex items-center gap-2.5">
            <span className="size-2 rounded-full bg-info-accent" />
            <span>
              <strong className="font-semibold">
                {byCount(changes.length, m.matrix_unsaved_one, () =>
                  m.matrix_unsaved_other({ count: changes.length }),
                )}
              </strong>{" "}
              {offline === 0
                ? m.matrix_applies_now()
                : byCount(offline, m.matrix_offline_one, () =>
                    m.matrix_offline_other({ count: offline }),
                  )}
            </span>
          </div>
          <div className="flex gap-2">
            <Button variant="outline" size="sm" onClick={() => setDraft(new Map())}>
              {m.matrix_discard()}
            </Button>
            <Button size="sm" onClick={() => apply.mutate()} disabled={apply.isPending}>
              {m.matrix_apply()}
            </Button>
          </div>
        </div>
      )}
      {apply.error && (
        <p className="border-b border-border bg-danger-surface px-5 py-3 text-danger">
          {errorMessage(apply.error)}
        </p>
      )}
      {toast && changes.length === 0 && (
        <div className="flex items-center gap-2.5 border-b border-ok-border bg-ok-surface px-5 py-3 text-ok-foreground">
          <Check className="size-4" strokeWidth={2.4} aria-hidden="true" />
          <span>{toast}</span>
        </div>
      )}

      {overview.folders.length === 0 ? (
        <p className="px-5 py-10 text-center text-muted-foreground">{m.matrix_no_folders()}</p>
      ) : (
        <div className="overflow-x-auto">
          <table
            className="w-full min-w-fit border-collapse text-left"
            aria-label={m.matrix_label()}
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
                  {m.matrix_folder_column()}
                </th>
                <th scope="col" className="border-l border-muted px-2.5 py-3 font-normal">
                  <div className="flex items-center gap-1.5 font-medium">
                    <Database className="size-3.5" aria-hidden="true" />
                    <span className="font-mono text-[13px]">{m.server()}</span>
                  </div>
                  <div className="flex items-center gap-1.5 text-xs whitespace-nowrap text-muted-foreground">
                    <StatusDot online />
                    {m.matrix_hub_status()}
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
                const holderCount = holders(overview, folder.id).length;
                const badge = folderBadge(overview, live, folder.id);
                return (
                  <tr key={folder.id} className="border-b border-muted hover:bg-subtle">
                    <th scope="row" className="px-5 py-3 font-normal">
                      <div className="flex flex-col gap-0.5">
                        <div className="flex flex-wrap items-center gap-2">
                          <Link
                            to="/folders/$folderId"
                            params={{ folderId: folder.id }}
                            className="font-medium hover:underline"
                          >
                            {folder.label}
                          </Link>
                          <Badge tone={badge.tone}>{badge.text}</Badge>
                        </div>
                        <span className="text-xs text-muted-foreground">
                          {folderSize(live, folder.id) ??
                            byCount(holderCount, m.matrix_on_devices_one, () =>
                              m.matrix_on_devices_other({ count: holderCount }),
                            )}
                        </span>
                      </div>
                    </th>
                    <td className="border-l border-muted p-2">
                      <div
                        className="flex h-11 w-full items-center justify-center gap-1.5 rounded-md border border-border bg-muted text-[13px] font-medium"
                        title={m.matrix_hub_cell_title({ folder: folder.label })}
                      >
                        <Database className="size-3.5" aria-hidden="true" />
                        {m.matrix_hub_cell()}
                      </div>
                    </td>
                    {devices.map((device) => {
                      const mode = cellMode(overview, draft, folder.id, device.id);
                      const changed = draft.has(cellKey(folder.id, device.id));
                      const waiting =
                        !changed && mode !== "off" && isWaiting(device, overview.revision);
                      const percent = completion(live, folder.id, device.id)?.percent ?? 100;
                      const progress =
                        !changed && !waiting && mode !== "off" && device.online && percent < 100
                          ? Math.floor(percent)
                          : undefined;
                      const label = m.matrix_cell_label({
                        folder: folder.label,
                        device: device.name,
                        mode: modeLong(mode),
                      });
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
                            <span>{modeLabel(mode)}</span>
                            {progress !== undefined && (
                              <span className="absolute -top-2 -right-1.5 rounded-full border border-info-border bg-background px-1.5 text-[10px] font-medium text-info-foreground tabular-nums dark:text-info-accent">
                                {progress} %
                              </span>
                            )}
                            {waiting && (
                              <span className="absolute -top-2 -right-1.5 inline-flex items-center gap-0.5 rounded-full border border-info-border bg-background px-1.5 text-[10px] font-medium text-info-foreground dark:text-info-accent">
                                <Clock className="size-2.5" strokeWidth={2.6} aria-hidden="true" />
                                {m.matrix_waiting()}
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
        {m.matrix_footer()}
      </div>
    </Card>
  );
}
