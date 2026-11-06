import { describe, expect, it } from "vitest";
import { demoApi } from "./api";

describe("demoApi", () => {
  it("never reuses a folder id after a removal", async () => {
    const api = demoApi();
    const first = await api.createFolder("A");
    await api.createFolder("B");
    await api.deleteFolder(first.id);
    await api.createFolder("C");
    const ids = (await api.overview()).folders.map((f) => f.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it("shows a new token's secret once and forgets a revoked one", async () => {
    const api = demoApi();
    const created = await api.createToken("script", "read");
    expect(created.secret).toMatch(/^stw_/);
    expect((await api.tokens()).find((t) => t.id === created.id)).not.toHaveProperty("secret");
    await api.deleteToken(created.id);
    expect((await api.tokens()).some((t) => t.id === created.id)).toBe(false);
  });
});
