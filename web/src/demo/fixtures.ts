import type {
  ActivityEvent,
  ApiToken,
  Conflict,
  ConflictContent,
  Device,
  FileVersions,
  Folder,
  FolderView,
  Live,
  Share,
  WaitingDevice,
} from "../api/types";

// Demo data for `vite --mode demo`, the E2E tests and the screenshots. Times are relative to
// `now`, so "offline for 3 days" stays true.

const DAY = 24 * 60 * 60;

export type DemoState = {
  revision: number;
  devices: Device[];
  folders: Folder[];
  shares: Share[];
  events: ActivityEvent[];
  /** By folder ID. */
  conflicts: Record<string, Conflict[]>;
  /** By folder ID. */
  versions: Record<string, FileVersions[]>;
  /** Agents in the LAN waiting for approval. */
  waiting: WaitingDevice[];
  tokens: ApiToken[];
};

export const DEMO_CONFLICT_PATH = "haul/src/config.sync-conflict-20261012-103112-WORKBEN.ts";

/** Both sides of the demo conflict, as in the mockup. */
export const demoConflictContent = (now: number): ConflictContent => ({
  original: {
    size: 4_100,
    modified: now - 11 * 60,
    text: "export const config = {\n  retries: 5,\n  hoster: 'ddownload',\n  proxy: true,\n};\n",
  },
  copy: {
    size: 4_000,
    modified: now - 13 * 60,
    text: "export const config = {\n  retries: 3,\n  timeoutMs: 30_000,\n  parallel: 4,\n  hoster: 'ddownload',\n};\n",
  },
});

