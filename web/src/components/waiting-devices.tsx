import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Radar } from "lucide-react";
import { api } from "../api";
import { refresh, statusQuery } from "../api/queries";
import type { WaitingDevice } from "../api/types";
import { errorMessage } from "../lib/errors";
import { nowSeconds, osLabel, since } from "../lib/format";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";
import { Card } from "./ui/card";

/**
 * Devices in the LAN that came without a code (LAN pairing). They are let in here, after
 * comparing the fingerprint with the one the device printed.
 */
export function WaitingDevices({ waiting }: { waiting: readonly WaitingDevice[] }) {
  if (waiting.length === 0) return null;
  return (
    <Card aria-labelledby="waiting-title" className="flex flex-col gap-3 px-5 py-4">
      <div className="flex flex-col gap-0.5">
        <h2 id="waiting-title" className="text-base font-semibold">
          {m.waiting_title()}
        </h2>
        <p className="text-[13px] text-muted-foreground">{m.waiting_description()}</p>
      </div>
      <ul className="flex flex-col divide-y divide-border">
        {waiting.map((device) => (
          <WaitingRow key={device.fingerprint} device={device} />
        ))}
      </ul>
    </Card>
  );
}

function WaitingRow({ device }: { device: WaitingDevice }) {
  const queryClient = useQueryClient();
  const onSuccess = () =>
    Promise.all([
      queryClient.invalidateQueries({ queryKey: statusQuery.queryKey }),
      refresh(queryClient),
    ]);
  const approve = useMutation({
    mutationFn: () => api.approveWaiting(device.fingerprint),
    onSuccess,
  });
  const reject = useMutation({
    mutationFn: () => api.rejectWaiting(device.fingerprint),
    onSuccess,
  });
  const error = approve.error ?? reject.error;
  const busy = approve.isPending || reject.isPending;
  return (
    <li className="flex flex-wrap items-center gap-3 py-3 first:pt-1 last:pb-1">
      <div className="grid size-8 shrink-0 place-items-center rounded-lg bg-muted">
        <Radar className="size-4" aria-hidden="true" />
      </div>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="font-medium">{device.name}</span>
        <span className="text-[13px] text-muted-foreground">
          {m.waiting_meta({ os: osLabel(device.os), since: since(nowSeconds() - device.since) })}
        </span>
        <span className="text-[13px]">
          {m.waiting_fingerprint()}{" "}
          <span className="font-mono tracking-wide">{device.fingerprint}</span>
        </span>
        {error && <span className="text-[13px] text-danger">{errorMessage(error)}</span>}
      </div>
      <div className="flex gap-2">
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => reject.mutate()}
          aria-label={m.waiting_reject_device({ device: device.name })}
        >
          {m.waiting_reject()}
        </Button>
        <Button
          size="sm"
          disabled={busy}
          onClick={() => approve.mutate()}
          aria-label={m.waiting_approve_device({ device: device.name })}
        >
          {m.waiting_approve()}
        </Button>
      </div>
    </li>
  );
}
