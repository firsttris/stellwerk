import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

export function Card({ className, ...props }: ComponentProps<"section">) {
  return <section className={cn("rounded-xl border border-border", className)} {...props} />;
}

export function Badge({
  className,
  tone = "neutral",
  ...props
}: ComponentProps<"span"> & { tone?: "neutral" | "warn" | "info" | "ok" }) {
  const tones = {
    neutral: "border-border bg-muted text-muted-foreground",
    warn: "border-warn-border bg-warn text-warn-foreground",
    info: "border-info-border bg-info text-info-foreground",
    ok: "border-ok-border bg-ok-surface text-ok-foreground",
  };
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border px-2 py-px text-[11px] font-medium",
        tones[tone],
        className,
      )}
      {...props}
    />
  );
}
