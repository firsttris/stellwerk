import { describe, expect, it } from "vitest";
import { demoDirectory } from "./folders";

describe("demoDirectory", () => {
  it("starts in the home directory and walks down and up", () => {
    const home = demoDirectory("linux", "");
    expect(home.path).toBe("/home/demo");
    expect(home.parent).toBe("/home");
    const pictures = home.dirs.find((d) => d.name === "Pictures");
    expect(pictures?.path).toBe("/home/demo/Pictures");
    expect(demoDirectory("linux", pictures?.path ?? "").dirs.map((d) => d.name)).toEqual([
      "Holidays 2026",
      "Phone",
    ]);
    expect(demoDirectory("linux", "/").dirs.map((d) => d.path)).toEqual(["/home"]);
    expect(demoDirectory("linux", "/").parent).toBeNull();
  });

  it("writes Windows paths with drives and backslashes", () => {
    const home = demoDirectory("windows", "~");
    expect(home.path).toBe("C:\\Users\\demo");
    expect(home.dirs[0]?.path).toBe("C:\\Users\\demo\\Desktop");
    expect(home.roots).toEqual(["C:\\", "D:\\"]);
    expect(demoDirectory("windows", "C:\\Users").parent).toBe("C:\\");
    expect(demoDirectory("windows", "C:\\").dirs.map((d) => d.name)).toEqual(["Users"]);
    expect(demoDirectory("windows", "D:\\").dirs).toEqual([]);
  });

  it("refuses a relative path as the agent does", () => {
    expect(() => demoDirectory("macos", "Pictures")).toThrow(/not an absolute path/);
  });
});
