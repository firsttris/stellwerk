import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";
import { eventsQuery, overviewQuery, refresh, statusQuery } from "./queries";
import { uiMessageSchema } from "./types";

const MAX_RETRY_MS = 30_000;

const onlineKey = (ids: readonly string[]) => [...ids].sort().join(",");

/** Whether the live link to the Zentrale is up; the demo has no server and is always "open". */
export type LinkState = "connecting" | "open" | "closed";

/**
 * Keeps the Zentrale's live state in the query cache through `/api/ws`: the status is set
 * directly, the overview reloaded when the revision or who is online changed. Reconnects with
 * a growing pause. Not in demo mode, which has no server.
 */
export function useLiveUpdates(): LinkState {
  const queryClient = useQueryClient();
  const [link, setLink] = useState<LinkState>(import.meta.env.VITE_DEMO ? "open" : "connecting");
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
        setLink("open");
      };
      socket.onmessage = (event) => {
        const parsed = uiMessageSchema.safeParse(JSON.parse(String(event.data)));
        if (!parsed.success) return;
        const message = parsed.data;
        // A poll still in flight would overwrite this newer state with its older answer: cancel
        // it, and set the state once the cancelled fetch has settled (it reverts on cancel).
        void queryClient
          .cancelQueries({ queryKey: statusQuery.queryKey })
          .then(() => queryClient.setQueryData(statusQuery.queryKey, message.status));
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
        setLink("closed");
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
  return link;
}
