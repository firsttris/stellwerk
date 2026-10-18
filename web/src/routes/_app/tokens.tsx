import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { KeyRound, Plus } from "lucide-react";
import { type FormEvent, useState } from "react";
import { api } from "../../api";
import { tokensQuery } from "../../api/queries";
import type { ApiToken, CreatedToken, TokenScope } from "../../api/types";
import { CopyableCode } from "../../components/copyable-code";
import { PageHeader } from "../../components/layout";
import { Button } from "../../components/ui/button";
import { Badge, Card } from "../../components/ui/card";
import { Field, Input, Select } from "../../components/ui/input";
import { errorMessage } from "../../lib/errors";
import { nowSeconds } from "../../lib/format";
import { scopeLabel, TOKEN_SCOPES, tokenUsage } from "../../lib/tokens";
import { m } from "../../paraglide/messages.js";

export const Route = createFileRoute("/_app/tokens")({ component: Tokens });

/** API tokens for scripts and AI assistants: create one, see it once, revoke it. */
function Tokens() {
  const { data: tokens, error } = useQuery(tokensQuery);
  const [created, setCreated] = useState<CreatedToken>();
  return (
    <>
      <PageHeader title={m.tokens_title()} description={m.tokens_description()} />
      <NewToken onCreated={setCreated} />
      {created && <NewSecret token={created} />}
      {error && <p className="text-sm text-danger">{errorMessage(error)}</p>}
      {tokens && tokens.length === 0 && (
        <Card className="p-5 text-muted-foreground">{m.tokens_none()}</Card>
      )}
      {tokens && tokens.length > 0 && (
        <Card className="divide-y divide-border" aria-label={m.tokens_list()}>
          {tokens.map((token) => (
            <TokenRow
              key={token.id}
              token={token}
              onRevoked={() => {
                if (created?.id === token.id) setCreated(undefined);
              }}
            />
          ))}
        </Card>
      )}
    </>
  );
}

function NewToken({ onCreated }: { onCreated: (token: CreatedToken) => void }) {
  const [name, setName] = useState("");
  const [scope, setScope] = useState<TokenScope>("manage");
  const queryClient = useQueryClient();
  const create = useMutation({
    mutationFn: () => api.createToken(name, scope),
    onSuccess: async (token) => {
      setName("");
      onCreated(token);
      await queryClient.invalidateQueries({ queryKey: tokensQuery.queryKey });
    },
  });
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (name.trim()) create.mutate();
  };

  return (
    <Card className="p-5">
      <form onSubmit={submit} className="flex flex-col gap-4">
        <div className="grid gap-4 sm:grid-cols-[1fr_220px]">
          <Field label={m.name()}>
            {(id) => (
              <Input
                id={id}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder={m.tokens_name_placeholder()}
              />
            )}
          </Field>
          <Field label={m.tokens_scope()}>
            {(id) => (
              <Select
                id={id}
                value={scope}
                onChange={(e) => setScope(e.target.value as TokenScope)}
              >
                {TOKEN_SCOPES.map((s) => (
                  <option key={s} value={s}>
                    {scopeLabel(s)}
                  </option>
                ))}
              </Select>
            )}
          </Field>
        </div>
        <p className="text-sm text-muted-foreground">
          {scope === "manage" ? m.tokens_scope_manage_hint() : m.tokens_scope_read_hint()}
        </p>
        {create.error && <p className="text-sm text-danger">{errorMessage(create.error)}</p>}
        <div className="flex justify-end">
          <Button type="submit" disabled={!name.trim() || create.isPending}>
            <Plus aria-hidden="true" />
            {m.tokens_create()}
          </Button>
        </div>
      </form>
    </Card>
  );
}

/** The token itself, shown only right after it was created. */
function NewSecret({ token }: { token: CreatedToken }) {
  return (
    <Card className="flex flex-col gap-3 border-ok-border bg-ok-surface p-5" role="status">
      <span className="font-medium">{m.tokens_created_title({ name: token.name })}</span>
      <CopyableCode text={token.secret} label={m.tokens_copy()} />
      <span className="text-sm text-muted-foreground">{m.tokens_shown_once()}</span>
    </Card>
  );
}

function TokenRow({ token, onRevoked }: { token: ApiToken; onRevoked: () => void }) {
  const queryClient = useQueryClient();
  const revoke = useMutation({
    mutationFn: () => api.deleteToken(token.id),
    onSuccess: async () => {
      onRevoked();
      await queryClient.invalidateQueries({ queryKey: tokensQuery.queryKey });
    },
  });
  const confirmRevoke = () => {
    if (window.confirm(m.tokens_revoke_confirm({ name: token.name }))) revoke.mutate();
  };

  return (
    <div className="flex flex-wrap items-center gap-3 px-5 py-4">
      <KeyRound className="size-4 text-muted-foreground" aria-hidden="true" />
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="flex flex-wrap items-center gap-2 font-medium">
          {token.name}
          <Badge tone={token.scope === "manage" ? "info" : "neutral"}>
            {scopeLabel(token.scope)}
          </Badge>
        </span>
        <span className="text-[13px] text-muted-foreground">{tokenUsage(token, nowSeconds())}</span>
        {revoke.error && (
          <span role="alert" className="text-xs text-danger">
            {errorMessage(revoke.error)}
          </span>
        )}
      </div>
      <Button
        variant="destructive"
        size="sm"
        onClick={confirmRevoke}
        disabled={revoke.isPending}
        aria-label={m.tokens_revoke_token({ name: token.name })}
      >
        {m.tokens_revoke()}
      </Button>
    </div>
  );
}
