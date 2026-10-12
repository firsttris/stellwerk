import type { z } from "zod";
import {
  type Api,
  ApiError,
  deviceSchema,
  enrollmentSchema,
  enrollmentStatusSchema,
  eventSchema,
  folderSchema,
  overviewSchema,
  type Session,
  sessionSchema,
} from "./types";

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
    const text = await response.text();
    const json: unknown = text ? JSON.parse(text) : null;
    if (!response.ok) {
      const message =
        json && typeof json === "object" && "error" in json && typeof json.error === "string"
          ? json.error
          : response.statusText;
      throw new ApiError(response.status, message);
    }
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
    applyShares: async (changes) => {
      const json = await request("POST", "/api/shares", { changes });
      return json as { revision: number };
    },
    updateDevice: (id, patch) =>
      request("PATCH", `/api/devices/${encodeURIComponent(id)}`, patch).then(parsed(deviceSchema)),
    deleteDevice: async (id) => {
      await request("DELETE", `/api/devices/${encodeURIComponent(id)}`);
    },
    createEnrollment: () => request("POST", "/api/enrollments").then(parsed(enrollmentSchema)),
    enrollmentStatus: (code) =>
      request("POST", "/api/enrollments/status", { code }).then(parsed(enrollmentStatusSchema)),
    events: (limit = 20) =>
      request("GET", `/api/events?limit=${limit}`).then(parsed(eventSchema.array())),
  };
}
