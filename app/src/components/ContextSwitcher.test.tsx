import { render, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { ContextSwitcher } from "@/components/ContextSwitcher";
import { useOrchestratorStore } from "@/stores/useOrchestratorStore";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (_key: string, fallback?: string) => fallback ?? _key,
  }),
}));

describe("ContextSwitcher", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    Object.defineProperty(AbortSignal, "timeout", {
      configurable: true,
      value: vi.fn(() => new AbortController().signal),
    });
    window.localStorage.clear();
    useOrchestratorStore.setState({
      contexts: [],
      activeContextId: null,
      activeContext: null,
      url: null,
      info: null,
    });
  });

  it("checks the local previa-main on port 5056 before a context is saved", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ ok: false });
    vi.stubGlobal("fetch", fetchMock);

    render(<ContextSwitcher />);

    await waitFor(() => {
      expect(fetchMock).toHaveBeenCalledWith(
        "http://localhost:5056/health",
        expect.objectContaining({
          signal: expect.any(AbortSignal),
        }),
      );
    });
  });
});
