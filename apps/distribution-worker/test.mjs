import assert from "node:assert/strict";
import { test } from "node:test";
import worker from "./src/worker.mjs";

class KV { constructor() { this.values = new Map(); } async get(k, type) { const v = this.values.get(k); return type === "json" && v ? JSON.parse(v) : v ?? null; } async put(k, v) { this.values.set(k, v); } async delete(k) { this.values.delete(k); } }
const env = () => ({ SESSIONS: new KV(), RELEASES: { get: async () => null, head: async () => null }, GITHUB_REPOSITORY: "owner/argus", GITHUB_REPOSITORY_ID: "1" });

test("private endpoints require a completed device session", async () => {
  const response = await worker.fetch(new Request("https://install.example/api/releases/stable/control-plane/manifest"), env());
  assert.equal(response.status, 401);
});

test("artifact grants are single use", async () => {
  const e = env();
  e.RELEASES.get = async () => ({ body: "bundle" });
  await e.SESSIONS.put("grant:abc", JSON.stringify({ object: "stable/amd64/control-plane.tar.zst" }));
  assert.equal((await worker.fetch(new Request("https://install.example/api/artifacts/abc"), e)).status, 200);
  assert.equal((await worker.fetch(new Request("https://install.example/api/artifacts/abc"), e)).status, 404);
});
