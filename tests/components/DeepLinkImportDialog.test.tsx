import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { DeepLinkImportDialog } from "@/components/DeepLinkImportDialog";
import { deeplinkApi } from "@/lib/api/deeplink";
import { emitTauriEvent } from "../msw/tauriMocks";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@/lib/api/deeplink", () => ({
  deeplinkApi: {
    importFromDeeplink: vi
      .fn()
      .mockResolvedValue({ type: "provider", id: "imported" }),
    mergeDeeplinkConfig: vi.fn(),
  },
}));
vi.mock("@/components/ui/dialog", () => ({
  Dialog: ({ open, children }: { open: boolean; children: React.ReactNode }) =>
    open ? <div>{children}</div> : null,
  DialogContent: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DialogHeader: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
  DialogTitle: ({ children }: { children: React.ReactNode }) => (
    <h1>{children}</h1>
  ),
  DialogDescription: ({ children }: { children: React.ReactNode }) => (
    <p>{children}</p>
  ),
  DialogFooter: ({ children }: { children: React.ReactNode }) => (
    <div>{children}</div>
  ),
}));
const request = {
  version: "v1",
  resource: "provider" as const,
  app: "codex" as const,
  name: "Imported model",
  endpoint: "https://models.example/v1",
  apiKey: "secret-model-key",
  model: "model-a",
};
function mount() {
  render(
    <QueryClientProvider client={new QueryClient()}>
      <DeepLinkImportDialog />
    </QueryClientProvider>,
  );
}

describe("model resource imports", () => {
  it("masks the key and imports resources only after a user action", async () => {
    const user = userEvent.setup();
    mount();
    act(() => emitTauriEvent("deeplink-import", request));
    expect(await screen.findByText("Imported model")).toBeInTheDocument();
    expect(screen.getByText("••••••••")).toBeInTheDocument();
    expect(screen.queryByText(request.apiKey)).not.toBeInTheDocument();
    expect(deeplinkApi.importFromDeeplink).not.toHaveBeenCalled();
    await user.click(
      screen.getByRole("button", { name: "deeplink.resourceImport" }),
    );
    await waitFor(() =>
      expect(deeplinkApi.importFromDeeplink).toHaveBeenCalledWith(request),
    );
  });

  it("previews remote configuration before permitting import", async () => {
    const user = userEvent.setup();
    vi.mocked(deeplinkApi.mergeDeeplinkConfig).mockResolvedValue(request);
    mount();
    const remote = {
      ...request,
      apiKey: undefined,
      configUrl: "https://models.example/config.json",
    };
    act(() => emitTauriEvent("deeplink-import", remote));
    const preview = await screen.findByRole("button", {
      name: "deeplink.resourcePreview",
    });
    expect(
      screen.queryByRole("button", { name: "deeplink.resourceImport" }),
    ).not.toBeInTheDocument();
    await user.click(preview);
    await waitFor(() =>
      expect(deeplinkApi.mergeDeeplinkConfig).toHaveBeenCalledWith(remote),
    );
    expect(
      await screen.findByRole("button", { name: "deeplink.resourceImport" }),
    ).toBeEnabled();
    expect(deeplinkApi.importFromDeeplink).not.toHaveBeenCalled();
  });
});
