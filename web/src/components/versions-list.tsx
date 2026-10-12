import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { History, Search } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { eventsQuery } from "../api/queries";
import { intlLocale } from "../lib/i18n";
import { formatBytes } from "../lib/live";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";
import { Card } from "./ui/card";

const formatDate = (iso: string) =>
  new Intl.DateTimeFormat(intlLocale(), { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(iso),
  );

/** Archived versions on the server, per file; restoring keeps the current file as a version. */
export function VersionsList({ folderId }: { folderId: string }) {
  const [search, setSearch] = useState("");
  const [restored, setRestored] = useState("");
  const queryClient = useQueryClient();
  const versions = useQuery({
    queryKey: ["versions", folderId],
    queryFn: () => api.versions(folderId),
  });
  const restore = useMutation({
    mutationFn: ({ path, versionTime }: { path: string; versionTime: string }) =>
      api.restoreVersion(folderId, path, versionTime),
    onSuccess: async (_, { path }) => {
      setRestored(path);
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: ["versions", folderId] }),
        queryClient.invalidateQueries({ queryKey: eventsQuery.queryKey }),
      ]);
    },
  });
  const files = (versions.data ?? []).filter((f) =>
    f.path.toLowerCase().includes(search.trim().toLowerCase()),
  );

  return (
    <div className="flex flex-col gap-4">
      <p className="text-[13px] text-muted-foreground">{m.versions_explained()}</p>
      <label className="relative flex items-center">
        <span className="sr-only">{m.versions_search_label()}</span>
        <Search
          className="pointer-events-none absolute left-3 size-4 text-muted-foreground"
          aria-hidden="true"
        />
        <input
          type="search"
          placeholder={m.versions_search()}
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="h-10 w-full rounded-md border border-border bg-background pr-3 pl-9"
        />
      </label>
      {restored && (
        <p className="rounded-md border border-ok-border bg-ok-surface px-3 py-2 text-ok-foreground">
          {m.versions_restored({ path: restored })}
        </p>
      )}
      {restore.error && <p className="text-sm text-danger">{restore.error.message}</p>}
      {versions.error && <p className="text-sm text-danger">{versions.error.message}</p>}
      {versions.data && files.length === 0 && (
        <p className="text-muted-foreground">
          {search ? m.versions_none_search() : m.versions_none()}
        </p>
      )}
      <Card className="divide-y divide-muted">
        {files.map((file) => (
          <div key={file.path} className="flex flex-col gap-2 px-5 py-3">
            <span className="inline-flex items-center gap-2 font-mono text-[13px] font-medium break-all">
              <History className="size-4 shrink-0" aria-hidden="true" />
              {file.path}
            </span>
            <ul className="flex flex-col gap-1.5">
              {file.versions.map((version) => (
                <li
                  key={version.versionTime}
                  className="flex flex-wrap items-center justify-between gap-2 pl-6 text-[13px]"
                >
                  <span>
                    {formatDate(version.versionTime)}
                    <span className="text-muted-foreground"> · {formatBytes(version.size)}</span>
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={restore.isPending}
                    aria-label={m.versions_restore_label({
                      path: file.path,
                      date: formatDate(version.versionTime),
                    })}
                    onClick={() => {
                      if (window.confirm(m.versions_restore_confirm({ path: file.path }))) {
                        restore.mutate({ path: file.path, versionTime: version.versionTime });
                      }
                    }}
                  >
                    {m.versions_restore()}
                  </Button>
                </li>
              ))}
            </ul>
          </div>
        ))}
      </Card>
    </div>
  );
}
