import { demoApi } from "../demo/api";
import { httpApi } from "./http";
import type { Api } from "./types";

/** The Zentrale, or the fixtures in demo mode (`vite --mode demo`). */
export const api: Api = import.meta.env.VITE_DEMO ? demoApi() : httpApi();
