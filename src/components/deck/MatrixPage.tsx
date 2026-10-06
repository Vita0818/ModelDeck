import { useTranslation } from "react-i18next";
import { Loader2, TriangleAlert } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Group, GroupFooter, GroupTitle } from "@/components/ui/group";
import { Page, EmptyState } from "@/components/app/Page";
import { useDeck } from "@/lib/query/deck";
import { extractErrorMessage } from "@/utils/errorUtils";
import { MODELDECK_CLIENTS } from "@/config/modeldeckClients";

/**
 * Compatibility matrix.
 *
 * The counts come from the backend, which already resolves protocol
 * conversion and per-client restrictions, so this view never recomputes them.
 */
export default function MatrixPage() {
  const { t } = useTranslation();
  const { data: view, isLoading, error, refetch } = useDeck();

  if (isLoading) {
    return (
      <Page title={t("nav.resource.matrix")}>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          {t("deck.loading")}
        </div>
      </Page>
    );
  }

  if (error || !view) {
    return (
      <Page title={t("nav.resource.matrix")}>
        <EmptyState
          title={t("deck.loadFailed")}
          description={extractErrorMessage(error)}
          action={
            <Button variant="outline" onClick={() => void refetch()}>
              {t("deck.reload")}
            </Button>
          }
        />
      </Page>
    );
  }

  const endpoints = Object.values(view.store.endpoints);

  return (
    <Page
      title={t("nav.resource.matrix")}
      description={t("deck.matrixDescription")}
      bodyClassName="px-7"
    >
      {!endpoints.length ? (
        <EmptyState title={t("deck.noEndpoints")} />
      ) : (
        <section>
          <GroupTitle>{t("deck.compatibleModels")}</GroupTitle>
          <Group className="[&>*+*]:border-t-0">
            <div className="overflow-x-auto">
              <table className="w-full border-collapse text-base">
                <thead>
                  <tr className="border-b border-separator">
                    <th className="sticky left-0 bg-card px-3.5 py-2.5 text-left text-sm font-medium text-muted-foreground">
                      {t("deck.endpoint")}
                    </th>
                    {MODELDECK_CLIENTS.map((client) => (
                      <th
                        key={client.id}
                        className="whitespace-nowrap px-3 py-2.5 text-center text-sm font-medium text-muted-foreground"
                      >
                        {client.name}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {endpoints.map((endpoint) => (
                    <tr
                      key={endpoint.id}
                      className="border-b border-separator last:border-b-0"
                    >
                      <td className="sticky left-0 max-w-48 truncate bg-card px-3.5 py-2.5 font-medium">
                        {endpoint.name}
                      </td>
                      {MODELDECK_CLIENTS.map((client) => {
                        const cell = view.compatibility.find(
                          (c) =>
                            c.client === client.id &&
                            c.endpointId === endpoint.id,
                        );
                        const count = cell?.models.length ?? 0;
                        return (
                          <td
                            key={client.id}
                            className="px-3 py-2.5 text-center"
                          >
                            {cell?.issue ? (
                              <span
                                title={cell.issue}
                                className="inline-flex items-center gap-1 text-warning"
                              >
                                <TriangleAlert className="size-3.5" />
                                {count || "—"}
                              </span>
                            ) : count ? (
                              <Badge variant="success">{count}</Badge>
                            ) : (
                              <span className="text-muted-foreground/50">
                                —
                              </span>
                            )}
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </Group>
          <GroupFooter>{t("deck.matrixHint")}</GroupFooter>
        </section>
      )}

      {view.store.migration.retained.length > 0 && (
        <section>
          <GroupTitle>{t("deck.retainedConfigs")}</GroupTitle>
          <Group>
            {view.store.migration.retained.map((item) => (
              <div key={item} className="px-3.5 py-2.5 font-mono text-sm">
                {item}
              </div>
            ))}
          </Group>
          <GroupFooter>{t("deck.retainedHint")}</GroupFooter>
        </section>
      )}
    </Page>
  );
}
