import { ZodError } from "zod";
import { ApiError } from "../api/types";
import { m } from "../paraglide/messages.js";

// What went wrong, in the UI's language. The Zentrale's own texts are English; the cases a user
// can do something about are recognised by status, the rest keeps the server's detail.

/** A failed request or mutation, for the place where it happened. */
export function errorMessage(error: Error): string {
  if (error instanceof ApiError) {
    if (error.status === 401) return m.error_not_logged_in();
    if (error.status === 403) return m.error_forbidden();
    if (error.status >= 500) return m.error_server({ status: error.status });
    return error.message;
  }
  // fetch rejects with a TypeError when the server cannot be reached at all.
  if (error instanceof TypeError) return m.error_network();
  if (error instanceof ZodError) return m.error_unexpected_answer();
  return error.message;
}

/** A failed login: wrong password and "no password yet" are answers, not failures. */
export function loginErrorMessage(error: Error): string {
  if (error instanceof ApiError) {
    if (error.status === 401) return m.login_error_wrong_password();
    if (error.status === 409) return m.login_error_no_password();
  }
  return errorMessage(error);
}
