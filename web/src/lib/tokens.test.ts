import { describe, expect, it } from "vitest";
import type { ApiToken } from "../api/types";
import { claudeCommand, tokenUsage } from "./tokens";

const token = (lastUsedAt: number | null): ApiToken => ({
  id: "t",
  name: "Claude Code",
  scope: "manage",
  createdAt: 0,
  lastUsedAt,
});

describe("tokenUsage", () => {
  it("says when a token was made and last used", () => {
    expect(tokenUsage(token(null), 3 * 24 * 3600)).toBe("erstellt vor 3 Tagen · noch nie benutzt");
    expect(tokenUsage(token(3 * 24 * 3600 - 120), 3 * 24 * 3600)).toBe(
      "erstellt vor 3 Tagen · zuletzt benutzt vor 2 Min.",
    );
  });
});

describe("claudeCommand", () => {
  it("points Claude Code at the Zentrale's MCP endpoint with the token", () => {
    expect(claudeCommand("https://sync.example.org", "stw_abc")).toBe(
      'claude mcp add --transport http --scope user stellwerk https://sync.example.org/mcp --header "Authorization: Bearer stw_abc"',
    );
  });
});
