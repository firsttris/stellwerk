import { describe, expect, it } from "vitest";
import type { Overview } from "../api/types";
import { demoLive, demoState } from "../demo/fixtures";
import {
  deviceProgress,
  deviceSummary,
  folderBadge,
  folderErrors,
  folderSize,
  formatBytes,
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
  });

  it("gives every folder a badge", () => {
    const o = overview();
    const live = demoLive(now);
    expect(folderBadge(o, live, "fotos")).toEqual({ tone: "info", text: "Läuft · 72 %" });
    expect(folderBadge(o, live, "musik")).toEqual({ tone: "ok", text: "Synchron" });
    expect(folderBadge(o, live, "dokumente")).toEqual({ tone: "warn", text: "Fehler" });
    expect(folderBadge(o, undefined, "musik").text).toBe("…");
    const withConflict = { ...live, conflicts: demoState(now).conflicts };
    expect(folderBadge(o, withConflict, "projekte")).toEqual({ tone: "warn", text: "1 Konflikt" });
    expect(folderErrors(o, live, "dokumente")).toEqual([
      { device: "laptop", message: "2 Dateien nicht synchronisiert" },
    ]);
    expect(folderSize(live, "projekte")).toBe("48.210 Dateien · 18,4 GB");
  });

  it("says what a device is doing or waiting for", () => {
    const o = overview();
    const live = demoLive(now);
    const device = (id: string) => {
      const found = o.devices.find((d) => d.id === id);
      if (!found) throw new Error(id);
      return found;
    };
    expect(waitingChanges(o, live, "buero")).toBe(214);
    expect(deviceSummary(o, live, device("buero"))).toBe("214 Änderungen warten auf dem Server");
    expect(deviceProgress(o, live, "werkbank")).toBe(72);
    expect(deviceSummary(o, live, device("werkbank"))).toBe("Lädt 18 Änderungen · 72 %");
    expect(deviceSummary(o, live, device("macbook"))).toBe("Synchron");
  });
});
