import { describe, expect, it } from "vitest";
import { baseName } from "./paths";

describe("baseName", () => {
  it("takes the last part with either separator", () => {
    expect(baseName("/home/demo/Pictures")).toBe("Pictures");
    expect(baseName("C:\\Users\\demo\\Holidays 2026")).toBe("Holidays 2026");
    expect(baseName("~/Fotos/")).toBe("Fotos");
    expect(baseName("/")).toBe("");
  });
});
