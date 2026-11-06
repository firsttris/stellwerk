import type { ApiToken, TokenScope } from "../api/types";
import { m } from "../paraglide/messages.js";
import { ago } from "./format";

export const TOKEN_SCOPES: TokenScope[] = ["manage", "read"];

export function scopeLabel(scope: TokenScope): string {
  return scope === "manage" ? m.tokens_scope_manage() : m.tokens_scope_read();
}

/** Adds the Zentrale's MCP server (`/mcp`) to Claude Code, for every project of the user. */
export function claudeCommand(origin: string, secret: string): string {
  return `claude mcp add --transport http --scope user stellwerk ${origin}/mcp --header "Authorization: Bearer ${secret}"`;
}

/** "erstellt vor 9 Tagen · zuletzt benutzt vor 2 Std." */
export function tokenUsage(token: ApiToken, now: number): string {
  const created = m.tokens_created({ ago: ago(token.createdAt, now) });
  const used =
    token.lastUsedAt === null
      ? m.tokens_never_used()
      : m.tokens_last_used({ ago: ago(token.lastUsedAt, now) });
  return `${created} · ${used}`;
}
