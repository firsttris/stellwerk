import {
  type Api,
  ApiError,
  type ApiToken,
  type Device,
  type Enrollment,
  type Folder,
  type Overview,
  type Share,
} from "../api/types";
import { nowSeconds } from "../lib/format";
import { type DemoState, demoConflictContent, demoLive, demoState } from "./fixtures";
import { demoDirectory } from "./folders";

/** How many status polls until the demo's "new device" shows up in the pairing dialog. */
const POLLS_UNTIL_PAIRED = 2;

/**
 * The API in memory: logged in, seeded from the fixtures. Online devices apply every new
 * revision at once, offline ones keep waiting, as with a real Zentrale.
 */
export function demoApi(seed: DemoState = demoState(nowSeconds())): Api {
  let state = structuredClone(seed);
  const enrollments = new Map<string, number>();
  // A counter, not the list's length: that repeats after a folder other than the last is removed.
  let folderCount = state.folders.length;
  let tokenCount = state.tokens.length;

  const bump = () => {
    const revision = state.revision + 1;
    state = {
      ...state,
      revision,
      devices: state.devices.map((d) => (d.online ? { ...d, appliedRev: revision } : d)),
    };
    return revision;
  };

  const findFolder = (id: string): Folder => {
    const folder = state.folders.find((f) => f.id === id);
    if (!folder) throw new ApiError(404, "folder not found");
    return folder;
  };

  const overview = (): Overview => ({
    version: "0.1.8",
    revision: state.revision,
    hub: { syncthingId: "HUB-SYNCTHING-ID", address: "tcp://nas.local:22001" },
    devices: state.devices,
    folders: state.folders,
    shares: state.shares,
  });

  const newDevice = (
    name = "workshop-pc",
    os = "linux",
    fingerprint = "5E21-D0A7-88C3-41F9",
  ): Device => ({
    id: `${name}-${state.devices.length}`,
    name,
    kind: "desktop",
    os,
    syncthingId: `${name.toUpperCase()}-SYNCTHING-ID`,
    agentVersion: "0.1.0",
    syncthingVersion: "v2.1.6",
    approved: true,
    createdAt: nowSeconds(),
    lastSeen: nowSeconds(),
    appliedRev: state.revision + 1,
    online: true,
    onlineSince: nowSeconds(),
    fingerprint,
  });

  const takeWaiting = (fingerprint: string) => {
    const waiting = state.waiting.find((w) => w.fingerprint === fingerprint);
    if (!waiting) throw new ApiError(404, "this device is no longer waiting");
    state = { ...state, waiting: state.waiting.filter((w) => w !== waiting) };
    return waiting;
  };

  return {
    session: async () => ({ loggedIn: true, passwordSet: true, csrfToken: "demo" }),
    login: async () => ({ loggedIn: true, passwordSet: true, csrfToken: "demo" }),
    logout: async () => {},
    overview: async () => structuredClone(overview()),
    createFolder: async (label) => {
      if (!label.trim()) throw new ApiError(400, "a folder needs a name");
      folderCount++;
      const folder: Folder = {
        id: `folder-${folderCount}`,
        label: label.trim(),
        syncthingFolderId: `demo${folderCount}-xxxxx`,
        versioning: { type: "staggered", max_age_days: 365 },
        ignorePatterns: [],
        createdAt: nowSeconds(),
      };
      state = { ...state, folders: [...state.folders, folder] };
      bump();
      return folder;
    },
    updateFolder: async (id, patch) => {
      const folder = findFolder(id);
      const updated = {
        ...folder,
        label: patch.label?.trim() || folder.label,
        ignorePatterns:
          patch.ignorePatterns?.map((l) => l.trimEnd()).filter(Boolean) ?? folder.ignorePatterns,
      };
      state = { ...state, folders: state.folders.map((f) => (f.id === id ? updated : f)) };
      bump();
    },
    deleteFolder: async (id) => {
      findFolder(id);
      state = {
        ...state,
        folders: state.folders.filter((f) => f.id !== id),
        shares: state.shares.filter((s) => s.folderId !== id),
      };
      bump();
    },
    applyShares: async (changes) => {
      let shares: Share[] = state.shares;
      for (const change of changes) {
        const folder = findFolder(change.folderId);
        const current = shares.find(
          (s) => s.folderId === change.folderId && s.deviceId === change.deviceId,
        );
        shares = shares.filter((s) => s !== current);
        if (change.mode) {
          const path = change.path?.trim() || current?.path || `~/Stellwerk/${folder.label}`;
          shares = [...shares, { ...change, mode: change.mode, path }];
        }
      }
      state = { ...state, shares };
      return { revision: changes.length > 0 ? bump() : state.revision };
    },
    updateDevice: async (id, patch) => {
      const device = state.devices.find((d) => d.id === id);
      if (!device) throw new ApiError(404, "device not found");
      const updated = { ...device, ...patch };
      state = { ...state, devices: state.devices.map((d) => (d.id === id ? updated : d)) };
      if (patch.approved !== undefined) bump();
      return updated;
    },
    deleteDevice: async (id) => {
      state = {
        ...state,
        devices: state.devices.filter((d) => d.id !== id),
        shares: state.shares.filter((s) => s.deviceId !== id),
      };
      bump();
    },
    browse: async (deviceId, path) => {
      const device = state.devices.find((d) => d.id === deviceId);
      if (!device) throw new ApiError(404, "device not found");
      if (!device.online) throw new ApiError(409, "the device is offline");
      return demoDirectory(device.os, path);
    },
    createEnrollment: async (): Promise<Enrollment> => {
      const code = "K7Q2-MX9P-41TZ";
      enrollments.set(code, 0);
      return { code, expiresAt: nowSeconds() + 15 * 60, version: "0.1.3", caSha256: null };
    },
    enrollmentStatus: async (code) => {
      const polls = (enrollments.get(code) ?? 0) + 1;
      enrollments.set(code, polls);
      if (polls < POLLS_UNTIL_PAIRED) return { device: null };
      const existing = state.devices.find((d) => d.name === "workshop-pc");
      if (existing) return { device: existing };
      const device = newDevice();
      state = { ...state, devices: [...state.devices, device] };
      bump();
      return { device };
    },
    approveWaiting: async (fingerprint) => {
      const waiting = takeWaiting(fingerprint);
      const device = newDevice(waiting.name, waiting.os, waiting.fingerprint);
      state = { ...state, devices: [...state.devices, device] };
      bump();
    },
    rejectWaiting: async (fingerprint) => {
      takeWaiting(fingerprint);
    },
    events: async (limit = 20) => state.events.slice(0, limit),
    status: async () => ({
      ...demoLive(nowSeconds()),
      conflicts: structuredClone(state.conflicts),
      waiting: structuredClone(state.waiting),
    }),
    conflictContent: async (folderId, path) => {
      if (!state.conflicts[folderId]?.some((c) => c.path === path)) {
        throw new ApiError(404, "conflict copy not found");
      }
      return demoConflictContent(nowSeconds());
    },
    resolveConflict: async (folderId, path) => {
      const remaining = (state.conflicts[folderId] ?? []).filter((c) => c.path !== path);
      state = { ...state, conflicts: { ...state.conflicts, [folderId]: remaining } };
    },
    versions: async (folderId) => {
      findFolder(folderId);
      return structuredClone(state.versions[folderId] ?? []);
    },
    restoreVersion: async (folderId, path) => {
      findFolder(folderId);
      if (!state.versions[folderId]?.some((f) => f.path === path)) {
        throw new ApiError(400, "no such version");
      }
    },
    tokens: async () => structuredClone(state.tokens),
    createToken: async (name, scope) => {
      if (!name.trim()) throw new ApiError(400, "a token needs a name");
      tokenCount++;
      const token: ApiToken = {
        id: `token-${tokenCount}`,
        name: name.trim(),
        scope,
        createdAt: nowSeconds(),
        lastUsedAt: null,
      };
      state = { ...state, tokens: [token, ...state.tokens] };
      return { ...token, secret: `stw_demo${tokenCount}xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx` };
    },
    deleteToken: async (id) => {
      if (!state.tokens.some((t) => t.id === id)) throw new ApiError(404, "token not found");
      state = { ...state, tokens: state.tokens.filter((t) => t.id !== id) };
    },
  };
}
