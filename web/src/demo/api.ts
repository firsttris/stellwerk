import {
  type Api,
  ApiError,
  type Device,
  type Enrollment,
  type Folder,
  type Overview,
  type Share,
} from "../api/types";
import { type DemoState, demoState } from "./fixtures";

const nowSeconds = () => Math.floor(Date.now() / 1000);

/** How many status polls until the demo's "new device" shows up in the pairing dialog. */
const POLLS_UNTIL_PAIRED = 2;

/**
 * The API in memory: logged in, seeded from the fixtures. Online devices apply every new
 * revision at once, offline ones keep waiting, as with a real Zentrale.
 */
export function demoApi(seed: DemoState = demoState(nowSeconds())): Api {
  let state = structuredClone(seed);
  const enrollments = new Map<string, number>();

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
    revision: state.revision,
    hub: { syncthingId: "HUB-SYNCTHING-ID", address: "tcp://nas.local:22001" },
    devices: state.devices,
    folders: state.folders,
    shares: state.shares,
  });

  const newDevice = (): Device => ({
    id: `werkstatt-${state.devices.length}`,
    name: "werkstatt-pc",
    kind: "desktop",
    os: "linux",
    syncthingId: "WERKSTATT-SYNCTHING-ID",
    agentVersion: "0.1.0",
    syncthingVersion: "v2.1.6",
    approved: true,
    createdAt: nowSeconds(),
    lastSeen: nowSeconds(),
    appliedRev: state.revision + 1,
    online: true,
    onlineSince: nowSeconds(),
    fingerprint: "5E21-D0A7",
  });

  return {
    session: async () => ({ loggedIn: true, passwordSet: true, csrfToken: "demo" }),
    login: async () => ({ loggedIn: true, passwordSet: true, csrfToken: "demo" }),
    logout: async () => {},
    overview: async () => structuredClone(overview()),
    createFolder: async (label) => {
      if (!label.trim()) throw new ApiError(400, "a folder needs a name");
      const folder: Folder = {
        id: `folder-${state.folders.length + 1}`,
        label: label.trim(),
        syncthingFolderId: `demo${state.folders.length}-xxxxx`,
        versioning: { type: "staggered", max_age_days: 365 },
        createdAt: nowSeconds(),
      };
      state = { ...state, folders: [...state.folders, folder] };
      bump();
      return folder;
    },
    renameFolder: async (id, label) => {
      findFolder(id);
      state = {
        ...state,
        folders: state.folders.map((f) => (f.id === id ? { ...f, label: label.trim() } : f)),
      };
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
    createEnrollment: async (): Promise<Enrollment> => {
      const code = "K7Q2-MX9P-41TZ";
      enrollments.set(code, 0);
      return { code, expiresAt: nowSeconds() + 15 * 60 };
    },
    enrollmentStatus: async (code) => {
      const polls = (enrollments.get(code) ?? 0) + 1;
      enrollments.set(code, polls);
      if (polls < POLLS_UNTIL_PAIRED) return { device: null };
      const existing = state.devices.find((d) => d.name === "werkstatt-pc");
      if (existing) return { device: existing };
      const device = newDevice();
      state = { ...state, devices: [...state.devices, device] };
      bump();
      return { device };
    },
    events: async (limit = 20) => state.events.slice(0, limit),
  };
}
