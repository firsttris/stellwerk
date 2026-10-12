import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { type FormEvent, useState } from "react";
import { api } from "../api";
import { refresh } from "../api/queries";
import type { Folder, FolderPatch } from "../api/types";
import { errorMessage } from "../lib/errors";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";
import { Card } from "./ui/card";
import { Field, Input } from "./ui/input";

/** Name, ignore patterns and removal of a folder. */
export function FolderSettings({ folder }: { folder: Folder }) {
  const [label, setLabel] = useState(folder.label);
  const [ignores, setIgnores] = useState(folder.ignorePatterns.join("\n"));
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const update = useMutation({
    mutationFn: (patch: FolderPatch) => api.updateFolder(folder.id, patch),
    onSuccess: () => refresh(queryClient),
  });
  const remove = useMutation({
    mutationFn: () => api.deleteFolder(folder.id),
    onSuccess: async () => {
      await refresh(queryClient);
      await navigate({ to: "/folders" });
    },
  });
  const rename = (event: FormEvent) => {
    event.preventDefault();
    update.mutate({ label });
  };
  const ignoreLines = ignores
    .split("\n")
    .map((line) => line.trimEnd())
    .filter(Boolean);
  const ignoresChanged = ignoreLines.join("\n") !== folder.ignorePatterns.join("\n");
  const keep =
    folder.versioning.type === "staggered"
      ? m.settings_keep_staggered({ days: folder.versioning.max_age_days })
      : m.settings_keep_none();

  return (
    <div className="flex flex-col gap-4">
      <Card className="flex flex-col gap-4 p-5">
        <h2 className="text-base font-semibold">{m.name()}</h2>
        <form onSubmit={rename} className="flex flex-wrap gap-2">
          <Input
            aria-label={m.settings_name_label()}
            className="max-w-sm"
            value={label}
            onChange={(e) => setLabel(e.target.value)}
          />
          <Button
            type="submit"
            variant="outline"
            disabled={!label.trim() || label === folder.label}
          >
            {m.settings_rename()}
          </Button>
        </form>
        <p className="text-[13px] text-muted-foreground">{m.settings_versions({ keep })}</p>
      </Card>
      <Card className="flex flex-col gap-3 p-5">
        <h2 className="text-base font-semibold">{m.settings_ignore()}</h2>
        <Field label={m.settings_ignore_label()}>
          {(id) => (
            <textarea
              id={id}
              rows={6}
              value={ignores}
              onChange={(e) => setIgnores(e.target.value)}
              placeholder={"node_modules/\n*.log"}
              className="w-full rounded-md border border-border bg-background px-3 py-2 font-mono text-[13px]"
            />
          )}
        </Field>
        <div className="flex items-center justify-between gap-3">
          <a
            href="https://docs.syncthing.net/users/ignoring.html"
            target="_blank"
            rel="noreferrer"
            className="text-[13px] text-muted-foreground underline"
          >
            {m.settings_ignore_syntax()}
          </a>
          <Button
            variant="outline"
            disabled={!ignoresChanged || update.isPending}
            onClick={() => update.mutate({ ignorePatterns: ignoreLines })}
          >
            {m.settings_ignore_save()}
          </Button>
        </div>
        {update.error && <p className="text-sm text-danger">{errorMessage(update.error)}</p>}
      </Card>
      <Card className="flex flex-wrap items-center justify-between gap-4 border-danger/30 p-5">
        <div className="flex flex-col gap-0.5">
          <span className="font-medium">{m.settings_remove()}</span>
          <span className="text-[13px] text-muted-foreground">{m.settings_remove_hint()}</span>
        </div>
        <Button
          variant="destructive"
          onClick={() => {
            if (window.confirm(m.settings_remove_confirm({ folder: folder.label }))) {
              remove.mutate();
            }
          }}
        >
          {m.remove()}
        </Button>
      </Card>
    </div>
  );
}
