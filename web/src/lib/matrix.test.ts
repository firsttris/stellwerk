import { describe, expect, it } from "vitest";
import type { Overview } from "../api/types";
import { demoState } from "../demo/fixtures";
import {
  cellMode,
  cycleCell,
  type Draft,
  draftChanges,
  isWaiting,
  nextMode,
  offlineChanges,
  setCell,
} from "./matrix";

const overview = (): Overview => {
  const state = demoState(1_000_000);
  return {
    revision: state.revision,
    hub: { syncthingId: "HUB", address: "tcp://hub:22001" },
    devices: state.devices,
    folders: state.folders,
    shares: state.shares,
  };
};

describe("matrix draft", () => {
  it("cycles off → both → send → receive → off", () => {
    expect(nextMode("off")).toBe("sendReceive");
    expect(nextMode("sendReceive")).toBe("sendOnly");
    expect(nextMode("sendOnly")).toBe("receiveOnly");
    expect(nextMode("receiveOnly")).toBe("off");
  });

  it("keeps only cells that differ from what is stored", () => {
    const o = overview();
    let draft: Draft = new Map();
    // fotos on laptop is off; four clicks bring it back to off.
    for (let i = 0; i < 3; i++) draft = cycleCell(o, draft, "fotos", "laptop");
    expect(cellMode(o, draft, "fotos", "laptop")).toBe("receiveOnly");
    expect(draft.size).toBe(1);
    draft = cycleCell(o, draft, "fotos", "laptop");
    expect(draft.size).toBe(0);
  });

  it("turns the draft into API changes, off as null", () => {
    const o = overview();
    let draft: Draft = setCell(o, new Map(), "projekte", "werkbank", "off");
    draft = setCell(o, draft, "fotos", "laptop", "sendReceive");
    expect(draftChanges(o, draft)).toEqual([
      { folderId: "projekte", deviceId: "werkbank", mode: null },
      { folderId: "fotos", deviceId: "laptop", mode: "sendReceive" },
    ]);
  });

  it("drops cells of folders that are gone and counts offline devices", () => {
    const o = overview();
    let draft: Draft = setCell(o, new Map(), "gone", "laptop", "sendOnly");
    draft = setCell(o, draft, "musik", "buero", "receiveOnly");
    expect(draftChanges(o, draft)).toEqual([
      { folderId: "musik", deviceId: "buero", mode: "receiveOnly" },
    ]);
    expect(offlineChanges(o, draft)).toBe(1);
  });

  it("waits only for offline devices behind the revision", () => {
    const [online, , offline] = overview().devices;
    if (!online || !offline) throw new Error("fixtures changed");
    expect(isWaiting(offline, 12)).toBe(true);
    expect(isWaiting(offline, 11)).toBe(false);
    expect(isWaiting({ ...online, appliedRev: 1 }, 12)).toBe(false);
  });
});
