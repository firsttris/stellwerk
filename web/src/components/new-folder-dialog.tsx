import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { type FormEvent, useState } from "react";
import { api } from "../api";
import { overviewQuery, refresh } from "../api/queries";
import type { Device } from "../api/types";
import { errorMessage } from "../lib/errors";
import { matrixDevices } from "../lib/matrix";
import { baseName } from "../lib/paths";
import { m } from "../paraglide/messages.js";
import { PathField } from "./path-field";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogTrigger } from "./ui/dialog";
import { Field, Input, Select } from "./ui/input";

type NewFolder = { label: string; device?: Device; path: string };

/** The folder on the server, and with a device chosen, on that device at once ("both"). */
async function createFolder({ label, device, path }: NewFolder) {
  const folder = await api.createFolder(label);
  if (device) {
    await api.applyShares([
      {
        folderId: folder.id,
        deviceId: device.id,
        mode: "sendReceive",
        path: path.trim() || undefined,
      },
    ]);
  }
}

export function NewFolderDialog() {
  const [open, setOpen] = useState(false);
  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>
          <Plus aria-hidden="true" />
          {m.new_folder()}
        </Button>
      </DialogTrigger>
      {open && <NewFolderForm onDone={() => setOpen(false)} />}
    </Dialog>
  );
}

function NewFolderForm({ onDone }: { onDone: () => void }) {
  const overview = useQuery(overviewQuery).data;
  const devices = overview ? matrixDevices(overview) : [];
  const [label, setLabel] = useState("");
  // The first online device: the folder usually starts where its files are.
  const [deviceId, setDeviceId] = useState(() => devices.find((d) => d.online)?.id ?? "");
  const [path, setPath] = useState("");
  const device = devices.find((d) => d.id === deviceId);
  const queryClient = useQueryClient();
  const create = useMutation({
    mutationFn: createFolder,
    onSuccess: async () => {
      await refresh(queryClient);
      onDone();
    },
  });

  const pick = (picked: string) => {
    if (!label.trim() || label === baseName(path)) setLabel(baseName(picked));
    setPath(picked);
  };
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (label.trim()) create.mutate({ label, device, path });
  };

  return (
    <DialogContent title={m.new_folder()} description={m.new_folder_description()}>
      <form onSubmit={submit} className="flex flex-col gap-4">
        {devices.length > 0 && (
          <Field label={m.new_folder_device()}>
            {(id) => (
              <Select id={id} value={deviceId} onChange={(e) => setDeviceId(e.target.value)}>
                {devices.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.name}
                  </option>
                ))}
                <option value="">{m.new_folder_server_only()}</option>
              </Select>
            )}
          </Field>
        )}
        {device && (
          <Field label={m.new_folder_path({ device: device.name })}>
            {(id) => (
              <PathField
                id={id}
                device={device}
                value={path}
                onChange={setPath}
                onPick={pick}
                placeholder={`~/Stellwerk/${label.trim()}`}
              />
            )}
          </Field>
        )}
        <Field label={m.name()}>
          {(id) => (
            <Input
              id={id}
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              placeholder={m.new_folder_placeholder()}
            />
          )}
        </Field>
        {create.error && <p className="text-sm text-danger">{errorMessage(create.error)}</p>}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onDone}>
            {m.cancel()}
          </Button>
          <Button type="submit" disabled={!label.trim() || create.isPending}>
            {m.create()}
          </Button>
        </div>
      </form>
    </DialogContent>
  );
}
