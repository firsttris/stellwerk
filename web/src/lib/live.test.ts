import { describe, expect, it } from "vitest";
import type { Overview } from "../api/types";
import { demoLive, demoState } from "../demo/fixtures";
import {
  deviceFolderCount,
  deviceProgress,
  deviceSummary,
  folderBadge,
  folderErrors,
  folderSize,
  formatBytes,
  holders,
  shareStatus,
  waitingChanges,
} from "./live";

const now = 1_000_000;
const overview = (): Overview => {
  const state = demoState(now);
  return {
    revision: state.revision,
    hub: { syncthingId: "HUB", address: "tcp://hub:22001" },
    devices: state.devices,
    folders: state.folders,
    shares: state.shares,
  };
};

describe("live state in words", () => {
  it("formats sizes the German way", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512_000)).toBe("512 KB");
    expect(formatBytes(18_400_000_000)).toBe("18,4 GB");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(999_940)).toBe("999,9 KB");
    expect(formatBytes(999_990)).toBe("1 MB");
  });

  it("gives every folder a badge", () => {
    const o = overview();
    const live = demoLive(now);
    expect(folderBadge(o, live, "photos")).toEqual({ tone: "info", text: "Läuft · 72 %" });
    expect(folderBadge(o, live, "music")).toEqual({ tone: "ok", text: "Synchron" });
    expect(folderBadge(o, live, "documents")).toEqual({ tone: "warn", text: "Fehler" });
    expect(folderBadge(o, undefined, "music").text).toBe("…");
    const withConflict = { ...live, conflicts: demoState(now).conflicts };
    expect(folderBadge(o, withConflict, "projects")).toEqual({ tone: "warn", text: "1 Konflikt" });
    expect(folderErrors(o, live, "documents")).toEqual([
      { device: "laptop", message: "2 Dateien nicht synchronisiert" },
    ]);
    expect(folderSize(live, "projects")).toBe("48.210 Dateien · 18,4 GB");
  });

  it("says what a device is doing or waiting for", () => {
    const o = overview();
    const live = demoLive(now);
    const device = (id: string) => {
      const found = o.devices.find((d) => d.id === id);
      if (!found) throw new Error(id);
      return found;
    };
    expect(waitingChanges(o, live, "office")).toBe(214);
    expect(deviceSummary(o, live, device("office"))).toBe("214 Änderungen warten auf dem Server");
    expect(deviceProgress(o, live, "workbench")).toBe(72);
    expect(deviceSummary(o, live, device("workbench"))).toBe("Lädt 18 Änderungen · 72 %");
    expect(deviceSummary(o, live, device("macbook"))).toBe("Synchron");
    const onlyDeletes = structuredClone(live);
    const photos = onlyDeletes.hub.completion.photos?.workbench;
    if (!photos) throw new Error("fixtures changed");
    photos.needItems = 0;
    expect(deviceSummary(o, onlyDeletes, device("workbench"))).toBe("Synchron");
  });

  it("counts failed files in the singular too", () => {
    const o = overview();
    const live = structuredClone(demoLive(now));
    const laptop = live.devices.laptop?.folders.documents;
    if (!laptop) throw new Error("fixtures changed");
    laptop.errors = 1;
    expect(folderErrors(o, live, "documents")).toEqual([
      { device: "laptop", message: "1 Datei nicht synchronisiert" },
    ]);
  });

  it("leaves blocked devices out of the counts", () => {
    const o = overview();
    const before = holders(o, "projects").map((d) => d.id);
    expect(before).toContain("laptop");
    const blocked = {
      ...o,
      devices: o.devices.map((d) => (d.id === "laptop" ? { ...d, approved: false } : d)),
    };
    expect(holders(blocked, "projects").map((d) => d.id)).toEqual(
      before.filter((id) => id !== "laptop"),
    );
    const laptop = blocked.devices.find((d) => d.id === "laptop");
    if (!laptop) throw new Error("fixtures changed");
    expect(deviceFolderCount(blocked, laptop)).toBe(0);
    expect(deviceFolderCount(o, { ...laptop, approved: true })).toBeGreaterThan(0);
  });

  it("says how far a device is with one folder", () => {
    const o = overview();
    const live = demoLive(now);
    const device = (id: string) => {
      const found = o.devices.find((d) => d.id === id);
      if (!found) throw new Error(id);
      return found;
    };
    const status = (id: string, folder: string, l = live) =>
      shareStatus(device(id), l, folder, o.revision);
    expect(status("workbench", "photos")).toBe("Lädt 18 Änderungen · 72 %");
    expect(status("office", "projects")).toBe("Offline · 202 Änderungen warten auf dem Server");
    expect(status("macbook", "photos")).toBe("Synchron");
    const onlyDeletes = structuredClone(live);
    const photos = onlyDeletes.hub.completion.photos?.workbench;
    if (!photos) throw new Error("fixtures changed");
    photos.needItems = 0;
    expect(status("workbench", "photos", onlyDeletes)).toBe("Synchron");
  });
});
