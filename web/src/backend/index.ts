import { httpApi } from "../api/http";
import type { Api } from "../api/types";
import { demoApi } from "../demo/api";

/** The Zentrale, or the fixtures in demo mode (`vite --mode demo`). */
export const api: Api = import.meta.env.VITE_DEMO ? demoApi() : httpApi();
