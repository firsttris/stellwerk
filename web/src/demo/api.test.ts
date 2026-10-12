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
});
