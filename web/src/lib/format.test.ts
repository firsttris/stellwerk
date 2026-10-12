import { describe, expect, it } from "vitest";
import { demoState } from "../demo/fixtures";
import { ago, bothName, describeEvent, deviceStatus, duration, since } from "./format";

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
    const [workbench, , office] = demoState(now).devices;
    if (!workbench || !office) throw new Error("fixtures changed");
    expect(deviceStatus(workbench, now)).toBe("Online · Linux");
    expect(deviceStatus(office, now)).toBe("Offline · 3 Tage");
    expect(deviceStatus({ ...workbench, approved: false }, now)).toBe("Gesperrt");
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
      folderId: "photos",
    });
    expect(describeEvent(event("changes", "18"), devices, folders)).toBe(
      "laptop hat 18 Dateien in Photos geändert",
    );
    expect(describeEvent(event("changes", "1"), devices, folders)).toBe(
      "laptop hat 1 Datei in Photos geändert",
    );
    expect(describeEvent(event("offline"), devices, folders)).toBe("laptop ist offline");
  });

  it("names the copy kept next to the file like the engine does", () => {
    expect(bothName("haul/src/config.ts", "workbench")).toBe("config.workbench.ts");
    expect(bothName("archive.tar.gz", "laptop")).toBe("archive.tar.laptop.gz");
    expect(bothName("Makefile", "laptop")).toBe("Makefile.laptop");
    expect(bothName("dir/.bashrc", "laptop")).toBe(".bashrc.laptop");
    expect(bothName("a.txt", "Tristan's Mac")).toBe("a.Tristan-s-Mac.txt");
    expect(bothName("a.txt", "büro_pc-2")).toBe("a.büro_pc-2.txt");
  });
});
