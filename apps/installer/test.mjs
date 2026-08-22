import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const appDir = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(appDir, "../..");
const revision = "0123456789abcdef0123456789abcdef01234567";

async function buildFixture() {
  const temp = await mkdtemp(resolve(tmpdir(), "argus-installer-test-"));
  const binaryPath = resolve(temp, "argus-installer");
  const binary = Buffer.from("fake native argus installer\n");
  await writeFile(binaryPath, binary, { mode: 0o755 });
  const build = spawnSync(process.execPath, [resolve(appDir, "build.mjs")], {
    encoding: "utf8",
    env: {
      ...process.env,
      ARGUS_RELEASE_REVISION: revision,
      ARGUS_INSTALLER_BINARY: binaryPath,
    },
  });
  assert.equal(build.status, 0, build.stderr);
  return { temp, binary };
}

test("build publishes bootstrap, native installer and immutable manifest", async () => {
  const { temp, binary } = await buildFixture();
  try {
    const [sourceBootstrap, publishedBootstrap, publishedBinary, manifest] = await Promise.all([
      readFile(resolve(rootDir, "install.sh")),
      readFile(resolve(appDir, "dist/install")),
      readFile(resolve(appDir, "dist/bin/argus-installer-x86_64")),
      readFile(resolve(appDir, "dist/manifest.json"), "utf8"),
    ]);
    assert.deepEqual(publishedBootstrap, sourceBootstrap);
    assert.deepEqual(publishedBinary, binary);
    assert.deepEqual(JSON.parse(manifest), {
      revision,
      installer_sha256: createHash("sha256").update(binary).digest("hex"),
    });
  } finally {
    await rm(temp, { recursive: true, force: true });
  }
});

test("bootstrap downloads and verifies the native installer", async () => {
  const bootstrap = await readFile(resolve(rootDir, "install.sh"), "utf8");
  assert.match(bootstrap, /manifest\.json/);
  assert.match(bootstrap, /bin\/\$ASSET/);
  assert.match(bootstrap, /sha256sum/);
  assert.match(bootstrap, /installer_sha256/);
  assert.match(bootstrap, /ARGUS_VERSION=.*REVISION/);
  assert.match(bootstrap, /exec "\$TMP\/argus-installer"/);
});

test("bootstrap shell syntax is valid", () => {
  const result = spawnSync("bash", ["-n", resolve(rootDir, "install.sh")], {
    encoding: "utf8",
  });
  assert.equal(result.status, 0, result.stderr);
});

test("native installer uses public registry access", async () => {
  const [lifecycle, installer] = await Promise.all([
    readFile(resolve(rootDir, "crates/cli/src/lifecycle.rs"), "utf8"),
    readFile(resolve(rootDir, "crates/cli/src/installer.rs"), "utf8"),
  ]);
  assert.match(lifecycle, /DEFAULT_REGISTRY/);
  assert.doesNotMatch(lifecycle, /docker login|ARGUS_REGISTRY_TOKEN/);
  assert.match(installer, /Install an Argus control plane or managed node/);
});

test("native installer keeps interactive prompts visible and guides uninstall", async () => {
  const installer = await readFile(resolve(rootDir, "crates/cli/src/installer.rs"), "utf8");
  assert.match(installer, /Type YES to continue:/);
  assert.match(installer, /permanently remove all Argus data, backups, logs, and Docker volumes/);
  assert.match(installer, /Content domain \[\{default\}\]:/);
  assert.match(installer, /rerun with --yes/);
});

test("legacy uninstall script is only a native compatibility shim", async () => {
  const uninstall = await readFile(resolve(rootDir, "scripts/uninstall.sh"), "utf8");
  assert.match(uninstall, /argus-installer/);
  assert.doesNotMatch(uninstall, /docker compose/);
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

test("public site stays a generic install command", async () => {
  const [html, script] = await Promise.all([
    readFile(resolve(appDir, "public/index.html"), "utf8"),
    readFile(resolve(appDir, "public/app.js"), "utf8"),
  ]);
  assert.doesNotMatch(html, /GitHub username|<form/);
  assert.match(script, /\/install/);
  assert.doesNotMatch(script, /ARGUS_REGISTRY_TOKEN|read -rsp/);
});

test("installer portal explains the real deployment and published revision", async () => {
  const [html, script] = await Promise.all([
    readFile(resolve(appDir, "public/index.html"), "utf8"),
    readFile(resolve(appDir, "public/app.js"), "utf8"),
  ]);

  for (const expected of [
    "Install Argus on your server",
    "Server requirements",
    "What Argus installs",
    "Installation modes",
    "/opt/argus/",
    "/etc/argus/",
    "argusctl smoke",
    "No GitHub account or token required",
  ]) {
    assert.match(html, new RegExp(expected.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")));
  }

  assert.match(script, /fetch\("\/manifest\.json"/);
  assert.match(script, /manifest\.revision\.slice\(0, 12\)/);
  assert.match(script, /navigator\.clipboard\.writeText/);
});

test("updater retains progress and reports public pull failures", async () => {
  const updater = await readFile(resolve(rootDir, "scripts/update-first-test.sh"), "utf8");
  assert.doesNotMatch(updater, /docker login|ARGUS_REGISTRY_TOKEN/);
  assert.match(updater, /failed to pull \$ref \(docker exit \$status\): \$summary/);
  assert.match(updater, /progress_start "Downloading update"/);
  assert.match(updater, /progress_start "Installing update"/);
  assert.match(updater, /progress_start "Starting Argus services"/);
});
