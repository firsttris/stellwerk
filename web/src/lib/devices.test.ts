import { describe, expect, it } from "vitest";
import type { Device } from "../api/types";
import { demoState } from "../demo/fixtures";
import { agentOutdated, filterDevices } from "./devices";

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

describe("agentOutdated", () => {
  const agent = (agentVersion: string) => ({ agentVersion }) as Device;

  it("compares versions as numbers", () => {
    expect(agentOutdated(agent("0.1.9"), "0.1.11")).toBe(true);
    expect(agentOutdated(agent("0.1.11"), "0.2.0")).toBe(true);
    expect(agentOutdated(agent("0.1.11"), "0.1.11")).toBe(false);
    expect(agentOutdated(agent("0.1.12"), "0.1.11")).toBe(false);
  });

  it("does not guess about anything else", () => {
    expect(agentOutdated(agent(""), "0.1.11")).toBe(false);
    expect(agentOutdated(agent("dev"), "0.1.11")).toBe(false);
    expect(agentOutdated(agent("0.1.9"), "")).toBe(false);
  });
});
