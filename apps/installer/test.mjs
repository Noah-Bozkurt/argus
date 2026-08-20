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
