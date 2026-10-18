import { z } from "zod";

// The Zentrale's JSON, checked at the boundary (crates/zentrale/src/api).

export const modeSchema = z.enum(["sendReceive", "sendOnly", "receiveOnly"]);
export type Mode = z.infer<typeof modeSchema>;

export const deviceKindSchema = z.enum(["desktop", "laptop", "server", "phone"]);
export type DeviceKind = z.infer<typeof deviceKindSchema>;

export const deviceSchema = z.object({
  id: z.string(),
  name: z.string(),
  kind: deviceKindSchema,
  os: z.string(),
  syncthingId: z.string(),
  agentVersion: z.string(),
  syncthingVersion: z.string(),
  approved: z.boolean(),
  createdAt: z.number(),
  lastSeen: z.number().nullable(),
  appliedRev: z.number().nullable(),
  online: z.boolean(),
  onlineSince: z.number().nullable(),
  fingerprint: z.string(),
});
export type Device = z.infer<typeof deviceSchema>;

export const versioningSchema = z.discriminatedUnion("type", [
  z.object({ type: z.literal("none") }),
  z.object({ type: z.literal("staggered"), max_age_days: z.number() }),
]);

export const folderSchema = z.object({
  id: z.string(),
  label: z.string(),
  syncthingFolderId: z.string(),
  versioning: versioningSchema,
  ignorePatterns: z.array(z.string()),
  createdAt: z.number(),
});
export type Folder = z.infer<typeof folderSchema>;

export const shareSchema = z.object({
  folderId: z.string(),
  deviceId: z.string(),
  mode: modeSchema,
  path: z.string(),
});
export type Share = z.infer<typeof shareSchema>;

export const overviewSchema = z.object({
  /** The Zentrale's release. */
  version: z.string(),
  revision: z.number(),
  hub: z.object({ syncthingId: z.string(), address: z.string() }),
  devices: z.array(deviceSchema),
  folders: z.array(folderSchema),
  shares: z.array(shareSchema),
});
export type Overview = z.infer<typeof overviewSchema>;

/** The answer to a change of shares: the revision it became. */
export const revisionSchema = z.object({ revision: z.number() });

export const sessionSchema = z.object({
  loggedIn: z.boolean(),
  passwordSet: z.boolean(),
  csrfToken: z.string().optional(),
});
export type Session = z.infer<typeof sessionSchema>;

export const enrollmentSchema = z.object({
  code: z.string(),
  expiresAt: z.number(),
  /** The Zentrale's version: the pairing command fetches that release's agent. */
  version: z.string(),
  /** SHA-256 of the private CA devices are told to trust, if the Zentrale has one. */
  caSha256: z.string().nullable(),
});
export type Enrollment = z.infer<typeof enrollmentSchema>;

export const enrollmentStatusSchema = z.object({ device: deviceSchema.nullable() });

export const eventSchema = z.object({
  id: z.number(),
  at: z.number(),
  deviceId: z.string().nullable(),
  folderId: z.string().nullable(),
  kind: z.string(),
  text: z.string(),
});
export type ActivityEvent = z.infer<typeof eventSchema>;

export const syncStateSchema = z.enum([
  "idle",
  "scanning",
  "syncing",
  "waiting",
  "error",
  "unknown",
]);
export type SyncState = z.infer<typeof syncStateSchema>;

export const folderViewSchema = z.object({
  state: syncStateSchema,
  globalFiles: z.number(),
  globalBytes: z.number(),
  needFiles: z.number(),
  needBytes: z.number(),
  errors: z.number(),
  error: z.string().optional(),
});
export type FolderView = z.infer<typeof folderViewSchema>;

export const completionSchema = z.object({
  percent: z.number(),
  needBytes: z.number(),
  needItems: z.number(),
});
export type Completion = z.infer<typeof completionSchema>;

export const conflictSchema = z.object({
  path: z.string(),
  original: z.string(),
  at: z.number(),
  deviceId: z.string().nullable(),
  size: z.number(),
  modified: z.number(),
  originalSize: z.number().nullable(),
  originalModified: z.number().nullable(),
});
export type Conflict = z.infer<typeof conflictSchema>;

/** An agent without a code waiting for approval (crates/zentrale/src/waiting.rs). */
export const waitingSchema = z.object({
  fingerprint: z.string(),
  name: z.string(),
  os: z.string(),
  syncthingId: z.string(),
  since: z.number(),
});
export type WaitingDevice = z.infer<typeof waitingSchema>;

