import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Device, Live, Overview } from "../api/types";
import { AgentUpdate } from "./agent-update";

const api = vi.hoisted(() => ({ updateAgent: vi.fn() }));
vi.mock("../api", () => ({ api }));

const device = (patch: Partial<Device> = {}) =>
  ({
    id: "d1",
    os: "linux",
    agentVersion: "0.1.11",
    approved: true,
    online: true,
    ...patch,
  }) as Device;
const overview = (version = "0.1.12") => ({ version }) as Overview;
const live = (canUpdate?: boolean) =>
  ({ devices: { d1: { at: 1, folders: {}, canUpdate } } }) as unknown as Live;

const ui = (o: Overview, l: Live | undefined, d: Device, client: QueryClient) => (
  <QueryClientProvider client={client}>
    <AgentUpdate overview={o} live={l} device={d} />
  </QueryClientProvider>
);
const show = (o: Overview, l: Live | undefined, d: Device) =>
  render(ui(o, l, d, new QueryClient()));

describe("AgentUpdate", () => {
  afterEach(() => vi.clearAllMocks());

  it("links the manual steps where the docs have them", () => {
    show(overview(), live(false), device());
    expect(screen.getByRole("link").getAttribute("href")).toBe(
      "https://firsttris.github.io/stellwerk/installation.html#updating",
    );
  });

  it("promises no update from here to an offline agent that cannot do it", () => {
    show(overview(), live(false), device({ online: false }));
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText(/nicht selbst aktualisieren/)).toBeTruthy();
  });

  it("knows a Windows agent cannot update itself after the Zentrale restarted", () => {
    show(overview(), { devices: {} } as unknown as Live, device({ os: "windows", online: false }));
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.getByText(/nicht selbst aktualisieren/)).toBeTruthy();
  });

  it("still offers an offline agent that can update itself the update for later", () => {
    show(overview(), live(true), device({ online: false }));
    expect(screen.getByRole("button", { name: "Aktualisieren" })).toHaveProperty("disabled", true);
    expect(screen.getByRole("status").textContent).toMatch(/offline/);
  });
});
