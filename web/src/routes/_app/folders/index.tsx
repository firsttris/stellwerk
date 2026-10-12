import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link } from "@tanstack/react-router";
import { ChevronRight, Folder } from "lucide-react";
import { overviewQuery } from "../../../api/queries";
import { PageHeader } from "../../../components/layout";
import { NewFolderDialog } from "../../../components/new-folder-dialog";
import { Card } from "../../../components/ui/card";
import { byCount } from "../../../lib/i18n";
import { holders } from "../../../lib/live";
import { m } from "../../../paraglide/messages.js";

export const Route = createFileRoute("/_app/folders/")({ component: Folders });

function Folders() {
  const { data: overview } = useQuery(overviewQuery);
  return (
    <>
      <PageHeader
        title={m.folders_title()}
        description={m.folders_description()}
        actions={<NewFolderDialog />}
      />
      <Card className="divide-y divide-muted">
        {overview?.folders.length === 0 && (
          <p className="px-5 py-10 text-center text-muted-foreground">{m.folders_none()}</p>
        )}
        {overview?.folders.map((folder) => {
          const count = holders(overview, folder.id).length;
          return (
            <Link
              key={folder.id}
              to="/folders/$folderId"
              params={{ folderId: folder.id }}
              className="flex items-center gap-3 px-5 py-4 hover:bg-subtle"
            >
              <div className="grid size-9 place-items-center rounded-lg bg-muted">
                <Folder className="size-[18px]" aria-hidden="true" />
              </div>
              <div className="flex flex-1 flex-col">
                <span className="font-medium">{folder.label}</span>
                <span className="text-xs text-muted-foreground">
                  {byCount(count, m.folders_on_devices_one, () =>
                    m.folders_on_devices_other({ count }),
                  )}{" "}
                  · <span className="font-mono">{folder.syncthingFolderId}</span>
                </span>
              </div>
              <ChevronRight className="size-4 text-muted-foreground" aria-hidden="true" />
            </Link>
          );
        })}
      </Card>
    </>
  );
}
