import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";
import { overwriteGetLocale } from "./paraglide/runtime.js";

// The tests check German texts; jsdom would pick the language from its English navigator.
overwriteGetLocale(() => "de");

afterEach(cleanup);
