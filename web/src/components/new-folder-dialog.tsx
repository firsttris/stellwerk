import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { type FormEvent, useState } from "react";
import { api } from "../api";
import { refresh } from "../api/queries";
import { errorMessage } from "../lib/errors";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";
import { Dialog, DialogContent, DialogTrigger } from "./ui/dialog";
import { Field, Input } from "./ui/input";

export function NewFolderDialog() {
  const [open, setOpen] = useState(false);
  const [label, setLabel] = useState("");
  const queryClient = useQueryClient();
  const create = useMutation({
    mutationFn: (name: string) => api.createFolder(name),
    onSuccess: async () => {
      await refresh(queryClient);
      setOpen(false);
      setLabel("");
    },
  });

  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (label.trim()) create.mutate(label);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button>
          <Plus aria-hidden="true" />
          {m.new_folder()}
        </Button>
      </DialogTrigger>
      <DialogContent title={m.new_folder()} description={m.new_folder_description()}>
        <form onSubmit={submit} className="flex flex-col gap-4">
          <Field label={m.name()}>
            {(id) => (
              <Input
                id={id}
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder={m.new_folder_placeholder()}
                autoFocus
              />
            )}
          </Field>
          {create.error && <p className="text-sm text-danger">{errorMessage(create.error)}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              {m.cancel()}
            </Button>
            <Button type="submit" disabled={!label.trim() || create.isPending}>
              {m.create()}
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
