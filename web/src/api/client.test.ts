import { describe, expect, it, vi } from "vitest";
import { createQueryClient, shouldRetry, UNAUTHORIZED_IS_ANSWER } from "./client";
import { ApiError } from "./types";

describe("shouldRetry", () => {
  it("retries network and server errors, not client errors", () => {
    expect(shouldRetry(0, new TypeError("Failed to fetch"))).toBe(true);
    expect(shouldRetry(0, new ApiError(502, "Bad Gateway"))).toBe(true);
    expect(shouldRetry(3, new ApiError(502, "Bad Gateway"))).toBe(false);
    expect(shouldRetry(0, new ApiError(401, "not logged in"))).toBe(false);
    expect(shouldRetry(0, new ApiError(403, "missing or wrong CSRF token"))).toBe(false);
    expect(shouldRetry(0, new ApiError(404, "conflict copy not found"))).toBe(false);
  });
});

describe("createQueryClient", () => {
  const unauthorized = () => Promise.reject(new ApiError(401, "not logged in"));

  it("reports an ended session from queries and mutations", async () => {
    const onUnauthorized = vi.fn();
    const client = createQueryClient(onUnauthorized);
    await client.fetchQuery({ queryKey: ["x"], queryFn: unauthorized }).catch(() => undefined);
    expect(onUnauthorized).toHaveBeenCalledTimes(1);
    await client
      .getMutationCache()
      .build(client, { mutationFn: unauthorized })
      .execute(undefined)
      .catch(() => undefined);
    expect(onUnauthorized).toHaveBeenCalledTimes(2);
  });

  it("leaves other errors and a wrong password alone", async () => {
    const onUnauthorized = vi.fn();
    const client = createQueryClient(onUnauthorized);
    await client
      .fetchQuery({ queryKey: ["y"], queryFn: () => Promise.reject(new ApiError(404, "gone")) })
      .catch(() => undefined);
    await client
      .getMutationCache()
      .build(client, { mutationFn: unauthorized, meta: UNAUTHORIZED_IS_ANSWER })
      .execute(undefined)
      .catch(() => undefined);
    expect(onUnauthorized).not.toHaveBeenCalled();
  });
});
