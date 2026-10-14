// Exercise the model records with the SDK shipped by the official desktop app.
import { readFileSync } from "node:fs";
import { pathToFileURL } from "node:url";

const state = JSON.parse(readFileSync(process.argv[2], "utf8"));
const { CopilotClient } = await import(
  pathToFileURL(process.env.MODELDECK_NATIVE_COPILOT_SDK).href
);
const native = state.provider;
const settings = JSON.parse(native.settings_json);
const client = new CopilotClient({
  connection: { kind: "stdio", path: process.env.MODELDECK_NATIVE_COPILOT },
  useLoggedInUser: false,
  logLevel: "error",
  env: {
    ...process.env,
    COPILOT_GITHUB_TOKEN: undefined,
    GH_TOKEN: undefined,
    GITHUB_TOKEN: undefined,
    COPILOT_OFFLINE: "true",
    COPILOT_AUTO_UPDATE: "false",
    COPILOT_ALLOW_ALL: "false",
    // The standalone CLI's offline startup check precedes session creation.
    // A closed localhost port makes any accidental legacy fallback fail.
    COPILOT_PROVIDER_BASE_URL: "http://127.0.0.1:1/v1",
    COPILOT_MODEL: "startup-only",
    COPILOT_PROVIDER_MODEL_ID: "startup-only",
    COPILOT_PROVIDER_WIRE_MODEL: "startup-only",
    COPILOT_PROVIDER_TYPE: "openai",
    COPILOT_PROVIDER_WIRE_API: "completions",
    COPILOT_PROVIDER_TRANSPORT: "http",
    COPILOT_PROVIDER_API_KEY: undefined,
    COPILOT_PROVIDER_BEARER_TOKEN: undefined,
    COPILOT_PROVIDER_API_KEY_COMMAND: undefined,
    COPILOT_PROVIDER_HEADERS: undefined,
    COPILOT_PROVIDERS_CONFIG: undefined,
  },
});

try {
  await client.start();
  const session = await client.createSession({
    model: state.selection.value,
    streaming: true,
    providers: [
      {
        name: native.id,
        type: native.type === "anthropic" ? "anthropic" : "openai",
        baseUrl: settings.baseUrl,
        wireApi: settings.wireApi,
        headers: settings.headers,
      },
    ],
    models: state.models.map((model) => ({
      id: model.model_id,
      provider: native.id,
      wireModel: model.wire_model ?? model.model_id,
      name: model.display_name,
      maxPromptTokens: model.max_prompt_tokens,
      maxOutputTokens: model.max_output_tokens,
    })),
    onPermissionRequest: async () => ({ kind: "denied-no-approval-rule" }),
  });
  const response = await session.sendAndWait(
    { prompt: "Reply native-probe-ready; do not call tools." },
    25000,
  );
  if (!response?.data.content.includes("native-probe-ready")) {
    throw new Error("The desktop SDK did not return the expected mock reply");
  }
  console.log("desktop-sdk-native-probe-ready");
} finally {
  await client.stop();
}