/** What is happening right now (crates/zentrale/src/live.rs), by the Zentrale's IDs. */
export const liveSchema = z.object({
  devices: z.record(
    z.string(),
    z.object({ at: z.number(), folders: z.record(z.string(), folderViewSchema) }),
  ),
  hub: z.object({
    folders: z.record(z.string(), folderViewSchema),
    completion: z.record(z.string(), z.record(z.string(), completionSchema)),
  }),
  conflicts: z.record(z.string(), z.array(conflictSchema)),
  waiting: z.array(waitingSchema).default([]),
});
export type Live = z.infer<typeof liveSchema>;

export const uiMessageSchema = z.object({
  type: z.literal("update"),
  revision: z.number(),
  online: z.array(z.string()),
  status: liveSchema,
});
export type UiMessage = z.infer<typeof uiMessageSchema>;

const fileContentSchema = z.object({
  size: z.number(),
  modified: z.number(),
  text: z.string().nullable(),
});
export type FileContent = z.infer<typeof fileContentSchema>;

export const conflictContentSchema = z.object({
  original: fileContentSchema.nullable(),
  copy: fileContentSchema,
});
export type ConflictContent = z.infer<typeof conflictContentSchema>;

export const fileVersionsSchema = z.object({
  path: z.string(),
  versions: z.array(z.object({ versionTime: z.string(), modTime: z.string(), size: z.number() })),
});
export type FileVersions = z.infer<typeof fileVersionsSchema>;

/** A directory on a device, for the folder picker; the agent lists it on request. */
export const directorySchema = z.object({
  path: z.string(),
  parent: z.string().nullable(),
  dirs: z.array(z.object({ name: z.string(), path: z.string() })),
  home: z.string().nullable(),
  roots: z.array(z.string()),
});
export type Directory = z.infer<typeof directorySchema>;

export const tokenScopeSchema = z.enum(["read", "manage"]);
export type TokenScope = z.infer<typeof tokenScopeSchema>;

/** An API token for scripts and AI assistants; the token itself is never listed. */
export const apiTokenSchema = z.object({
  id: z.string(),
  name: z.string(),
  scope: tokenScopeSchema,
  createdAt: z.number(),
  lastUsedAt: z.number().nullable(),
});
export type ApiToken = z.infer<typeof apiTokenSchema>;

/** A new token with its secret, shown this once. */
export const createdTokenSchema = apiTokenSchema.extend({ secret: z.string() });
export type CreatedToken = z.infer<typeof createdTokenSchema>;

export type Keep = "original" | "copy" | "both";

/** One cell of the matrix as the API takes it; `mode: null` is "off". */
export type ShareChange = {
  folderId: string;
  deviceId: string;
  mode: Mode | null;
  path?: string;
};

export type FolderPatch = { label?: string; ignorePatterns?: string[] };

export type DevicePatch = { name?: string; kind?: DeviceKind; approved?: boolean };

export type Api = {
  session(): Promise<Session>;
  login(password: string): Promise<Session>;
  logout(): Promise<void>;
  overview(): Promise<Overview>;
  createFolder(label: string): Promise<Folder>;
  updateFolder(id: string, patch: FolderPatch): Promise<void>;
  deleteFolder(id: string): Promise<void>;
  applyShares(changes: ShareChange[]): Promise<{ revision: number }>;
  updateDevice(id: string, patch: DevicePatch): Promise<Device>;
  deleteDevice(id: string): Promise<void>;
  /** The directory at `path` on the device; its home directory for an empty path. */
  browse(deviceId: string, path: string): Promise<Directory>;
  createEnrollment(): Promise<Enrollment>;
  enrollmentStatus(code: string): Promise<{ device: Device | null }>;
  approveWaiting(fingerprint: string): Promise<void>;
  rejectWaiting(fingerprint: string): Promise<void>;
  events(limit?: number): Promise<ActivityEvent[]>;
  status(): Promise<Live>;
  conflictContent(folderId: string, path: string): Promise<ConflictContent>;
  resolveConflict(folderId: string, path: string, keep: Keep): Promise<void>;
  versions(folderId: string): Promise<FileVersions[]>;
  restoreVersion(folderId: string, path: string, versionTime: string): Promise<void>;
  tokens(): Promise<ApiToken[]>;
  createToken(name: string, scope: TokenScope): Promise<CreatedToken>;
  deleteToken(id: string): Promise<void>;
};

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}
