import type { ActivityEvent, Device, Folder, FolderView, Live, Share } from "../api/types";

// Demo data for `vite --mode demo`, the E2E tests and the screenshots. Times are relative to
// `now`, so "offline for 3 days" stays true.

const DAY = 24 * 60 * 60;

export type DemoState = {
  revision: number;
  devices: Device[];
  folders: Folder[];
  shares: Share[];
  events: ActivityEvent[];
};

const device = (now: number, d: Partial<Device> & Pick<Device, "id" | "name">): Device => ({
  kind: "desktop",
  os: "linux",
  syncthingId: `${d.id.toUpperCase()}-SYNCTHING-ID`,
  agentVersion: "0.1.0",
  syncthingVersion: "v2.1.6",
  approved: true,
  createdAt: now - 30 * DAY,
  lastSeen: now,
  appliedRev: 12,
  online: true,
  onlineSince: now - 2 * 60 * 60,
  fingerprint: "7F3A-91C2",
  ...d,
});

const folder = (
  now: number,
  id: string,
  label: string,
  syncthingFolderId: string,
  ignorePatterns: string[] = [],
): Folder => ({
  id,
  label,
  syncthingFolderId,
  versioning: { type: "staggered", max_age_days: 365 },
  ignorePatterns,
  createdAt: now - 20 * DAY,
});

export function demoState(now: number): DemoState {
  const devices = [
    device(now, { id: "werkbank", name: "werkbank", os: "linux", fingerprint: "7F3A-91C2" }),
    device(now, {
      id: "laptop",
      name: "laptop",
      kind: "laptop",
      os: "windows",
      fingerprint: "A04B-17EE",
    }),
    device(now, {
      id: "buero",
      name: "büro-pc",
      os: "windows",
      online: false,
      onlineSince: null,
      lastSeen: now - 3 * DAY,
      appliedRev: 11,
      fingerprint: "C3D9-5520",
    }),
    device(now, {
      id: "macbook",
      name: "macbook",
      kind: "laptop",
      os: "macos",
      fingerprint: "11F0-8B3A",
    }),
  ];
  const folders = [
    folder(now, "projekte", "Projekte", "k3j9x-2mwqa", [
      "node_modules/",
      "dist/",
      "target/",
      "*.log",
      ".DS_Store",
    ]),
    folder(now, "dokumente", "Dokumente", "p8rte-4nq2c"),
    folder(now, "fotos", "Fotos", "b7wz4-hh3ke"),
    folder(now, "musik", "Musik", "m2xc9-q7tpa"),
  ];
  const shares: Share[] = [
    { folderId: "projekte", deviceId: "werkbank", mode: "sendReceive", path: "~/dev" },
    {
      folderId: "projekte",
      deviceId: "laptop",
      mode: "sendReceive",
      path: "C:\\Users\\tristan\\dev",
    },
    { folderId: "projekte", deviceId: "buero", mode: "sendReceive", path: "D:\\Projekte" },
    { folderId: "dokumente", deviceId: "werkbank", mode: "sendReceive", path: "~/Dokumente" },
    {
      folderId: "dokumente",
      deviceId: "laptop",
      mode: "sendReceive",
      path: "C:\\Users\\tristan\\Documents",
    },
    { folderId: "dokumente", deviceId: "buero", mode: "receiveOnly", path: "D:\\Dokumente" },
    { folderId: "dokumente", deviceId: "macbook", mode: "receiveOnly", path: "~/Documents" },
    { folderId: "fotos", deviceId: "werkbank", mode: "receiveOnly", path: "~/Bilder" },
    { folderId: "fotos", deviceId: "macbook", mode: "sendOnly", path: "~/Pictures" },
    { folderId: "musik", deviceId: "werkbank", mode: "sendReceive", path: "~/Musik" },
    {
      folderId: "musik",
      deviceId: "laptop",
      mode: "receiveOnly",
      path: "C:\\Users\\tristan\\Music",
    },
  ];
  const events: ActivityEvent[] = [
    {
      id: 6,
      at: now - 2 * 60,
      deviceId: "werkbank",
      folderId: "projekte",
      kind: "changes",
      text: "18",
    },
    {
      id: 5,
      at: now - 9 * 60,
      deviceId: "macbook",
      folderId: "fotos",
      kind: "changes",
      text: "46",
    },
    { id: 4, at: now - 18 * 60, deviceId: "macbook", folderId: null, kind: "online", text: "" },
    {
      id: 3,
      at: now - 2 * 60 * 60,
      deviceId: "laptop",
      folderId: null,
      kind: "error",
      text: "Folder b7wz4-hh3ke: path is not writable",
    },
    { id: 2, at: now - 9 * DAY, deviceId: "macbook", folderId: null, kind: "paired", text: "" },
    { id: 1, at: now - 30 * DAY, deviceId: "werkbank", folderId: null, kind: "paired", text: "" },
  ];
  return { revision: 12, devices, folders, shares, events };
}

const folderView = (globalFiles: number, globalBytes: number, extra: Partial<FolderView> = {}) =>
  ({
    state: "idle",
    globalFiles,
    globalBytes,
    needFiles: 0,
    needBytes: 0,
    errors: 0,
    ...extra,
  }) satisfies FolderView;

/** Live state to go with [`demoState`]: Fotos is arriving on werkbank, büro-pc is behind. */
export function demoLive(now: number): Live {
  const done = { percent: 100, needBytes: 0, needItems: 0 };
  return {
    devices: {
      werkbank: {
        at: now,
        folders: {
          projekte: folderView(48_210, 18_400_000_000),
          dokumente: folderView(3_912, 2_100_000_000),
          fotos: folderView(61_004, 412_000_000_000, {
            state: "syncing",
            needFiles: 18,
            needBytes: 115_000_000_000,
          }),
          musik: folderView(9_880, 96_000_000_000),
        },
      },
      laptop: {
        at: now,
        folders: {
          projekte: folderView(48_210, 18_400_000_000),
          dokumente: folderView(3_912, 2_100_000_000, { errors: 2 }),
          musik: folderView(9_880, 96_000_000_000),
        },
      },
      macbook: {
        at: now,
        folders: {
          dokumente: folderView(3_912, 2_100_000_000),
          fotos: folderView(61_004, 412_000_000_000),
        },
      },
    },
    hub: {
      folders: {
        projekte: folderView(48_210, 18_400_000_000),
        dokumente: folderView(3_912, 2_100_000_000),
        fotos: folderView(61_004, 412_000_000_000),
        musik: folderView(9_880, 96_000_000_000),
      },
      completion: {
        projekte: {
          werkbank: done,
          laptop: done,
          buero: { percent: 96.2, needBytes: 690_000_000, needItems: 202 },
        },
        dokumente: {
          werkbank: done,
          laptop: done,
          buero: { percent: 99.1, needBytes: 18_000_000, needItems: 12 },
          macbook: done,
        },
        fotos: {
          werkbank: { percent: 72.4, needBytes: 115_000_000_000, needItems: 18 },
          macbook: done,
        },
        musik: { werkbank: done, laptop: done },
      },
    },
  };
}
