import { describe, expect, it } from "vitest";
import { demoState } from "../demo/fixtures";
import { ago, describeEvent, deviceStatus, duration, plural, since } from "./format";

describe("format", () => {
  it("says how long ago", () => {
    expect(ago(100, 130)).toBe("gerade eben");
    expect(ago(0, 5 * 60)).toBe("vor 5 Min.");
    expect(ago(0, 3 * 3600)).toBe("vor 3 Std.");
    expect(ago(0, 86400)).toBe("vor 1 Tag");
    expect(ago(0, 3 * 86400)).toBe("vor 3 Tagen");
  });

  it("describes devices", () => {
    const now = 1_000_000;
    const [werkbank, , buero] = demoState(now).devices;
    if (!werkbank || !buero) throw new Error("fixtures changed");
    expect(deviceStatus(werkbank, now)).toBe("Online · Linux");
    expect(deviceStatus(buero, now)).toBe("Offline · 3 Tage");
    expect(deviceStatus({ ...werkbank, approved: false }, now)).toBe("Gesperrt");
    expect(duration(30)).toBe("1 Min.");
    expect(since(3 * 86400)).toBe("seit 3 Tagen");
    expect(since(86400)).toBe("seit 1 Tag");
  });

  it("describes activity", () => {
    const { devices, folders } = demoState(0);
    const event = (kind: string, text = "") => ({
      kind,
      text,
      deviceId: "laptop",
      folderId: "fotos",
    });
    expect(describeEvent(event("changes", "18"), devices, folders)).toBe(
      "laptop hat 18 Dateien in Fotos geändert",
    );
    expect(describeEvent(event("changes", "1"), devices, folders)).toBe(
      "laptop hat 1 Datei in Fotos geändert",
    );
    expect(describeEvent(event("offline"), devices, folders)).toBe("laptop ist offline");
    expect(plural(1, "Gerät", "Geräte")).toBe("1 Gerät");
    expect(plural(4, "Gerät", "Geräte")).toBe("4 Geräte");
  });
});
