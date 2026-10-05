import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Layers,
  Loader2,
  Network,
  RotateCcw,
  Save,
  Settings2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { MODELDECK_CLIENTS, type ClientId } from "@/config/modeldeckClients";
import {
  deckApi,
  type DeckStore,
  type DeckView,
  type HarnessBinding,
} from "@/lib/api/deck";
import ResourceEditor, { Field, selectStyle } from "./ResourceEditor";
import EndpointEditor from "./EndpointEditor";

const blankBinding = (): HarnessBinding => ({
  endpointId: "",
  model: "",
  utilityModel: "",
  executable: "",
});
const resourceData = (store: DeckStore) =>
  JSON.stringify([store.providers, store.models, store.endpoints]);

export default function DeckPanel({
  client,
  onBusyChange,
  visible = true,
}: {
  client: ClientId;
  onBusyChange: (busy: boolean) => void;
  visible?: boolean;
}) {
  const { t } = useTranslation();
  const [view, setView] = useState<DeckView>();
  const [draft, setDraft] = useState<DeckStore>();
  const [selections, setSelections] = useState<
    Partial<Record<ClientId, HarnessBinding>>
  >({});
  const [mode, setMode] = useState<"harness" | "resources">("harness");
  const [resourceTab, setResourceTab] = useState<
    "providers" | "endpoints" | "matrix"
  >("providers");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [command, setCommand] = useState("");
  useEffect(() => {
    onBusyChange(busy);
  }, [busy, onBusyChange]);
  useEffect(() => {
    let cancelled = false;
    deckApi
      .get()
      .then((next) => {
        if (!cancelled) {
          setView(next);
          setDraft(structuredClone(next.store));
          setSelections(next.store.bindings);
        }
      })
      .catch((e) => {
        if (!cancelled) setError(String(e));
      });
    return () => {
      cancelled = true;
    };
  }, []);
  useEffect(() => {
    setCommand("");
    setError("");
    setMode("harness");
  }, [client]);
  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };
  const accept = (next: DeckView, resources = false) => {
    setView(next);
    if (resources) setDraft(structuredClone(next.store));
  };
  const saveResources = () =>
    run(async () => {
      if (!view || !draft) return;
      const next = await deckApi.save({
        ...draft,
        revision: view.store.revision,
        bindings: view.store.bindings,
        activeBindings: view.store.activeBindings,
      });
      accept(next, true);
      toast.success(t("deck.resourcesSaved"));
    });
  const reload = () =>
    run(async () => {
      const next = await deckApi.get();
      accept(next, true);
      setSelections(next.store.bindings);
    });
  const binding = selections[client] ?? blankBinding();
  const update = (patch: Partial<HarnessBinding>) =>
    setSelections((previous) => ({
      ...previous,
      [client]: { ...binding, ...patch },
    }));
  const saveSelection = async () => {
    if (!view) throw new Error(t("deck.loading"));
    const bindings = { ...view.store.bindings };
    if (binding.endpointId) bindings[client] = binding;
    else delete bindings[client];
    const next = await deckApi.save({ ...view.store, bindings });
    accept(next);
    return next;
  };
  const saveBinding = () =>
    run(async () => {
      await saveSelection();
      toast.success(t("deck.selectionSaved"));
    });
  const applyBinding = () =>
    run(async () => {
      const saved = await saveSelection();
      const result = await deckApi.apply(client, saved.store.revision);
      accept(result.view);
      setCommand(result.command ?? "");
      toast.success(
        t(result.restartRequired ? "deck.appliedRestart" : "deck.applied"),
      );
    });
  const restoreBinding = () =>
    run(async () => {
      if (!view) return;
      accept(await deckApi.restore(client, view.store.revision));
      setCommand("");
      toast.success(t("deck.restored"));
    });
  if (!visible) return null;
  if (!view || !draft)
    return (
      <div className="px-7 py-6">
        <p role={error ? "alert" : "status"}>{error || t("deck.loading")}</p>
        {error && (
          <Button className="mt-3" variant="outline" onClick={reload}>
            {t("deck.reload")}
          </Button>
        )}
      </div>
    );
  const endpoints = Object.values(view.store.endpoints);
  const available = view.compatibility.filter((item) => item.client === client);
  const compatibility = available.find(
    (item) => item.endpointId === binding.endpointId,
  );
  const models = compatibility?.models ?? [];
  const active = view.store.activeBindings[client];
  const changed = resourceData(draft) !== resourceData(view.store);
  return (
    <div
      className="mx-auto w-full max-w-[1500px] px-7 pb-10"
      data-testid="deck-panel"
    >
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3 border-b pb-3">
        <div
          role="tablist"
          aria-label={t("deck.layers")}
          className="flex gap-1"
        >
          <Button
            role="tab"
            aria-selected={mode === "harness"}
            variant={mode === "harness" ? "secondary" : "ghost"}
            onClick={() => setMode("harness")}
            disabled={busy}
          >
            <Settings2 className="mr-2 h-4 w-4" />
            {t("deck.harnessLayer")}
          </Button>
          <Button
            role="tab"
            aria-selected={mode === "resources"}
            variant={mode === "resources" ? "secondary" : "ghost"}
            onClick={() => setMode("resources")}
            disabled={busy}
          >
            <Layers className="mr-2 h-4 w-4" />
            {t("deck.resourceLayer")}
          </Button>
        </div>
        <div className="text-xs text-muted-foreground">
          {t("deck.poolSummary", {
            providers: Object.keys(view.store.providers).length,
            models: Object.keys(view.store.models).length,
            endpoints: endpoints.length,
          })}
        </div>
      </div>
      {error && (
        <div
          role="alert"
          className="mb-5 rounded-md border border-destructive/40 bg-destructive/5 px-4 py-3 text-sm text-destructive"
        >
          {error}
        </div>
      )}
      {mode === "harness" ? (
        <section
          className="max-w-3xl space-y-6"
          aria-label={t("deck.harnessLayer")}
        >
          <div>
            <h2 className="text-xl font-semibold">
              {MODELDECK_CLIENTS.find((c) => c.id === client)?.name}
            </h2>
            <p className="mt-1 text-sm text-muted-foreground">
              {t("deck.bindingHint")}
            </p>
          </div>
          <p className="border-l-2 border-primary pl-3 text-sm">
            {active
              ? t("deck.activeSelection", {
                  endpoint: view.store.endpoints[active.endpointId]?.name,
                  model: active.model,
                })
              : t("deck.nativeSelection")}
          </p>
          {!endpoints.length && (
            <div className="space-y-3 rounded-md border p-5">
              <p className="text-sm">{t("deck.noEndpoints")}</p>
              <Button variant="outline" onClick={() => setMode("resources")}>
                {t("deck.configureResources")}
              </Button>
            </div>
          )}
          <fieldset disabled={busy} className="grid gap-5 sm:grid-cols-2">
            <Field label={t("deck.selectedEndpoint")}>
              <select
                className={selectStyle}
                value={binding.endpointId}
                onChange={(e) => {
                  const match = available.find(
                    (item) => item.endpointId === e.target.value,
                  );
                  update({
                    endpointId: e.target.value,
                    model: match?.models[0] ?? "",
                    utilityModel: "",
                  });
                }}
              >
                <option value="">{t("deck.noSelection")}</option>
                {endpoints.map((e) => {
                  const count =
                    available.find((item) => item.endpointId === e.id)?.models
                      .length ?? 0;
                  return (
                    <option key={e.id} value={e.id} disabled={!count}>
                      {e.name} · {t("deck.compatibleCount", { count })}
                    </option>
                  );
                })}
              </select>
            </Field>
            <Field label={t("deck.defaultModel")}>
              <select
                className={selectStyle}
                value={binding.model}
                disabled={!binding.endpointId}
                onChange={(e) => update({ model: e.target.value })}
              >
                <option value="">{t("deck.chooseModel")}</option>
                {models.map((alias) => (
                  <option key={alias} value={alias}>
                    {alias}
                  </option>
                ))}
              </select>
            </Field>
            <Field label={t("deck.utilityModel")}>
              <select
                className={selectStyle}
                value={binding.utilityModel}
                disabled={!binding.endpointId}
                onChange={(e) => update({ utilityModel: e.target.value })}
              >
                <option value="">{t("deck.sameAsDefault")}</option>
                {models.map((alias) => (
                  <option key={alias} value={alias}>
                    {alias}
                  </option>
                ))}
              </select>
            </Field>
            {["antigravity", "muse", "cursor-agent"].includes(client) && (
              <Field label={t("deck.executable")}>
                <Input
                  value={binding.executable}
                  placeholder={t("deck.autoDetectExecutable")}
                  onChange={(e) => update({ executable: e.target.value })}
                />
              </Field>
            )}
          </fieldset>
          {compatibility?.issue && (
            <p
              role="status"
              className="text-sm text-amber-600 dark:text-amber-400"
            >
              {compatibility.issue}
            </p>
          )}
          <div className="flex flex-wrap gap-2">
            <Button
              variant="outline"
              onClick={saveBinding}
              disabled={busy || (!!binding.endpointId && !binding.model)}
            >
              <Save className="mr-2 h-4 w-4" />
              {t("deck.saveSelection")}
            </Button>
            <Button
              onClick={applyBinding}
              disabled={busy || !binding.model || !!compatibility?.issue}
            >
              {busy ? (
                <Loader2 className="mr-2 h-4 w-4 animate-spin" />
              ) : (
                <Network className="mr-2 h-4 w-4" />
              )}
              {t("deck.saveAndApply")}
            </Button>
            {active && (
              <Button variant="ghost" onClick={restoreBinding} disabled={busy}>
                <RotateCcw className="mr-2 h-4 w-4" />
                {t("deck.restoreNative")}
              </Button>
            )}
          </div>
          {command && (
            <div className="space-y-2">
              <p className="text-sm text-muted-foreground">
                {t("deck.launchCommand")}
              </p>
              <code className="block break-all rounded-md bg-muted px-3 py-2 text-xs">
                {command}
              </code>
            </div>
          )}
        </section>
      ) : (
        <section aria-label={t("deck.resourceLayer")}>
          <div className="mb-5 flex flex-wrap items-center justify-between gap-3">
            <div>
              <h2 className="text-xl font-semibold">
                {t("deck.resourceLayer")}
              </h2>
              <p className="mt-1 text-sm text-muted-foreground">
                {t("deck.resourceHint")}
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="ghost" onClick={reload} disabled={busy}>
                {t("deck.reload")}
              </Button>
              <Button onClick={saveResources} disabled={busy || !changed}>
                {busy ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <Save className="mr-2 h-4 w-4" />
                )}
                {t("deck.saveResources")}
              </Button>
            </div>
          </div>
          <div
            className="mb-6 flex gap-1"
            role="tablist"
            aria-label={t("deck.resourceSections")}
          >
            {(["providers", "endpoints", "matrix"] as const).map((tab) => (
              <Button
                key={tab}
                role="tab"
                aria-selected={resourceTab === tab}
                variant={resourceTab === tab ? "secondary" : "ghost"}
                onClick={() => setResourceTab(tab)}
                disabled={busy}
              >
                {t(`deck.${tab}`)}
              </Button>
            ))}
          </div>
          <div className={resourceTab === "providers" ? "block" : "hidden"}>
            <ResourceEditor store={draft} onChange={setDraft} disabled={busy} />
          </div>
          <div className={resourceTab === "endpoints" ? "block" : "hidden"}>
            <EndpointEditor
              store={draft}
              origin={view.origin}
              onChange={setDraft}
              disabled={busy}
            />
          </div>
          {resourceTab === "matrix" && (
            <div className="overflow-auto">
              <p className="mb-4 text-sm text-muted-foreground">
                {t("deck.matrixHint")}
              </p>
              <table className="w-full min-w-[1000px] text-left text-sm">
                <thead>
                  <tr className="border-b">
                    <th className="px-3 py-3 font-medium">
                      {t("deck.endpoints")}
                    </th>
                    {MODELDECK_CLIENTS.map((c) => (
                      <th
                        key={c.id}
                        className="px-3 py-3 text-center text-xs font-medium"
                      >
                        {c.name}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {endpoints.map((e) => (
                    <tr key={e.id} className="border-b">
                      <th className="px-3 py-3 font-medium">{e.name}</th>
                      {MODELDECK_CLIENTS.map((c) => {
                        const item = view.compatibility.find(
                          (v) => v.client === c.id && v.endpointId === e.id,
                        );
                        return (
                          <td
                            key={c.id}
                            className={`px-3 py-3 text-center font-mono text-xs ${item?.issue ? "text-muted-foreground" : "bg-primary/5 text-primary"}`}
                            title={
                              item?.issue ??
                              t("deck.compatibleCount", {
                                count: item?.models.length ?? 0,
                              })
                            }
                          >
                            {item?.models.length
                              ? `${item.models.length}${item.issue ? " *" : ""}`
                              : "—"}
                          </td>
                        );
                      })}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
          {view.store.migration.retained.length > 0 && (
            <details className="mt-6 border-t pt-4 text-sm text-muted-foreground">
              <summary className="cursor-pointer">
                {t("deck.retainedConfigs", {
                  count: view.store.migration.retained.length,
                })}
              </summary>
              <ul className="mt-2 space-y-1">
                {view.store.migration.retained.map((item, i) => (
                  <li key={i}>{item}</li>
                ))}
              </ul>
            </details>
          )}
        </section>
      )}
    </div>
  );
}
