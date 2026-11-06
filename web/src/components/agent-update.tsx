import { useMutation, useQueryClient } from "@tanstack/react-query";
import { api } from "../api";
import { refresh } from "../api/queries";
import type { Device, Live, Overview } from "../api/types";
import { agentOutdated } from "../lib/devices";
import { errorMessage } from "../lib/errors";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";

const UPDATING_DOCS = "https://firsttris.github.io/stellwerk/installation.html#updating";

/** Tells that a device's agent is older than the Zentrale and updates it, or shows how to. */
export function AgentUpdate({
  overview,
  live,
  device,
}: {
  overview: Overview;
  live: Live | undefined;
  device: Device;
}) {
  const queryClient = useQueryClient();
  const update = useMutation({
    mutationFn: () => api.updateAgent(device.id),
    onSuccess: () => refresh(queryClient),
  });
  if (!device.approved || !agentOutdated(device, overview.version)) return null;
  const canUpdate = live?.devices[device.id]?.canUpdate ?? false;
  const status = () => {
    if (!device.online) return m.update_offline();
    if (update.isError) return errorMessage(update.error);
    if (update.isPending) return m.update_running();
    return update.isSuccess ? m.update_sent() : m.update_hint();
  };

  return (
    <div className="mt-1 flex flex-col gap-1 text-xs">
      <span className="font-medium">
        {m.update_available()} ·{" "}
        {m.update_versions({ from: device.agentVersion, to: overview.version })}
      </span>
      {canUpdate || !device.online ? (
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => update.mutate()}
            disabled={!device.online || update.isPending || update.isSuccess}
          >
            {m.update_button()}
          </Button>
          <span className="text-muted-foreground" role="status">
            {status()}
          </span>
        </div>
      ) : (
        <span className="text-muted-foreground">
          {m.update_manual()}{" "}
          <a className="underline" href={UPDATING_DOCS} target="_blank" rel="noreferrer">
            {m.update_manual_link()}
          </a>
        </span>
      )}
    </div>
  );
}
