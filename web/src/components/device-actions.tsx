import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Ellipsis } from "lucide-react";
import { DropdownMenu } from "radix-ui";
import { api } from "../api";
import { refresh } from "../api/queries";
import type { Device } from "../api/types";
import { errorMessage } from "../lib/errors";
import { cn } from "../lib/utils";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";

/** Rename, block or remove a device; a failure shows under the menu. */
export function DeviceActions({ device }: { device: Device }) {
  const queryClient = useQueryClient();
  const update = useMutation({
    mutationFn: (patch: Parameters<typeof api.updateDevice>[1]) =>
      api.updateDevice(device.id, patch),
    onSuccess: () => refresh(queryClient),
  });
  const remove = useMutation({
    mutationFn: () => api.deleteDevice(device.id),
    onSuccess: () => refresh(queryClient),
  });

  const rename = () => {
    const name = window.prompt(m.devices_new_name(), device.name)?.trim();
    if (name && name !== device.name) update.mutate({ name });
  };
  const confirmRemove = () => {
    if (window.confirm(m.devices_remove_confirm({ device: device.name }))) {
      remove.mutate();
    }
  };

  const error = update.error ?? remove.error;
  const item =
    "flex h-9 cursor-pointer items-center rounded px-2.5 text-sm outline-none data-[highlighted]:bg-muted";
  return (
    <div className="flex flex-col items-end gap-1">
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button
            variant="ghost"
            size="icon"
            aria-label={m.devices_actions({ device: device.name })}
          >
            <Ellipsis aria-hidden="true" />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content
            align="end"
            className="z-50 min-w-44 rounded-md border border-border bg-background p-1 shadow-lg"
          >
            <DropdownMenu.Item className={item} onSelect={rename}>
              {m.devices_rename()}
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={item}
              onSelect={() => update.mutate({ approved: !device.approved })}
            >
              {device.approved ? m.devices_block() : m.devices_unblock()}
            </DropdownMenu.Item>
            <DropdownMenu.Item className={cn(item, "text-danger")} onSelect={confirmRemove}>
              {m.remove()}
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
      {error && (
        <span role="alert" className="max-w-48 text-right text-xs text-danger">
          {errorMessage(error)}
        </span>
      )}
    </div>
  );
}
