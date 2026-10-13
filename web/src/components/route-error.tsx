import { type ErrorComponentProps, Link } from "@tanstack/react-router";
import { TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { errorMessage } from "../lib/errors";
import { m } from "../paraglide/messages.js";
import { Button } from "./ui/button";

function Notice({ title, children }: { title: string; children: ReactNode }) {
  return (
    <main className="grid min-h-screen place-items-center bg-subtle px-4">
      <div className="flex w-full max-w-md flex-col gap-4 rounded-xl border border-border bg-background p-6">
        <div className="flex items-center gap-2.5">
          <TriangleAlert className="size-5 text-warn-foreground" aria-hidden="true" />
          <h1 className="text-lg font-semibold">{title}</h1>
        </div>
        {children}
      </div>
    </main>
  );
}

/** A page that failed to load, e.g. because the Zentrale is not reachable. */
export function RouteError({ error }: ErrorComponentProps) {
  return (
    <Notice title={m.error_page_title()}>
      <p className="text-sm text-danger">
        {error instanceof Error ? errorMessage(error) : String(error)}
      </p>
      <Button className="self-start" onClick={() => window.location.reload()}>
        {m.error_page_reload()}
      </Button>
    </Notice>
  );
}

/** An address the UI does not know. */
export function NotFound() {
  return (
    <Notice title={m.not_found_title()}>
      <Link to="/" className="text-sm underline">
        {m.not_found_back()}
      </Link>
    </Notice>
  );
}
