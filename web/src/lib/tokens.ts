import type { ApiToken, TokenScope } from "../api/types";
import { m } from "../paraglide/messages.js";
import { ago } from "./format";

export const TOKEN_SCOPES: TokenScope[] = ["manage", "read"];

export function scopeLabel(scope: TokenScope): string {
  return scope === "manage" ? m.tokens_scope_manage() : m.tokens_scope_read();
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
