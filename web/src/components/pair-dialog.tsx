import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleCheck, Copy, Link2 } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api } from "../api";
import { overviewQuery, refresh } from "../api/queries";
import type { Device, Enrollment } from "../api/types";
import { copyToClipboard } from "../lib/clipboard";
import { errorMessage } from "../lib/errors";
import { osLabel } from "../lib/format";
import { byCount } from "../lib/i18n";
import { type Os, pairCommand } from "../lib/pairing";
import { cn } from "../lib/utils";
import { m } from "../paraglide/messages.js";
import { PathField } from "./path-field";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogTrigger } from "./ui/dialog";
import { Field, Input } from "./ui/input";

const OS_TABS: readonly Os[] = ["linux", "windows", "macos"];

/** Where the manual installation is described, for a device the script does not cover. */
const MANUAL_INSTALL = "https://firsttris.github.io/stellwerk/installation.html#agent-on-a-device";

export function PairDialog({ variant = "outline" }: { variant?: "outline" | "default" }) {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant={variant}>
          <Link2 aria-hidden="true" />
          {m.pair_device()}
        </Button>
      </DialogTrigger>
      {open && <PairDialogContent onDone={() => setOpen(false)} />}
    </Dialog>
  );
}

function PairDialogContent({ onDone }: { onDone: () => void }) {
  // A query, not an effect: one code per opened dialog, also under StrictMode; dropped on close.
  const enrollment = useQuery({
    queryKey: ["new-enrollment"],
    queryFn: () => api.createEnrollment(),
    staleTime: Number.POSITIVE_INFINITY,
    gcTime: 0,
    retry: false,
  });
  const code = enrollment.data?.code;
  const expired = useExpired(enrollment.data?.expiresAt);

  const status = useQuery({
    queryKey: ["enrollment", code],
    queryFn: () => api.enrollmentStatus(code ?? ""),
    enabled: code !== undefined && !expired,
    refetchInterval: (query) => (query.state.data?.device ? false : 2_000),
  });
  const device = status.data?.device;

  return (
    <DialogContent title={m.pair_device()} description={m.pair_description()}>
      {device ? (
        <NewDevice device={device} onDone={onDone} />
      ) : (
        <Waiting
          enrollment={enrollment.data}
          expired={expired}
          onNewCode={() => void enrollment.refetch()}
          error={enrollment.error ? errorMessage(enrollment.error) : undefined}
          onCancel={onDone}
        />
      )}
    </DialogContent>
  );
}

/** True once `expiresAt` (seconds) has passed; the server keeps answering for a dead code. */
function useExpired(expiresAt: number | undefined): boolean {
  const [expired, setExpired] = useState(false);
  useEffect(() => {
    setExpired(false);
    if (expiresAt === undefined) return;
    const timer = setTimeout(() => setExpired(true), Math.max(0, expiresAt * 1000 - Date.now()));
    return () => clearTimeout(timer);
  }, [expiresAt]);
  return expired;
}

function Waiting({
  enrollment,
  expired,
  onNewCode,
  error,
  onCancel,
}: {
  enrollment: Enrollment | undefined;
  expired: boolean;
  onNewCode: () => void;
  error: string | undefined;
  onCancel: () => void;
}) {
  const [os, setOs] = useState<Os>("linux");
  const [copied, setCopied] = useState<"copied" | "selected" | undefined>();
  const commandRef = useRef<HTMLElement>(null);
  const command = enrollment
    ? pairCommand(os, window.location.origin, enrollment)
    : m.pair_code_pending();

  const copy = async () => {
    // No clipboard API on plain HTTP (a LAN address): select the command for a manual copy.
    const done = await copyToClipboard(command);
    if (!done && commandRef.current) window.getSelection()?.selectAllChildren(commandRef.current);
    setCopied(done ? "copied" : "selected");
    setTimeout(() => setCopied(undefined), 1800);
  };

  return (
    <>
      <div className="flex flex-col gap-4">
        <div role="tablist" aria-label={m.pair_os()} className="flex gap-1 rounded-lg bg-muted p-1">
          {OS_TABS.map((tab) => (
            <button
              key={tab}
              type="button"
              role="tab"
              aria-selected={tab === os}
              onClick={() => setOs(tab)}
              className={cn(
                "h-9 flex-1 cursor-pointer rounded-md text-sm",
                tab === os
                  ? "bg-background font-medium shadow-sm"
                  : "text-zinc-700 dark:text-zinc-300",
              )}
            >
              {osLabel(tab)}
            </button>
          ))}
        </div>
        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium">{m.pair_step_install()}</span>
          <div className="flex items-stretch gap-2 rounded-lg border border-border bg-subtle p-2 pl-3">
            <code
              className="min-w-0 flex-1 self-center font-mono text-[13px] break-all"
              data-testid="pair-command"
              ref={commandRef}
            >
              {command}
            </code>
            <Button
              variant="outline"
              size="sm"
              onClick={copy}
              disabled={!enrollment || expired}
              aria-label={m.pair_copy_command()}
            >
              <Copy aria-hidden="true" />
              {copied === "copied"
                ? m.pair_copied()
                : copied === "selected"
                  ? m.pair_selected()
                  : m.pair_copy()}
            </Button>
          </div>
          {os === "windows" && (
            <span className="text-xs text-muted-foreground">{m.pair_windows_shell()}</span>
          )}
          <span className="text-xs text-muted-foreground">{m.pair_code_valid()}</span>
          <span className="text-xs text-muted-foreground">{m.pair_lan_hint()}</span>
          <a
            href={MANUAL_INSTALL}
            target="_blank"
            rel="noreferrer"
            className="text-xs text-muted-foreground underline"
          >
            {m.pair_manual_install()}
          </a>
          {error && <span className="text-sm text-danger">{error}</span>}
        </div>
        {expired ? (
          <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-dashed border-border-strong p-3">
            <div className="flex flex-col">
              <span className="font-medium">{m.pair_expired()}</span>
              <span className="text-[13px] text-muted-foreground">{m.pair_expired_hint()}</span>
            </div>
            <Button variant="outline" size="sm" onClick={onNewCode}>
              {m.pair_new_code()}
            </Button>
          </div>
        ) : (
          <div className="flex items-center gap-3 rounded-lg border border-dashed border-border-strong p-3">
            <span className="size-2.5 animate-pulse rounded-full bg-info-accent" />
            <div className="flex flex-col">
              <span className="font-medium">{m.pair_step_wait()}</span>
              <span className="text-[13px] text-muted-foreground">{m.pair_wait_hint()}</span>
            </div>
          </div>
        )}
      </div>
      <div className="flex justify-end">
        <Button variant="outline" onClick={onCancel}>
          {m.cancel()}
        </Button>
      </div>
    </>
  );
}

