import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Plus, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  PROTOCOLS,
  PROTOCOL_LABELS,
  type DeckStore,
  type ExposedEndpoint,
} from "@/lib/api/deck";
import { Field, newId, selectStyle } from "./ResourceEditor";

export default function EndpointEditor({
  store,
  origin,
  onChange,
  disabled,
}: {
  store: DeckStore;
  origin: string;
  onChange: (store: DeckStore) => void;
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState("");
  const [newAlias, setNewAlias] = useState("");
  const endpoints = Object.values(store.endpoints);
  const endpoint = store.endpoints[selected] ?? endpoints[0];
  const models = Object.values(store.models);
  const update = (patch: Partial<ExposedEndpoint>) => {
    if (endpoint)
      onChange({
        ...store,
        endpoints: {
          ...store.endpoints,
          [endpoint.id]: { ...endpoint, ...patch },
        },
      });
  };
  const addEndpoint = () => {
    const id = newId();
    onChange({
      ...store,
      endpoints: {
        ...store.endpoints,
        [id]: {
          id,
          name: t("deck.newEndpoint"),
          protocols: [...PROTOCOLS],
          publicUrl: "",
          token: "",
          routes: {},
        },
      },
    });
    setSelected(id);
  };
  const deleteEndpoint = () => {
    if (!endpoint) return;
    if (
      [
        ...Object.values(store.bindings),
        ...Object.values(store.activeBindings),
      ].some((b) => b?.endpointId === endpoint.id)
    ) {
      toast.error(t("deck.endpointInUse"));
      return;
    }
    const next = { ...store.endpoints };
    delete next[endpoint.id];
    onChange({ ...store, endpoints: next });
    setSelected("");
  };
  const addRoute = () => {
    const alias = newAlias.trim();
    if (!endpoint || !models.length || !alias) return;
    if (endpoint.routes[alias]) {
      toast.error(t("deck.aliasExists"));
      return;
    }
    update({
      routes: {
        ...endpoint.routes,
        [alias]: { modelId: models[0].id, effort: "" },
      },
    });
    setNewAlias("");
  };
  return (
    <div className="grid gap-6 lg:grid-cols-[230px_minmax(0,1fr)]">
      <aside className="space-y-2 border-b pb-4 lg:border-b-0 lg:border-r lg:pr-5">
        <div className="mb-3 flex items-center justify-between">
          <h3 className="font-medium">{t("deck.endpoints")}</h3>
          <Button
            variant="ghost"
            size="icon"
            onClick={addEndpoint}
            disabled={disabled}
            aria-label={t("deck.addEndpoint")}
          >
            <Plus className="h-4 w-4" />
          </Button>
        </div>
        {endpoints.map((e) => (
          <button
            key={e.id}
            type="button"
            disabled={disabled}
            onClick={() => setSelected(e.id)}
            className={`w-full rounded-md px-3 py-2.5 text-left focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring ${e.id === endpoint?.id ? "bg-primary/10 text-primary" : "hover:bg-muted"}`}
          >
            <span className="block truncate text-sm font-medium">{e.name}</span>
            <span className="block text-xs text-muted-foreground">
              {Object.keys(e.routes).length} {t("deck.publicModelsCount")}
            </span>
          </button>
        ))}
        {!endpoints.length && (
          <p className="text-sm text-muted-foreground">
            {t("deck.addFirstEndpoint")}
          </p>
        )}
      </aside>
      {endpoint && (
        <fieldset disabled={disabled} className="min-w-0 space-y-5">
          <div className="flex items-center justify-between">
            <h3 className="text-base font-semibold">{endpoint.name}</h3>
            <Button variant="ghost" size="sm" onClick={deleteEndpoint}>
              <Trash2 className="mr-2 h-4 w-4" />
              {t("deck.deleteEndpoint")}
            </Button>
          </div>
          <div className="grid gap-4 sm:grid-cols-2">
            <Field label={t("deck.endpointName")}>
              <Input
                value={endpoint.name}
                onChange={(e) => update({ name: e.target.value })}
              />
            </Field>
            <Field label={t("deck.publicUrl")}>
              <Input
                value={endpoint.publicUrl}
                placeholder={`https://gateway.example/deck/${endpoint.id}`}
                onChange={(e) => update({ publicUrl: e.target.value })}
              />
            </Field>
          </div>
          <div>
            <p className="mb-1 text-sm text-muted-foreground">
              {t("deck.localUrl")}
            </p>
            <code className="break-all text-xs">
              {origin}/deck/{endpoint.id}
            </code>
          </div>
          <div className="space-y-2">
            <p className="text-sm text-muted-foreground">
              {t("deck.exposedProtocols")}
            </p>
            <div className="flex flex-wrap gap-4">
              {PROTOCOLS.map((p) => (
                <label key={p} className="flex items-center gap-2 text-sm">
                  <input
                    type="checkbox"
                    checked={endpoint.protocols.includes(p)}
                    onChange={(e) =>
                      update({
                        protocols: e.target.checked
                          ? [...endpoint.protocols, p]
                          : endpoint.protocols.filter((v) => v !== p),
                      })
                    }
                  />
                  {PROTOCOL_LABELS[p]}
                </label>
              ))}
            </div>
          </div>
          <section className="border-t pt-5">
            <h3 className="mb-1 font-medium">{t("deck.routes")}</h3>
            <p className="mb-4 text-sm text-muted-foreground">
              {t("deck.routeHint")}
            </p>
            <div className="space-y-3">
              {Object.entries(endpoint.routes).map(([alias, route]) => {
                const model = store.models[route.modelId];
                return (
                  <div
                    key={alias}
                    className="grid items-end gap-3 border-b pb-3 sm:grid-cols-[minmax(120px,1fr)_minmax(160px,2fr)_130px_36px]"
                  >
                    <Field label={t("deck.publicModel")}>
                      <Input readOnly value={alias} />
                    </Field>
                    <Field label={t("deck.targetResource")}>
                      <select
                        className={selectStyle}
                        value={route.modelId}
                        onChange={(e) =>
                          update({
                            routes: {
                              ...endpoint.routes,
                              [alias]: { modelId: e.target.value, effort: "" },
                            },
                          })
                        }
                      >
                        {models.map((m) => (
                          <option key={m.id} value={m.id}>
                            {store.providers[m.providerId]?.name} / {m.name}
                          </option>
                        ))}
                      </select>
                    </Field>
                    <Field label={t("deck.tier")}>
                      <select
                        className={selectStyle}
                        value={route.effort}
                        onChange={(e) =>
                          update({
                            routes: {
                              ...endpoint.routes,
                              [alias]: { ...route, effort: e.target.value },
                            },
                          })
                        }
                      >
                        <option value="">{t("deck.providerDefault")}</option>
                        {model?.reasoning.map((level) => (
                          <option key={level} value={level}>
                            {level}
                          </option>
                        ))}
                      </select>
                    </Field>
                    <Button
                      variant="ghost"
                      size="icon"
                      aria-label={t("deck.deleteRoute", { model: alias })}
                      onClick={() => {
                        const routes = { ...endpoint.routes };
                        delete routes[alias];
                        update({ routes });
                      }}
                    >
                      <Trash2 className="h-4 w-4" />
                    </Button>
                  </div>
                );
              })}
            </div>
            <div className="mt-4 flex max-w-lg gap-2">
              <Input
                aria-label={t("deck.newPublicModel")}
                placeholder={t("deck.aliasPlaceholder")}
                value={newAlias}
                onChange={(e) => setNewAlias(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    addRoute();
                  }
                }}
              />
              <Button
                variant="outline"
                disabled={!models.length || !newAlias.trim()}
                onClick={addRoute}
              >
                <Plus className="mr-2 h-4 w-4" />
                {t("deck.addRoute")}
              </Button>
            </div>
            {!models.length && (
              <p className="mt-2 text-sm text-muted-foreground">
                {t("deck.addModelsBeforeRoutes")}
              </p>
            )}
          </section>
        </fieldset>
      )}
    </div>
  );
}
