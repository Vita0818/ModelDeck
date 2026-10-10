"use strict";

// Scope a test process to an empty home without changing HOME or the installed app.
const fs = require("node:fs");
const os = require("node:os");
const path = require("node:path");
const moduleApi = require("node:module");
const home = process.env.MODELDECK_TEST_PRIVATE_HOME;
if (!home || !path.isAbsolute(home)) throw new Error("A private test home is required");
os.homedir = () => home;
moduleApi.syncBuiltinESMExports();

// The CLI's local model path should need only the loopback gateway. Reject and
// record any attempt to contact account, telemetry, or other remote services.
const allowed = new Set(["127.0.0.1", "localhost", "::1", "[::1]"]);
const log = process.env.MODELDECK_TEST_NETWORK_LOG;
function check(input, options, surface) {
  let host;
  let pathname;
  if (typeof input === "string" || input instanceof URL) {
    const value = new URL(input);
    host = value.hostname;
    pathname = value.pathname;
  } else {
    host = input?.hostname || input?.host || options?.hostname || options?.host;
    pathname = input?.path?.split("?")[0];
    if (host?.includes(":")) host = host.replace(/:\d+$/, "");
  }
  if (!host) return;
  const blocked = !allowed.has(host);
  if (log) fs.appendFileSync(log, JSON.stringify({ surface, host, path: pathname, blocked }) + "\n");
  if (blocked) throw new Error("Test process blocked a remote connection: " + host);
}
for (const name of ["node:http", "node:https"]) {
  const api = require(name);
  for (const method of ["request", "get"]) {
    const original = api[method];
    api[method] = function (input, options, ...rest) {
      check(input, options, name + "." + method);
      return original.call(this, input, options, ...rest);
    };
  }
}
if (globalThis.fetch) {
  const original = globalThis.fetch;
  let responseCount = 0;
  globalThis.fetch = async function (input, options) {
    check(input?.url || input, options, "fetch");
    const response = await original.call(this, input, options);
    if (log) {
      const file = log + ".response-" + (++responseCount) + ".txt";
      response.clone().text().then(text => fs.writeFileSync(file,
        text.replaceAll(process.env.CURSOR_LOCAL_AGENT_API_KEY || "not-a-key", "<redacted>"))
      ).catch(() => {});
    }
    return response;
  };
}
const http2 = require("node:http2");
const connect = http2.connect;
http2.connect = function (authority, ...args) {
  check(authority, undefined, "http2.connect");
  return connect.call(this, authority, ...args);
};
moduleApi.syncBuiltinESMExports();
