import { type QueryClient, queryOptions } from "@tanstack/react-query";
import { api } from ".";

// Query keys and options in one place; mutations invalidate through `refresh`.

export const sessionQuery = queryOptions({
  queryKey: ["session"],
  queryFn: () => api.session(),
  staleTime: 60_000,
});

// The WebSocket (`useLiveUpdates`) keeps these fresh; the intervals are only a fallback.

export const overviewQuery = queryOptions({
  queryKey: ["overview"],
  queryFn: () => api.overview(),
  refetchInterval: 30_000,
});

export const statusQuery = queryOptions({
  queryKey: ["status"],
  queryFn: () => api.status(),
  refetchInterval: 30_000,
});

export const eventsQuery = queryOptions({
  queryKey: ["events"],
  queryFn: () => api.events(8),
  refetchInterval: 60_000,
});

export const tokensQuery = queryOptions({
  queryKey: ["tokens"],
  queryFn: () => api.tokens(),
});

export function refresh(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: overviewQuery.queryKey }),
    queryClient.invalidateQueries({ queryKey: eventsQuery.queryKey }),
  ]);
}
