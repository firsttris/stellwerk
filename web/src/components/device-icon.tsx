import { Database, Laptop, Monitor, Smartphone } from "lucide-react";
import type { DeviceKind } from "../api/types";

const ICONS = { desktop: Monitor, laptop: Laptop, server: Database, phone: Smartphone };

export function DeviceIcon({ kind, className }: { kind: DeviceKind; className?: string }) {
  const Icon = ICONS[kind];
  return <Icon className={className ?? "size-3.5"} aria-hidden="true" />;
}

export function StatusDot({ online }: { online: boolean }) {
  return (
    <span
      className={
        online
          ? "inline-block size-2 shrink-0 rounded-full bg-ok shadow-[0_0_0_3px_var(--ok-ring)]"
          : "inline-block size-2 shrink-0 rounded-full bg-offline"
      }
    />
  );
}
