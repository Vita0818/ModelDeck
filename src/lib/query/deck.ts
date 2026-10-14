import {
  useMutation,
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import { deckApi, type DeckStore, type DeckView } from "@/lib/api/deck";
import type { ClientId } from "@/config/modeldeckClients";
import { proxyKeys } from "@/lib/query/proxy";

export const deckKeys = {
  all: ["deck"] as const,
  view: ["deck", "view"] as const,
};

/**
 * Applying a binding starts the gateway and rewrites native config, so the
 * proxy and settings caches are no longer accurate afterwards.
 */
async function invalidateRuntime(queryClient: QueryClient) {
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: proxyKeys.status }),
    queryClient.invalidateQueries({ queryKey: proxyKeys.globalConfig }),
    queryClient.invalidateQueries({ queryKey: ["settings"] }),
  ]);
}

export function useDeck() {
  return useQuery({
    queryKey: deckKeys.view,
    queryFn: () => deckApi.get(),
    staleTime: 30_000,
  });
}

/** Writes the resource graph. The backend rejects a stale revision. */
export function useSaveDeck() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (store: DeckStore) => deckApi.save(store),
    onSuccess: (view) => queryClient.setQueryData(deckKeys.view, view),
  });
}

export function useApplyBinding() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      client,
      revision,
    }: {
      client: ClientId;
      revision: number;
    }) => deckApi.apply(client, revision),
    onSuccess: async (result) => {
      queryClient.setQueryData<DeckView>(deckKeys.view, result.view);
      await invalidateRuntime(queryClient);
    },
  });
}

export function useRestoreBinding() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      client,
      revision,
    }: {
      client: ClientId;
      revision: number;
    }) => deckApi.restore(client, revision),
    onSuccess: async (view) => {
      queryClient.setQueryData(deckKeys.view, view);
      await invalidateRuntime(queryClient);
    },
  });
}
