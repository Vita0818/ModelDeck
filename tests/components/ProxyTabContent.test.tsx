import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ProxyTabContent } from "@/components/settings/ProxyTabContent";
import { proxyApi } from "@/lib/api/proxy";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

const deckState = vi.hoisted(() => ({
  activeBindings: {} as Record<string, unknown>,
}));
vi.mock("@/lib/query/deck", () => ({
  useDeck: () => ({ data: { store: deckState } }),
}));
vi.mock("@/components/settings/GlobalProxySettings", () => ({
  GlobalProxySettings: () => <div>Upstream network</div>,
}));
vi.mock("@/lib/api/proxy", () => ({
  proxyApi: {
    getGlobalProxyConfig: vi.fn(),
    getProxyStatus: vi.fn(),
    updateGlobalProxyConfig: vi.fn(),
    startProxyServer: vi.fn(),
    stopProxyServer: vi.fn(),
  },
}));

function mount() {
  const cache = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  });
  return render(
    <QueryClientProvider client={cache}>
      <ProxyTabContent />
    </QueryClientProvider>,
  );
}

describe("ModelDeck gateway settings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    deckState.activeBindings = {};
    vi.mocked(proxyApi.getGlobalProxyConfig).mockResolvedValue({
      proxyEnabled: false,
      listenAddress: "127.0.0.1",
      listenPort: 15721,
      enableLogging: true,
    });
    vi.mocked(proxyApi.getProxyStatus).mockResolvedValue({
      running: false,
    } as Awaited<ReturnType<typeof proxyApi.getProxyStatus>>);
  });

  it("starts the gateway and saves a new listen port", async () => {
    const user = userEvent.setup();
    mount();
    const port = await screen.findByLabelText("settings.gateway.port");
    await waitFor(() => expect(port).toHaveValue(15721));
    await user.click(
      screen.getByRole("button", { name: "settings.gateway.start" }),
    );
    await waitFor(() =>
      expect(proxyApi.startProxyServer).toHaveBeenCalledOnce(),
    );
    await user.clear(port);
    await user.type(port, "15722");
    await user.click(screen.getByRole("button", { name: "common.save" }));
    await waitFor(() =>
      expect(proxyApi.updateGlobalProxyConfig).toHaveBeenCalledWith(
        expect.objectContaining({
          listenAddress: "127.0.0.1",
          listenPort: 15722,
        }),
      ),
    );
  });

  it("protects active native client bindings from a stop or address change", async () => {
    deckState.activeBindings = { codex: {} };
    vi.mocked(proxyApi.getProxyStatus).mockResolvedValue({
      running: true,
      address: "127.0.0.1",
      port: 15721,
    } as Awaited<ReturnType<typeof proxyApi.getProxyStatus>>);
    mount();
    expect(
      await screen.findByRole("button", { name: "settings.gateway.stop" }),
    ).toBeDisabled();
    expect(screen.getByLabelText("settings.gateway.address")).toBeDisabled();
    expect(screen.getByLabelText("settings.gateway.port")).toBeDisabled();
    expect(proxyApi.stopProxyServer).not.toHaveBeenCalled();
  });
});
