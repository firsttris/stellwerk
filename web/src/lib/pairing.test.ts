import { describe, expect, it } from "vitest";
import { pairCommand } from "./pairing";

const enrollment = { code: "K7Q2-MX9P-41TZ", expiresAt: 0, version: "1.2.3", caSha256: null };
const origin = "https://stellwerk.home";
const ca = "da".repeat(32);

describe("pairCommand", () => {
  it("fetches the install script of the Zentrale's version from GitHub", () => {
    expect(pairCommand("linux", origin, enrollment)).toBe(
      "curl -fsSL https://github.com/firsttris/stellwerk/releases/download/v1.2.3/install.sh" +
        " | sh -s -- https://stellwerk.home K7Q2-MX9P-41TZ",
    );
    expect(pairCommand("macos", origin, enrollment)).toBe(pairCommand("linux", origin, enrollment));
    expect(pairCommand("windows", origin, enrollment)).toBe(
      "& ([scriptblock]::Create((irm https://github.com/firsttris/stellwerk/releases/download/v1.2.3/install.ps1)))" +
        " https://stellwerk.home K7Q2-MX9P-41TZ",
    );
  });

  it("passes a private CA's fingerprint on", () => {
    const withCa = { ...enrollment, caSha256: ca };
    expect(pairCommand("linux", origin, withCa)).toMatch(
      new RegExp(`sh -s -- https://stellwerk.home K7Q2-MX9P-41TZ --ca-sha256 ${ca}$`),
    );
    expect(pairCommand("windows", origin, withCa)).toMatch(
      new RegExp(`https://stellwerk.home K7Q2-MX9P-41TZ -CaSha256 ${ca}$`),
    );
  });
});
