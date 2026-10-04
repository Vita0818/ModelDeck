"use strict";

// Model-only adapter for Cursor's shipped CLI. Installed files stay intact.
// The native protobuf contract supports BYOK credentials, but the CLI's model
// manager drops them when constructing RequestedModel. Attach the credentials
// at that boundary and supply the explicitly configured model catalog.
const fs = require("node:fs");
const path = require("node:path");
const crypto = require("node:crypto");
const Module = require("node:module");

const supported = require("./cursor-models.manifest.json");
const modelServiceId = "./src/models/model-service.ts";
const modelManagerId = "./src/models/index.ts";
const protoId = "../proto/dist/generated/agent/v1/agent_pb.js";
const digest = (file) =>
  crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex");

function validateInstallation(entry) {
  const manifest = supported[digest(entry)];
  if (!manifest)
    throw new Error(
      "CC Switch: this Cursor Agent build has not been validated for BYOK. Open CC Switch to update its model adapter, or run the native Cursor command without this launcher.",
    );
  const directory = path.dirname(entry);
  for (const [name, expected] of Object.entries(manifest.chunks)) {
    if (digest(path.join(directory, name)) !== expected)
      throw new Error(
        "CC Switch: Cursor's model module has changed; the model adapter was not loaded.",
      );
  }
  return { directory, manifest };
}

function validateProfile(profile) {
  if (
    !profile?.enabled ||
    !Array.isArray(profile.models) ||
    !profile.models.length
  )
    throw new Error("CC Switch: no active Cursor Agent model connection.");
  const url = new URL(profile.endpoint);
  if (
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    url.search ||
    url.hash
  )
    throw new Error(
      "CC Switch: Cursor BYOK requires a public HTTPS model endpoint.",
    );
  const apiKey = profile.keyEnv ? process.env[profile.keyEnv] : profile.apiKey;
  if (!apiKey?.trim())
    throw new Error(
      "CC Switch: the dedicated Cursor Agent API key is unavailable.",
    );
  if (!profile.models.some((model) => model.id === profile.defaultModel))
    throw new Error(
      "CC Switch: default Cursor Agent model is not in its catalog.",
    );
  return apiKey;
}

function messageType(proto, name) {
  const value = Object.values(proto).find(
    (type) =>
      typeof type === "function" && type.typeName === `agent.v1.${name}`,
  );
  if (!value) throw new Error(`CC Switch: missing Cursor ${name} contract.`);
  return value;
}

function modelList(profile, proto) {
  const ModelDetails = messageType(proto, "ModelDetails");
  return profile.models.map(
    (model) =>
      new ModelDetails({
        modelId: model.id,
        displayModelId: model.id,
        displayName: model.name,
        displayNameShort: model.name,
        aliases: [model.id],
      }),
  );
}

function wrapService(original, profile) {
  return function modelService(module, exports, webpackRequire) {
    const nativeExports = {};
    original.call(this, module, nativeExports, webpackRequire);
    const proto = webpackRequire(protoId);
    let catalogFunctionFound = false;
    for (const name of Object.keys(nativeExports)) {
      const native = nativeExports[name];
      let replacement = native;
      if (
        typeof native === "function" &&
        String(native).includes("parameterizedModelsFetchStatus")
      ) {
        catalogFunctionFound = true;
        replacement = async function configuredModels() {
          const availableModels = modelList(profile, proto);
          return {
            availableModels,
            defaultModel: availableModels.find(
              (model) => model.modelId === profile.defaultModel,
            ),
            parameterizedModels: [],
            parameterizedModelsFetchStatus: "loaded",
          };
        };
      } else if (
        typeof native === "function" &&
        String(native).includes("availableModels(")
      ) {
        // The separate parameterized picker cannot represent third-party model
        // definitions. Cursor falls back to its ordinary native model picker.
        replacement = async () => [];
      }
      Object.defineProperty(exports, name, {
        enumerable: true,
        get: () => replacement,
      });
    }
    if (!catalogFunctionFound)
      throw new Error("CC Switch: Cursor model catalog contract changed.");
  };
}

function wrapManager(original, profile, apiKey) {
  return function modelManager(module, exports, webpackRequire) {
    original.call(this, module, exports, webpackRequire);
    const Manager = Object.values(exports).find(
      (value) =>
        typeof value === "function" &&
        typeof value.prototype?.buildRequestedModel === "function",
    );
    if (!Manager)
      throw new Error("CC Switch: Cursor BYOK request contract changed.");
    const build = Manager.prototype.buildRequestedModel;
    Manager.prototype.buildRequestedModel = function buildConfiguredModel(
      ...args
    ) {
      const request = build.apply(this, args);
      if (!request) return request;
      if (!profile.models.some((model) => model.id === request.modelId))
        throw new Error(
          "CC Switch: the selected Cursor model is not in the configured catalog.",
        );
      const json = request.toJson();
      delete json.azureCredentials;
      delete json.bedrockCredentials;
      json.apiKeyCredentials = { apiKey, baseUrl: profile.endpoint };
      json.builtInModel = false;
      return request.constructor.fromJson(json);
    };
  };
}

function install(entry, profile) {
  const { directory } = validateInstallation(entry);
  const apiKey = validateProfile(profile);
  const originalLoad = Module._load;
  const visited = new WeakSet();
  Module._load = function loadModelModule(request, parent, isMain) {
    const loaded = originalLoad.apply(this, arguments);
    if (
      !parent?.filename ||
      path.dirname(parent.filename) !== directory ||
      !loaded?.modules ||
      visited.has(loaded)
    )
      return loaded;
    visited.add(loaded);
    if (loaded.modules[modelServiceId])
      loaded.modules[modelServiceId] = wrapService(
        loaded.modules[modelServiceId],
        profile,
      );
    if (loaded.modules[modelManagerId])
      loaded.modules[modelManagerId] = wrapManager(
        loaded.modules[modelManagerId],
        profile,
        apiKey,
      );
    return loaded;
  };
}

if (process.env.CCSWITCH_CURSOR_PROFILE_FILE) {
  // A dedicated environment variable is set only by the managed launcher.
  // Other Node programs spawned by agent tools do not load this adapter.
  const entry = process.argv[1] && fs.realpathSync(process.argv[1]);
  if (entry && path.basename(entry) === "index.js") {
    const store = JSON.parse(
      fs.readFileSync(process.env.CCSWITCH_CURSOR_PROFILE_FILE, "utf8"),
    );
    install(entry, store.active?.["cursor-agent"]);
  }
}

module.exports = {
  validateInstallation,
  validateProfile,
  wrapService,
  wrapManager,
};
