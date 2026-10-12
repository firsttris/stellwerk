import { useQuery } from "@tanstack/react-query";
import { createFileRoute, Link, useNavigate } from "@tanstack/react-router";
import { Folder as FolderIcon } from "lucide-react";
import { overviewQuery, statusQuery } from "../../../api/queries";
import { ConflictCard } from "../../../components/conflict-card";
import { FolderDevices } from "../../../components/folder-devices";
import { FolderSettings } from "../../../components/folder-settings";
import { Card } from "../../../components/ui/card";
import { VersionsList } from "../../../components/versions-list";
import { byCount } from "../../../lib/i18n";
import { holders } from "../../../lib/live";
import { cn } from "../../../lib/utils";
import { m } from "../../../paraglide/messages.js";

const TABS = ["devices", "versions", "conflicts", "settings"] as const;
type Tab = (typeof TABS)[number];
const tabLabel = (tab: Tab): string =>
  ({
    devices: m.folder_tab_devices,
    versions: m.folder_tab_versions,
    conflicts: m.folder_tab_conflicts,
    settings: m.folder_tab_settings,
  })[tab]();

export const Route = createFileRoute("/_app/folders/$folderId")({
  validateSearch: (search: Record<string, unknown>): { tab?: Tab } => ({
    tab: TABS.find((t) => t === search.tab),
  }),
  component: FolderPage,
});

function FolderPage() {
  const { folderId } = Route.useParams();
  const { data: overview } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  const { tab = "devices" } = Route.useSearch();
  const navigate = useNavigate({ from: Route.fullPath });
  const setTab = (next: Tab) => navigate({ search: { tab: next }, replace: true });
  const folder = overview?.folders.find((f) => f.id === folderId);

  if (!overview) return null;
  if (!folder) {
    return (
      <p className="text-muted-foreground">
        {m.folder_gone()}{" "}
        <Link to="/folders" className="underline">
          {m.folder_back()}
        </Link>
      </p>
    );
  }
  const holderCount = holders(overview, folder.id).length;
  const conflicts = live?.conflicts[folder.id] ?? [];

  return (
    <>
      <nav
        aria-label={m.folder_breadcrumb()}
        className="flex items-center gap-2 text-[13px] text-muted-foreground"
      >
        <Link to="/folders" className="hover:underline">
          {m.folders_title()}
        </Link>
        <span>/</span>
        <span className="text-foreground">{folder.label}</span>
      </nav>
      <div className="flex items-center gap-3.5">
        <div className="grid size-11 place-items-center rounded-xl bg-muted">
          <FolderIcon className="size-5" aria-hidden="true" />
        </div>
        <div className="flex flex-col gap-0.5">
          <h1 className="text-2xl font-semibold tracking-tight">{folder.label}</h1>
          <span className="text-[13px] text-muted-foreground">
            {byCount(holderCount, m.folder_meta_one, () =>
              m.folder_meta_other({ count: holderCount }),
            )}{" "}
            <span className="font-mono">{folder.syncthingFolderId}</span>
          </span>
        </div>
      </div>
      <div
        role="tablist"
        aria-label={m.folder_tabs()}
        className="flex gap-1 border-b border-border"
      >
        {TABS.map((t) => (
          <button
            key={t}
            type="button"
            role="tab"
            aria-selected={tab === t}
            onClick={() => setTab(t)}
            className={cn(
              "-mb-px h-10 cursor-pointer border-b-2 px-3 text-sm",
              tab === t ? "border-primary font-medium" : "border-transparent text-muted-foreground",
            )}
          >
            {tabLabel(t)}
            {t === "conflicts" && conflicts.length > 0 && (
              <span className="ml-1.5 rounded-full bg-warn px-1.5 text-xs text-warn-foreground">
                {conflicts.length}
              </span>
            )}
          </button>
        ))}
      </div>
      {tab === "devices" && (
        <FolderDevices key={folder.id} overview={overview} live={live} folder={folder} />
      )}
      {tab === "versions" && <VersionsList folderId={folder.id} />}
      {tab === "conflicts" &&
        (conflicts.length === 0 ? (
          <Card className="p-5 text-muted-foreground">{m.folder_no_conflicts()}</Card>
        ) : (
          conflicts.map((conflict) => (
            <ConflictCard
              key={conflict.path}
              folderId={folder.id}
              conflict={conflict}
              devices={overview.devices}
            />
          ))
        ))}
      {tab === "settings" && <FolderSettings folder={folder} />}
    </>
  );
}
