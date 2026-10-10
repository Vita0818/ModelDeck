import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Boxes,
  BookMarked,
  ChevronLeft,
  Eye,
  EyeOff,
  Loader2,
  Plus,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Group,
  GroupFooter,
  GroupTitle,
  FieldRow,
  Row,
} from "@/components/ui/group";
import { Page, EmptyState } from "@/components/app/Page";
import {
  PROTOCOLS,
  PROTOCOL_LABELS,
  type ModelResource,
  type Protocol,
} from "@/lib/api/deck";
import type { DeckProviderPreset } from "@/config/deckProviderPresets";
import PresetPicker from "./PresetPicker";
import { newId, useResourceDraft } from "./useResourceDraft";
import { extractErrorMessage } from "@/utils/errorUtils";
import { ResourceToolbar } from "./ResourceToolbar";

export default function ProvidersPage() {
  const { t } = useTranslation();
  const state = useResourceDraft();
  const { draft, setDraft } = state;
  const [selectedId, setSelectedId] = useState("");
  const [picking, setPicking] = useState(false);
  const [revealKey, setRevealKey] = useState(false);

  const providers = useMemo(
    () => Object.values(draft?.providers ?? {}),
    [draft],
  );
  const provider =
    (selectedId ? draft?.providers[selectedId] : undefined) ?? providers[0];
  const models = useMemo(
    () =>
      Object.values(draft?.models ?? {}).filter(
        (m) => m.providerId === provider?.id,
      ),
    [draft, provider],
  );

  if (state.isLoading) {
    return (
      <Page title={t("nav.resource.providers")}>
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="size-4 animate-spin" />
          {t("deck.loading")}
        </div>
      </Page>
    );
  }
  if (state.error || !draft) {
    return (
      <Page title={t("nav.resource.providers")}>
        <EmptyState
          title={t("deck.loadFailed")}
          description={extractErrorMessage(state.error)}
          action={
            <Button variant="outline" onClick={() => void state.refetch()}>
              {t("deck.reload")}
            </Button>
          }
        />
      </Page>
    );
  }

  const patchProvider = (patch: Partial<(typeof providers)[number]>) => {
    if (!provider) return;
    setDraft({
      ...draft,
      providers: {
        ...draft.providers,
        [provider.id]: { ...provider, ...patch },
      },
    });
  };

  const patchModel = (id: string, patch: Partial<ModelResource>) => {
    const model = draft.models[id];
    if (!model) return;
    setDraft({
      ...draft,
      models: { ...draft.models, [id]: { ...model, ...patch } },
    });
  };

  const addProvider = () => {
    const id = newId();
    setDraft({
      ...draft,
      providers: {
        ...draft.providers,
        [id]: {
          id,
          name: t("deck.newProvider"),
          baseUrl: "",
          protocol: "openai-chat",
          apiKey: "",
          keyEnv: "",
        },
      },
    });
    setSelectedId(id);
    setPicking(false);
  };

  const applyPreset = (preset: DeckProviderPreset) => {
    const providerId = newId();
    const seeded: Record<string, ModelResource> = {};
    for (const m of preset.models) {
      const id = newId();
      seeded[id] = {
        id,
        providerId,
        upstreamId: m.id,
        name: m.name,
        // Unknown limits stay at 0 so validation asks rather than guessing a
        // value that would silently cap requests.
        context: m.context ?? 0,
        output: m.output ?? 0,
        reasoning: [],
        reasoningSupported: m.reasoning ?? null,
        tools: true,
        images: m.images ?? false,
      };
    }
    setDraft({
      ...draft,
      providers: {
        ...draft.providers,
        [providerId]: {
          id: providerId,
          name: preset.name,
          baseUrl: preset.baseUrl,
          protocol: preset.protocol,
          apiKey: "",
          keyEnv: "",
        },
      },
      models: { ...draft.models, ...seeded },
    });
    setSelectedId(providerId);
    setPicking(false);
  };

  /** Endpoints reference models, so a referenced resource cannot be removed. */
  const referencedModels = (ids: string[]) =>
    Object.values(draft.endpoints).some((endpoint) =>
      Object.values(endpoint.routes).some((route) =>
        ids.includes(route.modelId),
      ),
    );

  const deleteProvider = () => {
    if (!provider) return;
    const owned = Object.values(draft.models)
      .filter((m) => m.providerId === provider.id)
      .map((m) => m.id);
    if (referencedModels(owned)) {
      toast.error(t("deck.resourceInUse"));
      return;
    }
    const nextProviders = { ...draft.providers };
    delete nextProviders[provider.id];
    const nextModels = { ...draft.models };
    for (const id of owned) delete nextModels[id];
    setDraft({ ...draft, providers: nextProviders, models: nextModels });
    setSelectedId("");
  };

  const addModel = () => {
    if (!provider) return;
    const id = newId();
    setDraft({
      ...draft,
      models: {
        ...draft.models,
        [id]: {
          id,
          providerId: provider.id,
          upstreamId: "",
          name: t("deck.newModel"),
          context: 0,
          output: 0,
          reasoning: [],
          reasoningSupported: null,
          tools: true,
          images: false,
        },
      },
    });
  };

  const deleteModel = (id: string) => {
    if (referencedModels([id])) {
      toast.error(t("deck.resourceInUse"));
      return;
    }
    const next = { ...draft.models };
    delete next[id];
    setDraft({ ...draft, models: next });
  };

  // ---- Preset picker takes over the page -----------------------------------
  if (picking) {
    return (
      <Page
        title={t("deck.addFromPreset")}
        description={t("deck.presetDescription")}
        actions={
          <Button variant="ghost" onClick={() => setPicking(false)}>
            <ChevronLeft />
            {t("common.back")}
          </Button>
        }
      >
        <PresetPicker onPick={applyPreset} onCancel={() => setPicking(false)} />
      </Page>
    );
  }

  // ---- List view ------------------------------------------------------------
  if (!provider) {
    return (
      <Page
        title={t("nav.resource.providers")}
        description={t("deck.providersDescription")}
      >
        <EmptyState
          icon={<Boxes className="size-5" />}
          title={t("deck.addFirstProvider")}
          description={t("deck.addFirstProviderHint")}
          action={
            <div className="flex gap-2">
              <Button onClick={() => setPicking(true)}>
                <BookMarked />
                {t("deck.addFromPreset")}
              </Button>
              <Button variant="outline" onClick={addProvider}>
                <Plus />
                {t("deck.addProvider")}
              </Button>
            </div>
          }
        />
      </Page>
    );
  }

  const usesEnv = provider.keySource
    ? provider.keySource === "env"
    : Boolean(provider.keyEnv);

  return (
    <Page
      title={t("nav.resource.providers")}
      description={t("deck.providersDescription")}
      actions={
        <>
          <Button variant="ghost" onClick={() => setPicking(true)}>
            <BookMarked />
            {t("deck.addFromPreset")}
          </Button>
          <Button variant="outline" onClick={addProvider}>
            <Plus />
            {t("deck.addProvider")}
          </Button>
        </>
      }
    >
      <ResourceToolbar {...state} />

      <section>
        <GroupTitle>{t("deck.providers")}</GroupTitle>
        <Group>
          {providers.map((p) => (
            <Row
              key={p.id}
              label={p.name}
              description={p.baseUrl || t("deck.noEndpointUrl")}
              trailing={
                <Badge variant={p.id === provider.id ? "default" : "secondary"}>
                  {
                    Object.values(draft.models).filter(
                      (m) => m.providerId === p.id,
                    ).length
                  }
                </Badge>
              }
              onSelect={() => {
                setSelectedId(p.id);
                setRevealKey(false);
              }}
              className={p.id === provider.id ? "bg-primary/[0.07]" : undefined}
            />
          ))}
        </Group>
      </section>

      <section>
        <div className="flex items-end justify-between">
          <GroupTitle>{provider.name}</GroupTitle>
          <Button
            variant="ghost"
            size="sm"
            className="mb-1.5 text-destructive hover:bg-destructive/10 hover:text-destructive"
            onClick={deleteProvider}
          >
            <Trash2 />
            {t("deck.deleteProvider")}
          </Button>
        </div>
        <Group>
          <FieldRow label={t("deck.providerName")}>
            <Input
              value={provider.name}
              onChange={(e) => patchProvider({ name: e.target.value })}
            />
          </FieldRow>
          <FieldRow label={t("deck.upstreamProtocol")}>
            <Select
              value={provider.protocol}
              onValueChange={(value) =>
                patchProvider({ protocol: value as Protocol })
              }
            >
              <SelectTrigger aria-label={t("deck.upstreamProtocol")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {PROTOCOLS.map((p) => (
                  <SelectItem key={p} value={p}>
                    {PROTOCOL_LABELS[p]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </FieldRow>
          <FieldRow
            label={t("deck.upstreamUrl")}
            hint={
              provider.baseUrl && !/^https?:\/\//.test(provider.baseUrl)
                ? t("deck.urlMustBeAbsolute")
                : undefined
            }
          >
            <Input
              value={provider.baseUrl}
              onChange={(e) => patchProvider({ baseUrl: e.target.value })}
              placeholder="https://api.example.com/v1"
            />
          </FieldRow>
          <FieldRow
            label={t("deck.credentialSource")}
            hint={usesEnv ? t("deck.envKeyHint") : t("deck.storedKeyHint")}
          >
            <Select
              value={usesEnv ? "env" : "stored"}
              onValueChange={(value) =>
                patchProvider(
                  value === "env"
                    ? { keySource: "env", apiKey: "" }
                    : { keySource: "stored", keyEnv: "" },
                )
              }
            >
              <SelectTrigger aria-label={t("deck.credentialSource")}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="stored">{t("deck.storedKey")}</SelectItem>
                <SelectItem value="env">{t("deck.environmentKey")}</SelectItem>
              </SelectContent>
            </Select>
          </FieldRow>
          {usesEnv ? (
            <FieldRow label={t("deck.keyVariable")}>
              <Input
                value={provider.keyEnv}
                onChange={(e) => patchProvider({ keyEnv: e.target.value })}
                placeholder="OPENAI_API_KEY"
                className="font-mono"
              />
            </FieldRow>
          ) : (
            <FieldRow label={t("deck.apiKey")}>
              <div className="relative">
                <Input
                  type={revealKey ? "text" : "password"}
                  value={provider.apiKey}
                  onChange={(e) => patchProvider({ apiKey: e.target.value })}
                  placeholder="sk-..."
                  className="pr-9 font-mono"
                />
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  aria-label={t(revealKey ? "common.hide" : "common.show")}
                  className="absolute right-0.5 top-0.5"
                  onClick={() => setRevealKey((v) => !v)}
                >
                  {revealKey ? <EyeOff /> : <Eye />}
                </Button>
              </div>
            </FieldRow>
          )}
        </Group>
      </section>

      <section>
        <div className="flex items-end justify-between">
          <GroupTitle>{t("deck.modelResources")}</GroupTitle>
          <Button
            variant="ghost"
            size="sm"
            className="mb-1.5"
            onClick={addModel}
          >
            <Plus />
            {t("deck.addModel")}
          </Button>
        </div>
        {models.length ? (
          <Group>
            {models.map((model) => (
              <ModelEditor
                key={model.id}
                model={model}
                onChange={(patch) => patchModel(model.id, patch)}
                onDelete={() => deleteModel(model.id)}
              />
            ))}
          </Group>
        ) : (
          <Group>
            <div className="px-3.5 py-6 text-center text-sm text-muted-foreground">
              {t("deck.noModels")}
            </div>
          </Group>
        )}
        <GroupFooter>{t("deck.modelLimitsHint")}</GroupFooter>
      </section>
    </Page>
  );
}

function ModelEditor({
  model,
  onChange,
  onDelete,
}: {
  model: ModelResource;
  onChange: (patch: Partial<ModelResource>) => void;
  onDelete: () => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const incomplete = !model.upstreamId || !model.context || !model.output;

  return (
    <div className="px-3.5 py-3">
      <div className="flex items-center gap-3">
        <button
          type="button"
          onClick={() => setOpen((v) => !v)}
          className="flex min-w-0 flex-1 flex-col items-start gap-0.5 text-left focus-visible:outline-none"
        >
          <span className="truncate text-base text-foreground">
            {model.name || t("deck.newModel")}
          </span>
          <span className="truncate font-mono text-sm text-muted-foreground">
            {model.upstreamId || t("deck.modelIdMissing")}
          </span>
        </button>
        {incomplete && <Badge variant="warning">{t("deck.incomplete")}</Badge>}
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={t("deck.deleteModel")}
          onClick={onDelete}
          className="text-muted-foreground hover:text-destructive"
        >
          <Trash2 />
        </Button>
      </div>

      {open && (
        <div className="mt-3 grid gap-3 border-t border-separator pt-3 sm:grid-cols-2">
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="text-muted-foreground">{t("deck.modelId")}</span>
            <Input
              value={model.upstreamId}
              onChange={(e) => onChange({ upstreamId: e.target.value })}
              className="font-mono"
            />
          </label>
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="text-muted-foreground">{t("deck.modelName")}</span>
            <Input
              value={model.name}
              onChange={(e) => onChange({ name: e.target.value })}
            />
          </label>
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="text-muted-foreground">{t("deck.context")}</span>
            <Input
              type="number"
              min={0}
              value={model.context || ""}
              onChange={(e) =>
                onChange({ context: Number(e.target.value) || 0 })
              }
            />
          </label>
          <label className="flex flex-col gap-1.5 text-sm">
            <span className="text-muted-foreground">{t("deck.maxOutput")}</span>
            <Input
              type="number"
              min={0}
              value={model.output || ""}
              onChange={(e) =>
                onChange({ output: Number(e.target.value) || 0 })
              }
            />
          </label>
          <div className="flex items-center gap-5 sm:col-span-2">
            <label className="flex items-center gap-2 text-base">
              <Checkbox
                checked={model.tools}
                onCheckedChange={(checked) => onChange({ tools: checked })}
              />
              {t("deck.toolCalls")}
            </label>
            <label className="flex items-center gap-2 text-base">
              <Checkbox
                checked={model.images}
                onCheckedChange={(checked) => onChange({ images: checked })}
              />
              {t("deck.imageInput")}
            </label>
            <label className="flex items-center gap-2 text-base">
              <Checkbox
                checked={Boolean(model.reasoningSupported)}
                onCheckedChange={(checked) =>
                  onChange({ reasoningSupported: checked })
                }
              />
              {t("deck.reasoning")}
            </label>
          </div>
        </div>
      )}
    </div>
  );
}
