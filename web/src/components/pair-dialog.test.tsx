import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PairDialog } from "./pair-dialog";

const api = vi.hoisted(() => ({
  createEnrollment: vi.fn(),
  enrollmentStatus: vi.fn(),
}));
vi.mock("../api", () => ({ api }));

const MINUTE = 60_000;

describe("PairDialog", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    api.createEnrollment.mockImplementation(async () => ({
      code: `CODE-${api.createEnrollment.mock.calls.length}`,
      expiresAt: Math.floor(Date.now() / 1000) + 15 * 60,
    }));
    api.enrollmentStatus.mockResolvedValue({ device: null });
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  const open = async () => {
    const user = userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
    render(
      <QueryClientProvider client={new QueryClient()}>
        <PairDialog />
      </QueryClientProvider>,
    );
    await user.click(screen.getByRole("button", { name: "Gerät koppeln" }));
    expect(await screen.findByText(/CODE-1/)).toBeTruthy();
    return user;
  };

  it("says copied only when the clipboard took the command", async () => {
    const user = await open();
    await user.click(screen.getByRole("button", { name: "Befehl kopieren" }));
    expect(await navigator.clipboard.readText()).toMatch(/join --server .* CODE-1/);
    expect(await screen.findByText("Kopiert")).toBeTruthy();

    // A plain-HTTP origin has no clipboard API: the command is selected instead.
    Object.defineProperty(navigator, "clipboard", { value: undefined, configurable: true });
    await act(() => vi.advanceTimersByTimeAsync(2_000));
    await user.click(screen.getByRole("button", { name: "Befehl kopieren" }));
    expect(await screen.findByText("Markiert – jetzt kopieren")).toBeTruthy();
    expect(window.getSelection()?.toString()).toMatch(/CODE-1/);
  });

  it("stops waiting when the code expires and offers a new one", async () => {
    const user = await open();
    expect(screen.getByText("2. Warte auf das Gerät …")).toBeTruthy();

    await act(() => vi.advanceTimersByTimeAsync(15 * MINUTE));
    expect(screen.getByText("Der Code ist abgelaufen.")).toBeTruthy();
    const polls = api.enrollmentStatus.mock.calls.length;
    await act(() => vi.advanceTimersByTimeAsync(MINUTE));
    expect(api.enrollmentStatus.mock.calls.length).toBe(polls);

    await user.click(screen.getByRole("button", { name: "Neuen Code erzeugen" }));
    expect(await screen.findByText(/CODE-2/)).toBeTruthy();
    expect(screen.getByText("2. Warte auf das Gerät …")).toBeTruthy();
  });
});
