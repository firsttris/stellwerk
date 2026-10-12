import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { History, Search } from "lucide-react";
import { useState } from "react";
import { api } from "../api";
import { eventsQuery } from "../api/queries";
import { formatBytes } from "../lib/live";
import { Button } from "./ui/button";
import { Card } from "./ui/card";

const DATE = new Intl.DateTimeFormat("de-DE", { dateStyle: "medium", timeStyle: "short" });

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
      <p className="text-[13px] text-muted-foreground">
        Wenn ein Gerät eine Datei ändert oder löscht, hebt der Server die vorige Fassung auf.
        Wiederherstellen legt die aktuelle Fassung vorher ebenfalls dort ab.
      </p>
      <label className="relative flex items-center">
        <span className="sr-only">Versionen durchsuchen</span>
        <Search
          className="pointer-events-none absolute left-3 size-4 text-muted-foreground"
          aria-hidden="true"
        />
        <input
          type="search"
          placeholder="Datei suchen"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          className="h-10 w-full rounded-md border border-border bg-background pr-3 pl-9"
        />
      </label>
      {restored && (
        <p className="rounded-md border border-ok-border bg-ok-surface px-3 py-2 text-ok-foreground">
          {restored} wiederhergestellt. Die Geräte bekommen die Datei beim nächsten Abgleich.
        </p>
      )}
      {restore.error && <p className="text-sm text-danger">{restore.error.message}</p>}
      {versions.error && <p className="text-sm text-danger">{versions.error.message}</p>}
      {versions.data && files.length === 0 && (
        <p className="text-muted-foreground">Keine Versionen{search ? " zu dieser Suche" : ""}.</p>
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
                    {DATE.format(new Date(version.versionTime))}
                    <span className="text-muted-foreground"> · {formatBytes(version.size)}</span>
                  </span>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={restore.isPending}
                    aria-label={`${file.path} vom ${DATE.format(new Date(version.versionTime))} wiederherstellen`}
                    onClick={() => {
                      if (window.confirm(`${file.path} auf diese Fassung zurücksetzen?`)) {
                        restore.mutate({ path: file.path, versionTime: version.versionTime });
                      }
                    }}
                  >
                    Wiederherstellen
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
