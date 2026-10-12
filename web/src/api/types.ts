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
  revision: z.number(),
  hub: z.object({ syncthingId: z.string(), address: z.string() }),
  devices: z.array(deviceSchema),
  folders: z.array(folderSchema),
  shares: z.array(shareSchema),
});
export type Overview = z.infer<typeof overviewSchema>;

export const sessionSchema = z.object({
  loggedIn: z.boolean(),
  passwordSet: z.boolean(),
  csrfToken: z.string().optional(),
});
export type Session = z.infer<typeof sessionSchema>;

export const enrollmentSchema = z.object({ code: z.string(), expiresAt: z.number() });
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

/** One cell of the matrix as the API takes it; `mode: null` is "off". */
export type ShareChange = {
  folderId: string;
  deviceId: string;
  mode: Mode | null;
  path?: string;
};

export type DevicePatch = { name?: string; kind?: DeviceKind; approved?: boolean };

export type Api = {
  session(): Promise<Session>;
  login(password: string): Promise<Session>;
  logout(): Promise<void>;
  overview(): Promise<Overview>;
  createFolder(label: string): Promise<Folder>;
  renameFolder(id: string, label: string): Promise<void>;
  deleteFolder(id: string): Promise<void>;
  applyShares(changes: ShareChange[]): Promise<{ revision: number }>;
  updateDevice(id: string, patch: DevicePatch): Promise<Device>;
  deleteDevice(id: string): Promise<void>;
  createEnrollment(): Promise<Enrollment>;
  enrollmentStatus(code: string): Promise<{ device: Device | null }>;
  events(limit?: number): Promise<ActivityEvent[]>;
};

export class ApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
  }
}
