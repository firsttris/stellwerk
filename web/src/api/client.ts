import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query";
import { ApiError } from "./types";

/** A mutation whose 401 is an answer, not an ended session: the login itself. */
export const UNAUTHORIZED_IS_ANSWER = { unauthorizedIsAnswer: true } as const;

const MAX_RETRIES = 3;

/**
 * Retries only what may go away: network errors and server errors. A 4xx (not logged in, wrong
 * CSRF token, not found) gives the same answer again, so it is shown at once.
 */
export function shouldRetry(failureCount: number, error: Error): boolean {
  if (error instanceof ApiError && error.status < 500) return false;
  return failureCount < MAX_RETRIES;
}

const isUnauthorized = (error: Error) => error instanceof ApiError && error.status === 401;

/**
 * The query client for the app. Any 401 from a query or mutation means the session ended
 * (expired, or the password changed): `onUnauthorized` sends the admin to the login page.
 */
export function createQueryClient(onUnauthorized: () => void): QueryClient {
  return new QueryClient({
    queryCache: new QueryCache({
      onError: (error) => {
        if (isUnauthorized(error)) onUnauthorized();
      },
    }),
    mutationCache: new MutationCache({
      onError: (error, _variables, _context, mutation) => {
        if (isUnauthorized(error) && !mutation.meta?.unauthorizedIsAnswer) onUnauthorized();
      },
    }),
    defaultOptions: { queries: { retry: shouldRetry } },
  });
}
