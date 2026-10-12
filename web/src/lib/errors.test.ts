import { describe, expect, it } from "vitest";
import { z } from "zod";
import { ApiError } from "../api/types";
import { errorMessage, loginErrorMessage } from "./errors";

describe("errorMessage", () => {
  it("translates what the user can act on", () => {
    expect(errorMessage(new ApiError(401, "not logged in"))).toBe("Nicht mehr angemeldet.");
    expect(errorMessage(new ApiError(403, "missing or wrong CSRF token"))).toBe(
      "Die Anfrage wurde abgelehnt. Bitte die Seite neu laden.",
    );
    expect(errorMessage(new ApiError(502, "Bad Gateway"))).toBe(
      "Der Server hat einen Fehler gemeldet (502).",
    );
    expect(errorMessage(new TypeError("Failed to fetch"))).toBe("Der Server ist nicht erreichbar.");
    const zod = z.object({ a: z.string() }).safeParse({});
    expect(zod.success).toBe(false);
    if (!zod.success) expect(errorMessage(zod.error)).toMatch(/^Unerwartete Antwort/);
  });

  it("keeps the server's detail otherwise", () => {
    expect(errorMessage(new ApiError(400, "a device needs a name"))).toBe("a device needs a name");
  });
});

describe("loginErrorMessage", () => {
  it("names a wrong password and a missing one", () => {
    expect(loginErrorMessage(new ApiError(401, "wrong password"))).toBe("Falsches Passwort.");
    expect(loginErrorMessage(new ApiError(409, "no password set yet"))).toBe(
      "Noch kein Passwort gesetzt.",
    );
    expect(loginErrorMessage(new ApiError(503, ""))).toBe(
      "Der Server hat einen Fehler gemeldet (503).",
    );
  });
});
