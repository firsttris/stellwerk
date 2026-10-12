import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { overviewQuery, statusQuery } from "../../api/queries";
import { ConflictCard } from "../../components/conflict-card";
import { PageHeader } from "../../components/layout";
import { Card } from "../../components/ui/card";
import { m } from "../../paraglide/messages.js";

export const Route = createFileRoute("/_app/conflicts")({ component: Conflicts });

/** Every conflict in every folder, found on the server. */
function Conflicts() {
  const { data: overview } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  const folders = (overview?.folders ?? []).filter((f) => (live?.conflicts[f.id]?.length ?? 0) > 0);
  return (
    <>
      <PageHeader title={m.conflicts_title()} description={m.conflicts_description()} />
      {live && folders.length === 0 && (
        <Card className="p-5 text-muted-foreground">{m.conflicts_none()}</Card>
      )}
      {overview &&
        folders.map((folder) => (
          <section key={folder.id} className="flex flex-col gap-3" aria-label={folder.label}>
            <h2 className="text-base font-semibold">
              <Link
                to="/folders/$folderId"
                params={{ folderId: folder.id }}
                search={{ tab: "conflicts" }}
                className="hover:underline"
              >
                {folder.label}
              </Link>
            </h2>
            {live?.conflicts[folder.id]?.map((conflict) => (
              <ConflictCard
                key={conflict.path}
                folderId={folder.id}
                conflict={conflict}
                devices={overview.devices}
              />
            ))}
          </section>
        ))}
    </>
  );
}
