import { getLocale } from "../paraglide/runtime.js";

/** The locale for Intl number and date formats. */
export function intlLocale(): string {
  return getLocale() === "en" ? "en-GB" : "de-DE";
}

/** Picks the singular or plural message. */
export function byCount<T>(count: number, one: () => T, other: () => T): T {
  return count === 1 ? one() : other();
}
