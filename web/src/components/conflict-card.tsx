import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { TriangleAlert } from "lucide-react";
import { useMemo } from "react";
import { api } from "../api";
import { eventsQuery, statusQuery } from "../api/queries";
import type { Conflict, Device, FileContent, Keep } from "../api/types";
import { conflictLines, type DiffLine } from "../lib/diff";
import { errorMessage } from "../lib/errors";
import { ago, nowSeconds } from "../lib/format";
import { formatBytes } from "../lib/live";
import { cn } from "../lib/utils";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";
import { Card } from "./ui/card";

/** `dir/config.ts` + `workbench` → `config.workbench.ts`, as "keep both" names the copy. */
export function bothName(original: string, device: string): string {
  const file = original.split("/").pop() ?? original;
  const dot = file.lastIndexOf(".");
  const label = device.replace(/[^\p{L}\p{N}_-]/gu, "-");
  return dot > 0 ? `${file.slice(0, dot)}.${label}${file.slice(dot)}` : `${file}.${label}`;
}

function Side({
  title,
  file,
  lines,
  keepLabel,
  onKeep,
  disabled,
}: {
  title: string;
  file: FileContent | null | undefined;
  lines: DiffLine[] | undefined;
  keepLabel: string;
  onKeep: () => void;
  disabled: boolean;
}) {
  const now = nowSeconds();
  return (
    <div className="flex min-w-0 flex-col gap-2 rounded-lg border border-border p-3">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <span className="font-mono text-[13px] font-medium">{title}</span>
        <span className="text-xs text-muted-foreground">
          {file ? `${ago(file.modified, now)} · ${formatBytes(file.size)}` : m.conflict_deleted()}
        </span>
      </div>
      {!file ? null : lines ? (
        <pre
          // A scrollable region must be focusable to scroll it with the keyboard (WCAG 2.1.1).
          // biome-ignore lint/a11y/noNoninteractiveTabindex: see above
          tabIndex={0}
          className="max-h-72 overflow-auto rounded-md bg-subtle p-2 font-mono text-xs leading-5"
        >
          {lines.map((line, index) => (
            <span
              // Lines can repeat; their position is their identity here.
              // biome-ignore lint/suspicious/noArrayIndexKey: see above
              key={index}
              className={cn(
                "block whitespace-pre-wrap",
                line.kind === "removed" && "bg-danger-surface text-danger",
                line.kind === "added" && "bg-ok-surface text-ok-foreground",
              )}
            >
              {line.kind === "removed" ? "- " : line.kind === "added" ? "+ " : "  "}
              {line.text}
            </span>
          ))}
        </pre>
      ) : (
        <p className="rounded-md bg-subtle p-3 text-[13px] text-muted-foreground">
          {m.conflict_no_preview()}
        </p>
      )}
      <Button variant="outline" size="sm" onClick={onKeep} disabled={disabled || !file}>
        {keepLabel}
      </Button>
    </div>
  );
}

/** One conflict: both versions side by side, with a line diff for text. */
export function ConflictCard({
  folderId,
  conflict,
  devices,
}: {
  folderId: string;
  conflict: Conflict;
  devices: readonly Device[];
}) {
  const queryClient = useQueryClient();
  const content = useQuery({
    queryKey: ["conflict", folderId, conflict.path],
    queryFn: () => api.conflictContent(folderId, conflict.path),
  });
  const resolve = useMutation({
    mutationFn: (keep: Keep) => api.resolveConflict(folderId, conflict.path, keep),
    onSuccess: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: statusQuery.queryKey }),
        queryClient.invalidateQueries({ queryKey: eventsQuery.queryKey }),
      ]),
  });
  const device = devices.find((d) => d.id === conflict.deviceId)?.name ?? m.conflict_other_device();
  const original = content.data?.original;
  const copy = content.data?.copy;
  // The diff is the expensive part of the page; every live update re-renders the route.
  const lines = useMemo(() => conflictLines(original, copy), [original, copy]);

  return (
    <Card
      aria-label={m.conflict_label({ path: conflict.original })}
      className="flex flex-col gap-4 p-5"
    >
      <div className="flex items-start gap-3">
        <div className="grid size-9 shrink-0 place-items-center rounded-lg bg-warn text-warn-foreground">
          <TriangleAlert className="size-4" aria-hidden="true" />
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="font-mono font-medium break-all">{conflict.original}</span>
          <span className="text-[13px] text-muted-foreground">{m.conflict_explained()}</span>
        </div>
      </div>
      {content.error && <p className="text-sm text-danger">{errorMessage(content.error)}</p>}
      <div className="grid grid-cols-[repeat(auto-fit,minmax(min(320px,100%),1fr))] gap-3">
        <Side
          title={m.conflict_current()}
          file={original}
          lines={lines?.filter((l) => l.kind !== "added")}
          keepLabel={m.conflict_keep_this()}
          onKeep={() => resolve.mutate("original")}
          disabled={resolve.isPending}
        />
        <Side
          title={m.conflict_from_device({ device })}
          file={copy}
          lines={lines?.filter((l) => l.kind !== "removed")}
          keepLabel={m.conflict_keep_this()}
          onKeep={() => resolve.mutate("copy")}
          disabled={resolve.isPending}
        />
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
        <span className="text-[13px] text-muted-foreground">
          {m.conflict_both_before()}{" "}
          <span className="font-mono">{bothName(conflict.original, device)}</span>
          {m.conflict_both_after()}
        </span>
        <Button onClick={() => resolve.mutate("both")} disabled={resolve.isPending}>
          {m.conflict_keep_both()}
        </Button>
      </div>
      {resolve.error && <p className="text-sm text-danger">{errorMessage(resolve.error)}</p>}
    </Card>
  );
}
