import { useQuery } from "@tanstack/react-query";
import { ArrowUp, Folder, FolderOpen, House } from "lucide-react";
import { type FormEvent, useState } from "react";
import { api } from "../api";
import type { Device } from "../api/types";
import { errorMessage } from "../lib/errors";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogTrigger } from "./ui/dialog";
import { Input } from "./ui/input";

/**
 * A button that opens the device's folders, listed by its agent, and hands back the one picked.
 * Starts at `value` when set, else in the home directory.
 */
export function FolderPicker({
  device,
  value,
  onPick,
  disabled,
}: {
  device: Device;
  value: string;
  onPick: (path: string) => void;
  disabled?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const label = device.online
    ? m.picker_open({ device: device.name })
    : m.picker_offline({ device: device.name });
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          type="button"
          variant="outline"
          size="icon"
          aria-label={label}
          title={label}
          disabled={disabled || !device.online}
        >
          <FolderOpen aria-hidden="true" />
        </Button>
      </DialogTrigger>
      {open && (
        <Browser
          device={device}
          start={value.trim()}
          onPick={(path) => {
            onPick(path);
            setOpen(false);
          }}
          onCancel={() => setOpen(false)}
        />
      )}
    </Dialog>
  );
}

function Browser({
  device,
  start,
  onPick,
  onCancel,
}: {
  device: Device;
  start: string;
  onPick: (path: string) => void;
  onCancel: () => void;
}) {
  const [path, setPath] = useState(start);
  const [typed, setTyped] = useState<string>();
  const listing = useQuery({
    queryKey: ["browse", device.id, path],
    queryFn: () => api.browse(device.id, path),
    retry: false,
    staleTime: 10_000,
  });
  const directory = listing.data;
  const go = (next: string) => {
    setTyped(undefined);
    setPath(next);
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (typed !== undefined) go(typed);
  };

  return (
    <DialogContent
      title={m.picker_title({ device: device.name })}
      description={m.picker_description()}
    >
      <form onSubmit={submit} className="flex gap-2">
        <Button
          type="button"
          variant="outline"
          size="icon"
          aria-label={m.picker_up()}
          title={m.picker_up()}
          disabled={!directory?.parent}
          onClick={() => directory?.parent && go(directory.parent)}
        >
          <ArrowUp aria-hidden="true" />
        </Button>
        <Button
          type="button"
          variant="outline"
          size="icon"
          aria-label={m.picker_home()}
          title={m.picker_home()}
          onClick={() => go("")}
        >
          <House aria-hidden="true" />
        </Button>
        <Input
          aria-label={m.picker_path()}
          className="font-mono"
          value={typed ?? directory?.path ?? path}
          onChange={(e) => setTyped(e.target.value)}
        />
      </form>
      {directory && directory.roots.length > 1 && (
        <div className="flex flex-wrap gap-2">
          {directory.roots.map((root) => (
            <Button key={root} type="button" variant="ghost" size="sm" onClick={() => go(root)}>
              {root}
            </Button>
          ))}
        </div>
      )}
      <div className="h-72 overflow-y-auto rounded-md border border-border p-1">
        {listing.isPending && (
          <p className="p-3 text-sm text-muted-foreground">{m.picker_loading()}</p>
        )}
        {listing.error && <p className="p-3 text-sm text-danger">{errorMessage(listing.error)}</p>}
        {directory?.dirs.length === 0 && (
          <p className="p-3 text-sm text-muted-foreground">{m.picker_empty()}</p>
        )}
        {directory && directory.dirs.length > 0 && (
          <ul aria-label={m.picker_list()}>
            {directory.dirs.map((entry) => (
              <li key={entry.path}>
                <button
                  type="button"
                  className="flex w-full cursor-pointer items-center gap-2 rounded px-2.5 py-2 text-left text-sm hover:bg-muted"
                  onClick={() => go(entry.path)}
                >
                  <Folder className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                  <span className="truncate">{entry.name}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="flex justify-end gap-2">
        <Button type="button" variant="outline" onClick={onCancel}>
          {m.cancel()}
        </Button>
        <Button
          type="button"
          disabled={!directory}
          onClick={() => directory && onPick(directory.path)}
        >
          {m.picker_choose()}
        </Button>
      </div>
    </DialogContent>
  );
}
