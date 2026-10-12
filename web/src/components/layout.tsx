import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import { Folder, LayoutGrid, LogOut, Monitor, RefreshCw } from "lucide-react";
import type { ReactNode } from "react";
import { api } from "../api";
import { useLiveUpdates } from "../api/live";
import { overviewQuery, sessionQuery } from "../api/queries";
import { Button } from "./ui/button";

const navLink =
  "flex min-h-9 items-center gap-2.5 rounded-md px-2.5 py-2 text-zinc-700 hover:bg-muted dark:text-zinc-300 [&_svg]:size-4";
const navActive = "bg-muted font-medium text-foreground";

function hostOf(address: string): string {
  return address.replace(/^\w+:\/\//, "").replace(/:\d+$/, "");
}

export function Layout({ children }: { children: ReactNode }) {
  const { data: overview } = useQuery(overviewQuery);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  useLiveUpdates();

  const logout = async () => {
    await api.logout();
    queryClient.removeQueries({ queryKey: sessionQuery.queryKey });
    await navigate({ to: "/login" });
  };

  return (
    <div className="flex min-h-screen flex-wrap">
      <aside className="flex max-w-full flex-[1_1_248px] flex-col gap-5 border-r border-border bg-subtle px-3 py-4">
        <div className="flex items-center gap-2.5 px-2 py-1">
          <div className="grid size-8 place-items-center rounded-lg bg-primary text-primary-foreground">
            <RefreshCw className="size-[18px]" aria-hidden="true" />
          </div>
          <div className="flex flex-col">
            <span className="font-semibold">Stellwerk</span>
            <span className="font-mono text-xs text-muted-foreground">
              {overview ? hostOf(overview.hub.address) : "…"}
            </span>
          </div>
        </div>
        <nav aria-label="Hauptnavigation" className="flex flex-col gap-0.5">
          <Link
            to="/"
            className={navLink}
            activeProps={{ className: navActive }}
            activeOptions={{ exact: true }}
          >
            <LayoutGrid aria-hidden="true" />
            Übersicht
          </Link>
          <Link to="/folders" className={navLink} activeProps={{ className: navActive }}>
            <Folder aria-hidden="true" />
            Ordner
            <span className="ml-auto text-xs text-muted-foreground">
              {overview?.folders.length}
            </span>
          </Link>
          <Link to="/devices" className={navLink} activeProps={{ className: navActive }}>
            <Monitor aria-hidden="true" />
            Geräte
            <span className="ml-auto text-xs text-muted-foreground">
              {overview?.devices.length}
            </span>
          </Link>
        </nav>
        <div className="mt-auto flex flex-col gap-3">
          <div className="flex flex-col gap-1 rounded-lg border border-border bg-background p-3">
            <div className="flex items-center gap-2 font-medium">
              <span className="size-2 rounded-full bg-ok" />
              Server online
            </div>
            <span className="text-xs text-muted-foreground">
              Hub hält jeden Ordner, Revision {overview?.revision ?? "…"}
            </span>
          </div>
          <Button variant="ghost" className="justify-start" onClick={logout}>
            <LogOut aria-hidden="true" />
            Abmelden
          </Button>
        </div>
      </aside>
      <main className="min-w-0 flex-[999_1_560px] px-4 pt-7 pb-12 sm:px-10">
        <div className="mx-auto flex max-w-[1160px] flex-col gap-6">{children}</div>
      </main>
    </div>
  );
}

export function PageHeader({
  title,
  description,
  actions,
}: {
  title: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="flex flex-wrap items-end justify-between gap-4">
      <div className="flex flex-col gap-1">
        <h1 className="text-2xl font-semibold tracking-tight">{title}</h1>
        {description && <p className="text-muted-foreground">{description}</p>}
      </div>
      {actions && <div className="flex flex-wrap gap-2">{actions}</div>}
    </div>
  );
}
