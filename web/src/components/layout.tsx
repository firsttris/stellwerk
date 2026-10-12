import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Link, useNavigate } from "@tanstack/react-router";
import {
  Folder,
  Languages,
  LayoutGrid,
  LogOut,
  Monitor,
  RefreshCw,
  TriangleAlert,
} from "lucide-react";
import type { ReactNode } from "react";
import { api } from "../api";
import { useLiveUpdates } from "../api/live";
import { overviewQuery, sessionQuery, statusQuery } from "../api/queries";
import { m } from "../paraglide/messages.js";
import { getLocale, type Locale, setLocale } from "../paraglide/runtime.js";
import { Button } from "./ui/button";

const navLink =
  "flex min-h-9 items-center gap-2.5 rounded-md px-2.5 py-2 text-zinc-700 hover:bg-muted dark:text-zinc-300 [&_svg]:size-4";
const navActive = "bg-muted font-medium text-foreground";

function hostOf(address: string): string {
  return address.replace(/^\w+:\/\//, "").replace(/:\d+$/, "");
}

export function Layout({ children }: { children: ReactNode }) {
  const { data: overview } = useQuery(overviewQuery);
  const { data: live } = useQuery(statusQuery);
  const conflicts = Object.values(live?.conflicts ?? {}).reduce(
    (sum, list) => sum + list.length,
    0,
  );
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  useLiveUpdates();

  // Out in any case: a failed logout (session already gone, server down) must not keep the
  // admin on a page that no longer works.
  const logout = async () => {
    try {
      await api.logout();
    } finally {
      queryClient.removeQueries({ queryKey: sessionQuery.queryKey });
      await navigate({ to: "/login" });
    }
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
        <nav aria-label={m.nav_label()} className="flex flex-col gap-0.5">
          <Link
            to="/"
            className={navLink}
            activeProps={{ className: navActive }}
            activeOptions={{ exact: true }}
          >
            <LayoutGrid aria-hidden="true" />
            {m.nav_overview()}
          </Link>
          <Link to="/folders" className={navLink} activeProps={{ className: navActive }}>
            <Folder aria-hidden="true" />
            {m.nav_folders()}
            <span className="ml-auto text-xs text-muted-foreground">
              {overview?.folders.length}
            </span>
          </Link>
          <Link to="/devices" className={navLink} activeProps={{ className: navActive }}>
            <Monitor aria-hidden="true" />
            {m.nav_devices()}
            <span className="ml-auto text-xs text-muted-foreground">
              {overview?.devices.length}
            </span>
          </Link>
          <Link to="/conflicts" className={navLink} activeProps={{ className: navActive }}>
            <TriangleAlert aria-hidden="true" />
            {m.nav_conflicts()}
            {conflicts > 0 && (
              <span className="ml-auto rounded-full bg-amber-700 px-[7px] text-xs leading-[18px] font-medium text-white">
                {conflicts}
              </span>
            )}
          </Link>
        </nav>
        <div className="mt-auto flex flex-col gap-3">
          <div className="flex flex-col gap-1 rounded-lg border border-border bg-background p-3">
            <div className="flex items-center gap-2 font-medium">
              <span className="size-2 rounded-full bg-ok" />
              {m.server_online()}
            </div>
            <span className="text-xs text-muted-foreground">
              {m.server_revision({ revision: overview?.revision ?? "…" })}
            </span>
          </div>
          <label className="flex items-center gap-2.5 px-2.5 text-sm text-zinc-700 dark:text-zinc-300">
            <Languages className="size-4" aria-hidden="true" />
            <span className="sr-only">{m.language()}</span>
            <select
              aria-label={m.language()}
              value={getLocale()}
              onChange={(e) => setLocale(e.target.value as Locale)}
              className="h-9 flex-1 rounded-md border border-border bg-background px-2"
            >
              <option value="de">Deutsch</option>
              <option value="en">English</option>
            </select>
          </label>
          <Button variant="ghost" className="justify-start" onClick={logout}>
            <LogOut aria-hidden="true" />
            {m.logout()}
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
