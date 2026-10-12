import { afterEach, describe, expect, it, vi } from "vitest";
import { httpApi } from "./http";
import { ApiError } from "./types";

const respond = (status: number, body: string, statusText = "") =>
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => new Response(body || null, { status, statusText })),
  );

const failure = (promise: Promise<unknown>) =>
  promise.then(
    () => {
      throw new Error("expected a failure");
    },
    (error: unknown) => error,
  );

describe("httpApi", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("turns the Zentrale's error body into an ApiError", async () => {
    respond(401, JSON.stringify({ error: "not logged in" }));
    const error = await failure(httpApi().overview());
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ status: 401, message: "not logged in" });
  });

  it("keeps the status when the body is not JSON", async () => {
    respond(502, "<html>Bad Gateway</html>", "Bad Gateway");
    const error = await failure(httpApi().overview());
    expect(error).toBeInstanceOf(ApiError);
    expect(error).toMatchObject({ status: 502, message: "Bad Gateway" });
  });

  it("checks the revision an apply returns", async () => {
    respond(200, JSON.stringify({ revision: 7 }));
    await expect(httpApi().applyShares([])).resolves.toEqual({ revision: 7 });
    respond(200, JSON.stringify({ rev: 7 }));
    await expect(httpApi().applyShares([])).rejects.toThrow();
  });
});
