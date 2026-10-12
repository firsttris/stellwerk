import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { eventsQuery, overviewQuery, refresh, statusQuery } from "./queries";
import { uiMessageSchema } from "./types";

const MAX_RETRY_MS = 30_000;

const onlineKey = (ids: readonly string[]) => [...ids].sort().join(",");

/**
 * Keeps the Zentrale's live state in the query cache through `/api/ws`: the status is set
 * directly, the overview reloaded when the revision or who is online changed. Reconnects with
 * a growing pause. Not in demo mode, which has no server.
 */
export function useLiveUpdates() {
  const queryClient = useQueryClient();
  useEffect(() => {
    if (import.meta.env.VITE_DEMO) return;
    let socket: WebSocket | undefined;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let delay = 1_000;
    let stopped = false;

    const connect = () => {
      const scheme = window.location.protocol === "https:" ? "wss" : "ws";
      socket = new WebSocket(`${scheme}://${window.location.host}/api/ws`);
      socket.onopen = () => {
        delay = 1_000;
      };
      socket.onmessage = (event) => {
        const parsed = uiMessageSchema.safeParse(JSON.parse(String(event.data)));
        if (!parsed.success) return;
        const message = parsed.data;
        queryClient.setQueryData(statusQuery.queryKey, message.status);
        const overview = queryClient.getQueryData(overviewQuery.queryKey);
        const online = overview?.devices.filter((d) => d.online).map((d) => d.id) ?? [];
        if (
          !overview ||
          overview.revision !== message.revision ||
          onlineKey(online) !== onlineKey(message.online)
        ) {
          void refresh(queryClient);
        } else {
          void queryClient.invalidateQueries({ queryKey: eventsQuery.queryKey });
        }
      };
      socket.onclose = () => {
        if (stopped) return;
        retry = setTimeout(connect, delay);
        delay = Math.min(delay * 2, MAX_RETRY_MS);
      };
    };

    connect();
    return () => {
      stopped = true;
      clearTimeout(retry);
      socket?.close();
    };
  }, [queryClient]);
}
