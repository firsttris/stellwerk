import { type QueryClient, queryOptions } from "@tanstack/react-query";
import { api } from ".";

// Query keys and options in one place; mutations invalidate through `refresh`.

export const sessionQuery = queryOptions({
  queryKey: ["session"],
  queryFn: () => api.session(),
  staleTime: 60_000,
});

/** Polled, so online states and "waiting" cells follow the agents. */
export const overviewQuery = queryOptions({
  queryKey: ["overview"],
  queryFn: () => api.overview(),
  refetchInterval: 5_000,
});

export const eventsQuery = queryOptions({
  queryKey: ["events"],
  queryFn: () => api.events(8),
  refetchInterval: 15_000,
});

export function refresh(queryClient: QueryClient) {
  return Promise.all([
    queryClient.invalidateQueries({ queryKey: overviewQuery.queryKey }),
    queryClient.invalidateQueries({ queryKey: eventsQuery.queryKey }),
  ]);
}
