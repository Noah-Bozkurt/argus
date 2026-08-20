import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const appDir = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(appDir, "../..");

test("build publishes the canonical installer with a valid checksum", async () => {
  const build = spawnSync(process.execPath, [resolve(appDir, "build.mjs")], {
    encoding: "utf8",
  });
  assert.equal(build.status, 0, build.stderr);

  const [source, published, checksum] = await Promise.all([
    readFile(resolve(rootDir, "install.sh")),
    readFile(resolve(appDir, "dist/install.sh")),
    readFile(resolve(appDir, "dist/install.sh.sha256"), "utf8"),
  ]);
  assert.deepEqual(published, source);
  const digest = createHash("sha256").update(source).digest("hex");
  assert.equal(checksum, `${digest}  install.sh\n`);
});

test("bootstrap endpoint downloads and verifies the canonical installer", async () => {
  const bootstrap = await readFile(resolve(appDir, "public/install"), "utf8");
  assert.match(bootstrap, /install\.sh\.sha256/);
  assert.match(bootstrap, /sha256sum -c/);
  assert.match(bootstrap, /bash "\$tmp\/install\.sh"/);
});

test("public site does not contain a packaged registry credential", async () => {
  const files = await Promise.all(
    ["index.html", "app.js", "styles.css"].map((name) =>
      readFile(resolve(appDir, "public", name), "utf8"),
    ),
  );
  for (const contents of files) {
    assert.doesNotMatch(contents, /ghp_[A-Za-z0-9]+|github_pat_[A-Za-z0-9_]+/);
  }
});

test("public site is a generic verified command without configuration fields", async () => {
  const [html, script] = await Promise.all([
    readFile(resolve(appDir, "public/index.html"), "utf8"),
    readFile(resolve(appDir, "public/app.js"), "utf8"),
  ]);
  assert.doesNotMatch(html, /GitHub username|Argus domain|<form/);
  assert.match(html, /app\.js\?v=pat-installer-2/);
  assert.match(html, /Loading verified install command/);
  assert.match(script, /\/install/);
  assert.doesNotMatch(script, /ARGUS_REGISTRY_TOKEN|read -rsp/);
});

test("guided installer uses hidden PAT authentication without distribution storage", async () => {
  const installer = await readFile(resolve(rootDir, "install.sh"), "utf8");
  assert.match(installer, /classic PAT with read:packages/);
  assert.match(installer, /read -r -s -p 'GitHub token/);
  assert.match(installer, /INSTALL_MODE.*control-plane.*agent/s);
  assert.doesNotMatch(installer, /R2|device\/start|artifact-grants|release_public_key/);
  assert.match(installer, /registry\.env/);
  assert.match(installer, /chmod 0600/);
  const updater = await readFile(resolve(rootDir, "scripts/update-first-test.sh"), "utf8");
  assert.match(updater, /REGISTRY_CREDENTIAL_FILE/);
  assert.match(updater, /argusctl registry-login/);
});
