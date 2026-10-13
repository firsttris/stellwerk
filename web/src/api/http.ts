import type { z } from "zod";
import {
  type Api,
  ApiError,
  conflictContentSchema,
  deviceSchema,
  directorySchema,
  enrollmentSchema,
  enrollmentStatusSchema,
  eventSchema,
  fileVersionsSchema,
  folderSchema,
  liveSchema,
  overviewSchema,
  revisionSchema,
  type Session,
  sessionSchema,
} from "./types";

/**
 * The body as JSON, or `undefined` when it is not JSON: a proxy's HTML error page, a plain-text
 * 404. The status decides what happened, not whether the body parses.
 */
function parseJson(text: string): unknown {
  if (!text) return null;
  try {
    return JSON.parse(text);
  } catch {
    return undefined;
  }
}

/** The Zentrale's `{"error": "…"}`, if that is what came back. */
function errorText(json: unknown): string | undefined {
  return json && typeof json === "object" && "error" in json && typeof json.error === "string"
    ? json.error
    : undefined;
}

/** The real Zentrale. Remembers the CSRF token from the session for every change. */
export function httpApi(): Api {
  let csrfToken: string | undefined;

  const request = async (method: string, path: string, body?: unknown): Promise<unknown> => {
    const headers: Record<string, string> = {};
    if (body !== undefined) headers["Content-Type"] = "application/json";
    if (method !== "GET" && csrfToken) headers["X-CSRF-Token"] = csrfToken;
    const response = await fetch(path, {
      method,
      headers,
      credentials: "same-origin",
      body: body === undefined ? undefined : JSON.stringify(body),
    });
    const json = parseJson(await response.text());
    if (!response.ok) throw new ApiError(response.status, errorText(json) ?? response.statusText);
    return json;
  };

  const parsed =
    <T>(schema: z.ZodType<T>) =>
    (json: unknown): T =>
      schema.parse(json);

  const remember = (session: Session): Session => {
    csrfToken = session.csrfToken;
    return session;
  };

  return {
    session: () => request("GET", "/api/session").then(parsed(sessionSchema)).then(remember),
    login: (password) =>
      request("POST", "/api/login", { password }).then(parsed(sessionSchema)).then(remember),
    logout: async () => {
      await request("POST", "/api/logout");
      csrfToken = undefined;
    },
    overview: () => request("GET", "/api/overview").then(parsed(overviewSchema)),
    createFolder: (label) => request("POST", "/api/folders", { label }).then(parsed(folderSchema)),
    updateFolder: async (id, patch) => {
      await request("PATCH", `/api/folders/${encodeURIComponent(id)}`, patch);
    },
    deleteFolder: async (id) => {
      await request("DELETE", `/api/folders/${encodeURIComponent(id)}`);
    },
    applyShares: (changes) =>
      request("POST", "/api/shares", { changes }).then(parsed(revisionSchema)),
    updateDevice: (id, patch) =>
      request("PATCH", `/api/devices/${encodeURIComponent(id)}`, patch).then(parsed(deviceSchema)),
    deleteDevice: async (id) => {
      await request("DELETE", `/api/devices/${encodeURIComponent(id)}`);
    },
    browse: (deviceId, path) =>
      request(
        "GET",
        `/api/devices/${encodeURIComponent(deviceId)}/browse?path=${encodeURIComponent(path)}`,
      ).then(parsed(directorySchema)),
    createEnrollment: () => request("POST", "/api/enrollments").then(parsed(enrollmentSchema)),
    enrollmentStatus: (code) =>
      request("POST", "/api/enrollments/status", { code }).then(parsed(enrollmentStatusSchema)),
    approveWaiting: async (fingerprint) => {
      await request("POST", `/api/waiting/${encodeURIComponent(fingerprint)}/approve`);
    },
    rejectWaiting: async (fingerprint) => {
      await request("POST", `/api/waiting/${encodeURIComponent(fingerprint)}/reject`);
    },
    status: () => request("GET", "/api/status").then(parsed(liveSchema)),
    conflictContent: (folderId, path) =>
      request(
        "GET",
        `/api/folders/${encodeURIComponent(folderId)}/conflicts/content?path=${encodeURIComponent(path)}`,
      ).then(parsed(conflictContentSchema)),
    resolveConflict: async (folderId, path, keep) => {
      await request("POST", `/api/folders/${encodeURIComponent(folderId)}/conflicts/resolve`, {
        path,
        keep,
      });
    },
    versions: (folderId) =>
      request("GET", `/api/folders/${encodeURIComponent(folderId)}/versions`).then(
        parsed(fileVersionsSchema.array()),
      ),
    restoreVersion: async (folderId, path, versionTime) => {
      await request("POST", `/api/folders/${encodeURIComponent(folderId)}/versions/restore`, {
        path,
        versionTime,
      });
    },
    events: (limit = 20) =>
      request("GET", `/api/events?limit=${limit}`).then(parsed(eventSchema.array())),
  };
}
