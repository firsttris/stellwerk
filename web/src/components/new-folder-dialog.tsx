import { useMutation, useQueryClient } from "@tanstack/react-query";
import { Plus } from "lucide-react";
import { type FormEvent, useState } from "react";
import { api } from "../api";
import { refresh } from "../api/queries";
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
          Neuer Ordner
        </Button>
      </DialogTrigger>
      <DialogContent
        title="Neuer Ordner"
        description="Der Server hält von jedem Ordner eine Kopie. Welche Geräte ihn bekommen, legst du danach in der Übersicht fest."
      >
        <form onSubmit={submit} className="flex flex-col gap-4">
          <Field label="Name">
            {(id) => (
              <Input
                id={id}
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder="z. B. Fotos"
                autoFocus
              />
            )}
          </Field>
          {create.error && <p className="text-sm text-danger">{create.error.message}</p>}
          <div className="flex justify-end gap-2">
            <Button type="button" variant="outline" onClick={() => setOpen(false)}>
              Abbrechen
            </Button>
            <Button type="submit" disabled={!label.trim() || create.isPending}>
              Anlegen
            </Button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
