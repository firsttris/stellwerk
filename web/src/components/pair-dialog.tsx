import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleCheck, Copy, Link2 } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { overviewQuery, refresh } from "../api/queries";
import type { Device } from "../api/types";
import { osLabel } from "../lib/format";
import { byCount } from "../lib/i18n";
import { cn } from "../lib/utils";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogTrigger } from "./ui/dialog";
import { Field, Input } from "./ui/input";

type Os = "linux" | "windows" | "macos";
const OS_TABS: { id: Os; label: string }[] = [
  { id: "linux", label: "Linux" },
  { id: "windows", label: "Windows" },
  { id: "macos", label: "macOS" },
];

/** The command shown for a code; the agent binary comes from the release page for now. */
export function pairCommand(os: Os, origin: string, code: string): string {
  const agent = os === "windows" ? "stellwerk-agent.exe" : "stellwerk-agent";
  const join = `${agent} join --server ${origin} ${code}`;
  return os === "windows" ? `${join}; ${agent} run` : `${join} && ${agent} run`;
}

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

  const status = useQuery({
    queryKey: ["enrollment", code],
    queryFn: () => api.enrollmentStatus(code ?? ""),
    enabled: code !== undefined,
    refetchInterval: (query) => (query.state.data?.device ? false : 2_000),
  });
  const device = status.data?.device;

  return (
    <DialogContent title={m.pair_device()} description={m.pair_description()}>
      {device ? (
        <NewDevice device={device} onDone={onDone} />
      ) : (
        <Waiting code={code} error={enrollment.error?.message} onCancel={onDone} />
      )}
    </DialogContent>
  );
}

function Waiting({
  code,
  error,
  onCancel,
}: {
  code: string | undefined;
  error: string | undefined;
  onCancel: () => void;
}) {
  const [os, setOs] = useState<Os>("linux");
  const [copied, setCopied] = useState(false);
  const command = code ? pairCommand(os, window.location.origin, code) : m.pair_code_pending();

  const copy = async () => {
    await navigator.clipboard?.writeText(command).catch(() => undefined);
    setCopied(true);
    setTimeout(() => setCopied(false), 1800);
  };

  return (
    <>
      <div className="flex flex-col gap-4">
        <div role="tablist" aria-label={m.pair_os()} className="flex gap-1 rounded-lg bg-muted p-1">
          {OS_TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              role="tab"
              aria-selected={tab.id === os}
              onClick={() => setOs(tab.id)}
              className={cn(
                "h-9 flex-1 cursor-pointer rounded-md text-sm",
                tab.id === os
                  ? "bg-background font-medium shadow-sm"
                  : "text-zinc-700 dark:text-zinc-300",
              )}
            >
              {tab.label}
            </button>
          ))}
        </div>
        <div className="flex flex-col gap-2">
          <span className="text-sm font-medium">{m.pair_step_install()}</span>
          <div className="flex items-stretch gap-2 rounded-lg border border-border bg-subtle p-2 pl-3">
            <code
              className="min-w-0 flex-1 self-center font-mono text-[13px] break-all"
              data-testid="pair-command"
            >
              {command}
            </code>
            <Button
              variant="outline"
              size="sm"
              onClick={copy}
              disabled={!code}
              aria-label={m.pair_copy_command()}
            >
              <Copy aria-hidden="true" />
              {copied ? m.pair_copied() : m.pair_copy()}
            </Button>
          </div>
          <span className="text-xs text-muted-foreground">{m.pair_code_valid()}</span>
          <span className="text-xs text-muted-foreground">{m.pair_lan_hint()}</span>
          {error && <span className="text-sm text-danger">{error}</span>}
        </div>
        <div className="flex items-center gap-3 rounded-lg border border-dashed border-border-strong p-3">
          <span className="size-2.5 animate-pulse rounded-full bg-info-accent" />
          <div className="flex flex-col">
            <span className="font-medium">{m.pair_step_wait()}</span>
            <span className="text-[13px] text-muted-foreground">{m.pair_wait_hint()}</span>
          </div>
        </div>
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
  const [picked, setPicked] = useState<ReadonlySet<string>>(new Set());

  const save = useMutation({
    mutationFn: async () => {
      if (name.trim() && name.trim() !== device.name) {
        await api.updateDevice(device.id, { name: name.trim() });
      }
      if (picked.size > 0) {
        await api.applyShares(
          [...picked].map((folderId) => ({ folderId, deviceId: device.id, mode: "sendReceive" })),
        );
      }
    },
    onSuccess: async () => {
      await refresh(queryClient);
      onDone();
    },
  });

  const toggle = (folderId: string) => {
    const next = new Set(picked);
    if (next.has(folderId)) next.delete(folderId);
    else next.add(folderId);
    setPicked(next);
  };

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
            <label
              key={folder.id}
              className="flex min-h-11 cursor-pointer items-center gap-3 rounded-md border border-border px-3"
            >
              <input
                type="checkbox"
                className="size-4 accent-primary"
                checked={picked.has(folder.id)}
                onChange={() => toggle(folder.id)}
              />
              <span className="flex-1">{folder.label}</span>
            </label>
          ))}
          <span className="text-xs text-muted-foreground">{m.pair_default_mode()}</span>
        </fieldset>
        {save.error && <p className="text-sm text-danger">{save.error.message}</p>}
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
