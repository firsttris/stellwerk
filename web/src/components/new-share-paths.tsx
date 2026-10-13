import type { Device, Folder } from "../api/types";
import { cellKey, type DraftPaths } from "../lib/matrix";
import { m } from "../paraglide/messages.js";
import { PathField } from "./path-field";

/** For each folder the draft puts on a device for the first time: where it goes there. */
export function NewSharePaths({
  shares,
  folders,
  devices,
  paths,
  onChange,
}: {
  shares: readonly { folderId: string; deviceId: string }[];
  folders: readonly Folder[];
  devices: readonly Device[];
  paths: DraftPaths;
  onChange: (key: string, path: string) => void;
}) {
  return (
    <div className="flex flex-col gap-3 border-b border-border px-5 py-4">
      <div className="flex flex-col gap-0.5">
        <span className="font-medium">{m.matrix_new_paths_title()}</span>
        <span className="text-[13px] text-muted-foreground">{m.matrix_new_paths_hint()}</span>
      </div>
      {shares.map(({ folderId, deviceId }) => {
        const folder = folders.find((f) => f.id === folderId);
        const device = devices.find((d) => d.id === deviceId);
        if (!folder || !device) return null;
        const key = cellKey(folderId, deviceId);
        const label = m.share_path_label({ folder: folder.label, device: device.name });
        return (
          <div
            key={key}
            className="grid gap-2 sm:grid-cols-[minmax(160px,1fr)_3fr] sm:items-center"
          >
            <span className="text-sm">{label}</span>
            <PathField
              device={device}
              label={label}
              value={paths[key] ?? ""}
              placeholder={`~/Stellwerk/${folder.label}`}
              onChange={(path) => onChange(key, path)}
            />
          </div>
        );
      })}
    </div>
  );
}