function NewDevice({ device, onDone }: { device: Device; onDone: () => void }) {
  const { data: overview } = useQuery(overviewQuery);
  const queryClient = useQueryClient();
  const [name, setName] = useState(device.name);
  // Folder ID → its path here; empty for the Zentrale's default.
  const [picked, setPicked] = useState<ReadonlyMap<string, string>>(new Map());

  const save = useMutation({
    mutationFn: async () => {
      if (name.trim() && name.trim() !== device.name) {
        await api.updateDevice(device.id, { name: name.trim() });
      }
      if (picked.size > 0) {
        await api.applyShares(
          [...picked].map(([folderId, path]) => ({
            folderId,
            deviceId: device.id,
            mode: "sendReceive",
            path: path.trim() || undefined,
          })),
        );
      }
    },
    onSuccess: async () => {
      await refresh(queryClient);
      onDone();
    },
  });

  const toggle = (folderId: string) => {
    const next = new Map(picked);
    if (next.has(folderId)) next.delete(folderId);
    else next.set(folderId, "");
    setPicked(next);
  };
  const setPath = (folderId: string, path: string) =>
    setPicked((current) => new Map(current).set(folderId, path));

  return (
    <>
      <div className="flex flex-col gap-4">
        <div className="flex items-center gap-3 rounded-lg border border-ok-border bg-ok-surface p-3 text-ok-foreground">
          <CircleCheck className="size-5 shrink-0" aria-hidden="true" />
          <div className="flex flex-col">
            <span className="font-medium">{m.pair_connected()}</span>
            <span className="text-[13px]">
              {m.pair_details({ os: osLabel(device.os), version: device.agentVersion })}{" "}
              <span className="font-mono">{device.fingerprint}</span>
            </span>
          </div>
        </div>
        <Field label={m.name()}>
          {(id) => <Input id={id} value={name} onChange={(e) => setName(e.target.value)} />}
        </Field>
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 text-sm font-medium">{m.pair_which_folders()}</legend>
          {overview?.folders.map((folder) => (
            <div
              key={folder.id}
              className="flex flex-col gap-2 rounded-md border border-border px-3"
            >
              <label className="flex min-h-11 cursor-pointer items-center gap-3">
                <input
                  type="checkbox"
                  className="size-4 accent-primary"
                  checked={picked.has(folder.id)}
                  onChange={() => toggle(folder.id)}
                />
                <span className="flex-1">{folder.label}</span>
              </label>
              {picked.has(folder.id) && (
                <div className="pb-3">
                  <PathField
                    device={device}
                    label={m.share_path_label({
                      folder: folder.label,
                      device: name.trim() || device.name,
                    })}
                    value={picked.get(folder.id) ?? ""}
                    placeholder={`~/Stellwerk/${folder.label}`}
                    onChange={(path) => setPath(folder.id, path)}
                  />
                </div>
              )}
            </div>
          ))}
          <span className="text-xs text-muted-foreground">{m.pair_default_mode()}</span>
        </fieldset>
        {save.error && <p className="text-sm text-danger">{errorMessage(save.error)}</p>}
      </div>
      <div className="flex justify-end gap-2">
        <Button variant="outline" onClick={onDone}>
          {m.pair_later()}
        </Button>
        <Button onClick={() => save.mutate()} disabled={save.isPending}>
          {picked.size === 0
            ? m.pair_done()
            : byCount(picked.size, m.pair_setup_one, () =>
                m.pair_setup_other({ count: picked.size }),
              )}
        </Button>
      </div>
    </>
  );
}
