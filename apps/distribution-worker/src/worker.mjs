const json = (body, status = 200, headers = {}) => new Response(JSON.stringify(body), {
  status,
  headers: { "content-type": "application/json; charset=utf-8", "cache-control": "no-store", ...headers },
});
const error = (status, code, message) => json({ code, message }, status);
const randomId = () => crypto.randomUUID().replaceAll("-", "");
const now = () => Math.floor(Date.now() / 1000);
const key = (kind, id) => `${kind}:${id}`;

async function github(path, init = {}) {
  return fetch(`https://api.github.com${path}`, {
    ...init,
    headers: { accept: "application/vnd.github+json", "user-agent": "argus-distribution", "x-github-api-version": "2022-11-28", ...(init.headers ?? {}) },
  });
}

async function githubOAuth(path, body) {
  return fetch(`https://github.com${path}`, {
    method: "POST",
    headers: { accept: "application/json", "content-type": "application/json", "user-agent": "argus-distribution" },
    body: JSON.stringify(body),
  });
}

async function startDevice(env) {
  if (!env.GITHUB_CLIENT_ID) return error(503, "not_configured", "GitHub device authorization is not configured");
  const response = await githubOAuth("/login/device/code", { client_id: env.GITHUB_CLIENT_ID });
  const device = await response.json();
  if (!response.ok || !device.device_code) return error(502, "github_error", "GitHub did not start device authorization");
  const id = randomId();
  const ttl = Math.min(Number(device.expires_in || 900), Number(env.SESSION_TTL_SECONDS || 900));
  await env.SESSIONS.put(key("session", id), JSON.stringify({
    device_code: device.device_code, status: "pending", interval: Number(device.interval || 5),
    next_poll_at: now(), expires_at: now() + ttl,
  }), { expirationTtl: ttl });
  return json({ id, user_code: device.user_code, verification_uri: device.verification_uri, expires_in: ttl, interval: Number(device.interval || 5) }, 201);
}

async function pollSession(env, id) {
  const stored = await env.SESSIONS.get(key("session", id), "json");
  if (!stored) return error(404, "expired", "Device session expired");
  if (stored.status === "authorized" || stored.status === "denied") return json({ status: stored.status, expires_at: stored.expires_at });
  if (now() < stored.next_poll_at) return json({ status: "pending", retry_after: stored.next_poll_at - now() }, 202);

  const tokenResponse = await githubOAuth("/login/oauth/access_token", { client_id: env.GITHUB_CLIENT_ID, device_code: stored.device_code, grant_type: "urn:ietf:params:oauth:grant-type:device_code" });
  const token = await tokenResponse.json();
  if (token.error) {
    if (token.error === "authorization_pending" || token.error === "slow_down") {
      stored.interval += token.error === "slow_down" ? 5 : 0;
      stored.next_poll_at = now() + stored.interval;
      await env.SESSIONS.put(key("session", id), JSON.stringify(stored), { expiration: stored.expires_at });
      return json({ status: "pending", retry_after: stored.interval }, 202);
    }
    const denied = ["access_denied", "expired_token"].includes(token.error);
    stored.status = "denied";
    await env.SESSIONS.put(key("session", id), JSON.stringify(stored), { expiration: stored.expires_at });
    return error(denied ? 403 : 502, token.error, denied ? "GitHub authorization was denied or expired" : "GitHub authorization failed");
  }

  const bearer = { authorization: `Bearer ${token.access_token}` };
  const [userResponse, repoResponse] = await Promise.all([
    github("/user", { headers: bearer }), github(`/repos/${env.GITHUB_REPOSITORY}`, { headers: bearer }),
  ]);
  const user = await userResponse.json();
  const repo = await repoResponse.json();
  const authorized = userResponse.ok && repoResponse.ok && String(repo.id) === String(env.GITHUB_REPOSITORY_ID) && repo.permissions?.pull === true;
  if (env.GITHUB_CLIENT_SECRET) {
    await github(`/applications/${env.GITHUB_CLIENT_ID}/token`, {
      method: "DELETE",
      headers: { authorization: `Basic ${btoa(`${env.GITHUB_CLIENT_ID}:${env.GITHUB_CLIENT_SECRET}`)}`, "content-type": "application/json" },
      body: JSON.stringify({ access_token: token.access_token }),
    }).catch(() => undefined);
  }
  // The token is never persisted or returned to the installer, even if best-effort
  // revocation is temporarily unavailable.
  stored.device_code = undefined;
  stored.status = authorized ? "authorized" : "denied";
  stored.github_login = authorized ? user.login : undefined;
  await env.SESSIONS.put(key("session", id), JSON.stringify(stored), { expiration: stored.expires_at });
  return authorized ? json({ status: "authorized", expires_at: stored.expires_at }) : error(403, "repository_access_denied", "This GitHub account cannot access Argus");
}

