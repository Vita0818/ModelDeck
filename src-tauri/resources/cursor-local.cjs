"use strict";

// Use Cursor's official authless runtime. This adapter supplies only the
// connection and refreshes its native model picker from ModelDeck's /models.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const Module = require("node:module");
const supported = require("./cursor-models.manifest.json");

function validateInstallation(entry) {
  const hash = crypto.createHash("sha256").update(fs.readFileSync(entry)).digest("hex");
  const manifest = supported[hash];
  if (manifest?.mode !== "local-api")
    throw new Error("ModelDeck: select a verified official cursor-agent-local runtime.");
  const directory = path.dirname(entry);
  for (const [file, expected] of Object.entries(manifest.chunks)) {
    const actual = crypto.createHash("sha256")
      .update(fs.readFileSync(path.join(directory, file))).digest("hex");
    if (actual !== expected)
      throw new Error("ModelDeck: Cursor's local model module changed; adapter was not loaded.");
  }
  return directory;
}

function projectDeck(store, origin) {
  const binding = store.activeBindings?.["cursor-agent"];
  const endpoint = binding && store.endpoints?.[binding.endpointId];
  if (!endpoint?.protocols?.includes("openai-chat"))
    throw new Error("ModelDeck: no active Cursor local model connection.");
  const models = Object.entries(endpoint.routes || {}).flatMap(([alias, route]) => {
    const model = store.models?.[route.modelId];
    const provider = model && store.providers?.[model.providerId];
    return model?.tools && provider?.protocol === "openai-chat" &&
      !alias.startsWith("claude-") && !alias.startsWith("gemini-")
      ? [{ id: alias, name: alias }] : [];
  });
  return {
    enabled: true,
    endpoint: `${origin.replace(/\/$/, "")}/deck-clients/cursor-agent`,
    apiKey: endpoint.token,
    models,
    defaultModel: binding.model,
  };
}

function configure(profile) {
  if (!profile?.enabled || !profile.models?.some(m => m.id === profile.defaultModel))
    throw new Error("ModelDeck: the default Cursor local model is unavailable.");
  const url = new URL(profile.endpoint);
  if (!["http:", "https:"].includes(url.protocol) || url.username || url.password || url.search || url.hash)
    throw new Error("ModelDeck: invalid Cursor local endpoint.");
  const key = profile.keyEnv ? process.env[profile.keyEnv] : profile.apiKey;
  if (!key?.trim()) throw new Error("ModelDeck: the Cursor local endpoint key is unavailable.");
  process.env.CURSOR_ENABLE_AUTHLESS = "1";
  process.env.CURSOR_LOCAL_AGENT_BASE_URL = profile.endpoint;
  process.env.CURSOR_LOCAL_AGENT_API_KEY = key;
}

function wrapMain(original) {
  return function localMainFactory(module, exports, webpackRequire) {
    const native = {};
    original.call(this, module, native, webpackRequire);
    if (typeof native.runAgentCliMain !== "function")
      throw new Error("ModelDeck: Cursor's local startup contract changed.");
    for (const name of Object.keys(native)) {
      let value = native[name];
      if (name === "runAgentCliMain") {
        value = function startLocal(options) {
          const runtime = options?.localAgentRuntime;
          const Manager = runtime?.LocalModelManager;
          if (typeof Manager?.prototype.refreshAvailableModelsFromProvider !== "function")
            throw new Error("ModelDeck: Cursor's local model discovery contract changed.");
          class ConfiguredModelManager extends Manager {
            constructor(settings) {
              super(settings);
              // Authless mode intentionally skips automatic discovery. Calling
              // this native method adds the real gateway catalog and metadata.
              this.modelFetchPromise = this.refreshAvailableModelsFromProvider(settings);
            }
          }
          return native.runAgentCliMain({
            ...options,
            localAgentRuntime: { ...runtime, LocalModelManager: ConfiguredModelManager },
          });
        };
      }
      Object.defineProperty(exports, name, { enumerable: true, get: () => value });
    }
  };
}

function wrapModelService(original, profile) {
  return function localCatalogFactory(module, exports, webpackRequire) {
    const native = {};
    original.call(this, module, native, webpackRequire);
    const proto = webpackRequire("../proto/dist/generated/agent/v1/agent_pb.js");
    const ModelDetails = Object.values(proto).find(value =>
      typeof value === "function" && value.typeName === "agent.v1.ModelDetails");
    if (!ModelDetails) throw new Error("ModelDeck: Cursor's model catalog contract changed.");
    let found = false;
    for (const name of Object.keys(native)) {
      let value = native[name];
      if (typeof value === "function" && String(value).includes("parameterizedModelsFetchStatus")) {
        found = true;
        value = async () => {
          const availableModels = profile.models.map(model => new ModelDetails({
            modelId: model.id, displayModelId: model.id,
            displayName: model.name, displayNameShort: model.name, aliases: [model.id],
          }));
          return {
            availableModels,
            defaultModel: availableModels.find(model => model.modelId === profile.defaultModel),
            parameterizedModels: [], parameterizedModelsFetchStatus: "loaded",
          };
        };
      }
      Object.defineProperty(exports, name, { enumerable: true, get: () => value });
    }
    if (!found) throw new Error("ModelDeck: Cursor's model list contract changed.");
  };
}

function install(entry, profile) {
  const directory = validateInstallation(entry);
  configure(profile);
  const load = Module._load;
  const visited = new WeakSet();
  Module._load = function loadLocalMain(request, parent) {
    const loaded = load.apply(this, arguments);
    if (parent?.filename && path.dirname(parent.filename) === directory &&
        loaded?.modules && !visited.has(loaded)) {
      visited.add(loaded);
      const factory = loaded.modules["./dist/index.js"];
      if (factory) loaded.modules["./dist/index.js"] = wrapMain(factory);
      const models = loaded.modules["./dist/models/model-service.js"];
      if (models) loaded.modules["./dist/models/model-service.js"] = wrapModelService(models, profile);
    }
    return loaded;
  };
}

if (process.env.CCSWITCH_CURSOR_PROFILE_FILE) {
  const entry = process.argv[1] && fs.realpathSync(process.argv[1]);
  if (entry && path.basename(entry) === "index.js") {
    const store = JSON.parse(fs.readFileSync(process.env.CCSWITCH_CURSOR_PROFILE_FILE, "utf8"));
    const origin = store.schemaVersion === 1
      ? fs.readFileSync(process.env.MODELDECK_CURSOR_ORIGIN_FILE, "utf8").trim() : undefined;
    install(entry, store.schemaVersion === 1 ? projectDeck(store, origin) : store.active?.["cursor-agent"]);
  }
}

module.exports = { validateInstallation, projectDeck, configure, wrapMain, wrapModelService };