const iso = (seconds: number) => new Date(seconds * 1000).toISOString();

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
  fingerprint: "7F3A-91C2-4E08-B6D1",
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
    device(now, {
      id: "workbench",
      name: "workbench",
      os: "linux",
      fingerprint: "7F3A-91C2-4E08-B6D1",
    }),
    device(now, {
      id: "laptop",
      name: "laptop",
      kind: "laptop",
      os: "windows",
      fingerprint: "A04B-17EE-93C5-0F72",
    }),
    device(now, {
      id: "office",
      name: "office-pc",
      os: "windows",
      online: false,
      onlineSince: null,
      lastSeen: now - 3 * DAY,
      appliedRev: 11,
      fingerprint: "C3D9-5520-1AB4-E86F",
    }),
    device(now, {
      id: "macbook",
      name: "macbook",
      kind: "laptop",
      os: "macos",
      fingerprint: "11F0-8B3A-D27C-5409",
    }),
  ];
  const folders = [
    folder(now, "projects", "Projects", "k3j9x-2mwqa", [
      "node_modules/",
      "dist/",
      "target/",
      "*.log",
      ".DS_Store",
    ]),
    folder(now, "documents", "Documents", "p8rte-4nq2c"),
    folder(now, "photos", "Photos", "b7wz4-hh3ke"),
    folder(now, "music", "Music", "m2xc9-q7tpa"),
  ];
  const shares: Share[] = [
    { folderId: "projects", deviceId: "workbench", mode: "sendReceive", path: "~/dev" },
    {
      folderId: "projects",
      deviceId: "laptop",
      mode: "sendReceive",
      path: "C:\\Users\\tristan\\dev",
    },
    { folderId: "projects", deviceId: "office", mode: "sendReceive", path: "D:\\Projects" },
    { folderId: "documents", deviceId: "workbench", mode: "sendReceive", path: "~/Documents" },
    {
      folderId: "documents",
      deviceId: "laptop",
      mode: "sendReceive",
      path: "C:\\Users\\tristan\\Documents",
    },
    { folderId: "documents", deviceId: "office", mode: "receiveOnly", path: "D:\\Documents" },
    { folderId: "documents", deviceId: "macbook", mode: "receiveOnly", path: "~/Documents" },
    { folderId: "photos", deviceId: "workbench", mode: "receiveOnly", path: "~/Pictures" },
    { folderId: "photos", deviceId: "macbook", mode: "sendOnly", path: "~/Pictures" },
    { folderId: "music", deviceId: "workbench", mode: "sendReceive", path: "~/Music" },
    {
      folderId: "music",
      deviceId: "laptop",
      mode: "receiveOnly",
      path: "C:\\Users\\tristan\\Music",
    },
  ];
  const events: ActivityEvent[] = [
    {
      id: 6,
      at: now - 2 * 60,
      deviceId: "workbench",
      folderId: "projects",
      kind: "changes",
      text: "18",
    },
    {
      id: 5,
      at: now - 9 * 60,
      deviceId: "macbook",
      folderId: "photos",
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
    { id: 1, at: now - 30 * DAY, deviceId: "workbench", folderId: null, kind: "paired", text: "" },
  ];
  const conflicts: Record<string, Conflict[]> = {
    projects: [
      {
        path: DEMO_CONFLICT_PATH,
        original: "haul/src/config.ts",
        at: now - 11 * 60,
        deviceId: "workbench",
        size: 4_000,
        modified: now - 13 * 60,
        originalSize: 4_100,
        originalModified: now - 11 * 60,
      },
    ],
  };
  const version = (ago: number, size: number) => ({
    versionTime: iso(now - ago),
    modTime: iso(now - ago - 300),
    size,
  });
  const versions: Record<string, FileVersions[]> = {
    projects: [
      {
        path: "haul/src/config.ts",
        versions: [version(2 * 3600, 4_050), version(DAY, 3_900), version(9 * DAY, 3_700)],
      },
      { path: "README.md", versions: [version(3 * DAY, 12_400)] },
    ],
    documents: [{ path: "Taxes/2025/Receipts.pdf", versions: [version(5 * DAY, 2_400_000)] }],
  };
  const waiting: WaitingDevice[] = [
    {
      fingerprint: "9C41-07AB-E2D5-3F18",
      name: "living-room-pc",
      os: "windows",
      syncthingId: "LIVINGR-SYNCTHING-ID",
      since: now - 40,
    },
  ];
  const tokens: ApiToken[] = [
    {
      id: "token-claude",
      name: "Claude Code",
      scope: "manage",
      createdAt: now - 9 * DAY,
      lastUsedAt: now - 2 * 3600,
    },
  ];
  return { revision: 12, devices, folders, shares, events, conflicts, versions, waiting, tokens };
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

/** Live state to go with [`demoState`]: Photos is arriving on workbench, office-pc is behind. */
export function demoLive(now: number): Live {
  const done = { percent: 100, needBytes: 0, needItems: 0 };
  return {
    devices: {
      workbench: {
        at: now,
        folders: {
          projects: folderView(48_210, 18_400_000_000),
          documents: folderView(3_912, 2_100_000_000),
          photos: folderView(61_004, 412_000_000_000, {
            state: "syncing",
            needFiles: 18,
            needBytes: 115_000_000_000,
          }),
          music: folderView(9_880, 96_000_000_000),
        },
      },
      laptop: {
        at: now,
        folders: {
          projects: folderView(48_210, 18_400_000_000),
          documents: folderView(3_912, 2_100_000_000, { errors: 2 }),
          music: folderView(9_880, 96_000_000_000),
        },
      },
      macbook: {
        at: now,
        folders: {
          documents: folderView(3_912, 2_100_000_000),
          photos: folderView(61_004, 412_000_000_000),
        },
      },
    },
    hub: {
      folders: {
        projects: folderView(48_210, 18_400_000_000),
        documents: folderView(3_912, 2_100_000_000),
        photos: folderView(61_004, 412_000_000_000),
        music: folderView(9_880, 96_000_000_000),
      },
      completion: {
        projects: {
          workbench: done,
          laptop: done,
          office: { percent: 96.2, needBytes: 690_000_000, needItems: 202 },
        },
        documents: {
          workbench: done,
          laptop: done,
          office: { percent: 99.1, needBytes: 18_000_000, needItems: 12 },
          macbook: done,
        },
        photos: {
          workbench: { percent: 72.4, needBytes: 115_000_000_000, needItems: 18 },
          macbook: done,
        },
        music: { workbench: done, laptop: done },
      },
    },
    conflicts: {},
    waiting: [],
  };
}
