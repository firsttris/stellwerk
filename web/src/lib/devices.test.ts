import { describe, expect, it } from "vitest";
import { demoState } from "../demo/fixtures";
import { filterDevices } from "./devices";

describe("filterDevices", () => {
  const { devices } = demoState(0);
  const names = (filter: "all" | "online" | "offline", search: string) =>
    filterDevices(devices, filter, search).map((d) => d.name);

  it("filters by online state", () => {
    expect(names("all", "")).toHaveLength(devices.length);
    expect(names("offline", "")).toEqual(["office-pc"]);
    expect(names("online", "")).not.toContain("office-pc");
  });

  it("searches name and system, ignoring case and spaces", () => {
    expect(names("all", " MAC ")).toEqual(["macbook"]);
    expect(names("all", "windows")).toEqual(
      devices.filter((d) => d.os === "windows").map((d) => d.name),
    );
    expect(names("online", "office")).toEqual([]);
  });
});