async function requireAuthorized(env, request) {
  const id = request.headers.get("x-argus-device-session");
  if (!id) return null;
  const session = await env.SESSIONS.get(key("session", id), "json");
  return session?.status === "authorized" && session.expires_at > now() ? { id, session } : null;
}

async function manifest(env, request, channel, bundle) {
  if (!await requireAuthorized(env, request)) return error(401, "authorization_required", "Complete GitHub device authorization first");
  if (!/^[a-z0-9._-]+$/.test(channel) || !["control-plane", "managed-node"].includes(bundle)) return error(400, "invalid_release", "Invalid release selection");
  const base = `${channel}/amd64/${bundle}`;
  const [body, signature] = await Promise.all([env.RELEASES.get(`${base}.manifest.json`), env.RELEASES.get(`${base}.manifest.sig`)]);
  if (!body || !signature) return error(404, "release_not_found", "Release manifest not found");
  return json({ manifest: await body.text(), signature: await signature.text() });
}

async function createGrant(env, request) {
  const auth = await requireAuthorized(env, request);
  if (!auth) return error(401, "authorization_required", "Complete GitHub device authorization first");
  const input = await request.json().catch(() => ({}));
  const object = input.object;
  if (typeof object !== "string" || !/^[a-z0-9._/-]+\.tar(?:\.zst|\.gz)$/.test(object) || object.includes("..")) return error(400, "invalid_artifact", "Invalid artifact name");
  if (!await env.RELEASES.head(object)) return error(404, "artifact_not_found", "Artifact not found");
  const grant = randomId();
  const ttl = Number(env.GRANT_TTL_SECONDS || 120);
  await env.SESSIONS.put(key("grant", grant), JSON.stringify({ object }), { expirationTtl: ttl });
  return json({ url: new URL(`/api/artifacts/${grant}`, request.url).toString(), expires_in: ttl }, 201);
}

async function download(env, grant) {
  const grantKey = key("grant", grant);
  const value = await env.SESSIONS.get(grantKey, "json");
  if (!value) return error(404, "grant_expired", "Artifact grant expired or was already used");
  await env.SESSIONS.delete(grantKey);
  const object = await env.RELEASES.get(value.object);
  if (!object) return error(404, "artifact_not_found", "Artifact not found");
  return new Response(object.body, { headers: { "content-type": "application/octet-stream", "cache-control": "private, no-store" } });
}

export default { async fetch(request, env) {
  const url = new URL(request.url);
  if (request.method === "POST" && url.pathname === "/api/device/start") return startDevice(env);
  const session = url.pathname.match(/^\/api\/device\/sessions\/([a-f0-9]+)$/);
  if (request.method === "GET" && session) return pollSession(env, session[1]);
  const release = url.pathname.match(/^\/api\/releases\/([a-z0-9._-]+)\/(control-plane|managed-node)\/manifest$/);
  if (request.method === "GET" && release) return manifest(env, request, release[1], release[2]);
  if (request.method === "POST" && url.pathname === "/api/artifact-grants") return createGrant(env, request);
  const artifact = url.pathname.match(/^\/api\/artifacts\/([a-f0-9]+)$/);
  if (request.method === "GET" && artifact) return download(env, artifact[1]);
  return error(404, "not_found", "Not found");
} };
