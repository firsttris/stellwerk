import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { RefreshCw } from "lucide-react";
import { type FormEvent, useState } from "react";
import { api } from "../api";
import { sessionQuery } from "../api/queries";
import { Button } from "../components/ui/button";
import { Field, Input } from "../components/ui/input";

export const Route = createFileRoute("/login")({ component: Login });

function Login() {
  const [password, setPassword] = useState("");
  const { data: session } = useQuery(sessionQuery);
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const login = useMutation({
    mutationFn: () => api.login(password),
    onSuccess: async (next) => {
      queryClient.setQueryData(sessionQuery.queryKey, next);
      await navigate({ to: "/" });
    },
  });

  const submit = (event: FormEvent) => {
    event.preventDefault();
    login.mutate();
  };

  return (
    <main className="grid min-h-screen place-items-center bg-subtle px-4">
      <form
        onSubmit={submit}
        className="flex w-full max-w-sm flex-col gap-5 rounded-xl border border-border bg-background p-6"
      >
        <div className="flex items-center gap-2.5">
          <div className="grid size-8 place-items-center rounded-lg bg-primary text-primary-foreground">
            <RefreshCw className="size-[18px]" aria-hidden="true" />
          </div>
          <h1 className="text-lg font-semibold">Stellwerk</h1>
        </div>
        {session && !session.passwordSet ? (
          <p className="text-sm text-muted-foreground">
            Noch kein Passwort gesetzt. Auf dem Server einmal{" "}
            <code className="font-mono">stellwerk set-password</code> ausführen.
          </p>
        ) : (
          <Field label="Passwort">
            {(id) => (
              <Input
                id={id}
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoFocus
              />
            )}
          </Field>
        )}
        {login.error && <p className="text-sm text-danger">{login.error.message}</p>}
        <Button type="submit" disabled={!password || login.isPending}>
          Anmelden
        </Button>
      </form>
    </main>
  );
}
