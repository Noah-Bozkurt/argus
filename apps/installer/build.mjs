import { createHash } from "node:crypto";
import { chmod, copyFile, cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appDir = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(appDir, "../..");
const outputDir = resolve(appDir, "dist");
const bootstrap = await readFile(resolve(rootDir, "install.sh"));
const revision = process.env.ARGUS_RELEASE_REVISION ?? "";
const binaryPath = process.env.ARGUS_INSTALLER_BINARY;

if (!/^[0-9a-f]{40}$/.test(revision)) {
  throw new Error("ARGUS_RELEASE_REVISION must be the tested 40-character commit SHA");
}
if (!binaryPath) {
  throw new Error("ARGUS_INSTALLER_BINARY must point to the tested native installer binary");
}

const binary = await readFile(binaryPath);
const maxPagesFileSize = 25 * 1024 * 1024;
if (binary.length > maxPagesFileSize) {
  throw new Error(`argus-installer is ${binary.length} bytes; Cloudflare Pages allows at most ${maxPagesFileSize}`);
}
const digest = createHash("sha256").update(binary).digest("hex");

await rm(outputDir, { recursive: true, force: true });
await mkdir(resolve(outputDir, "bin"), { recursive: true });
await cp(resolve(appDir, "public"), outputDir, { recursive: true });
await writeFile(resolve(outputDir, "install"), bootstrap, { mode: 0o644 });
await copyFile(binaryPath, resolve(outputDir, "bin/argus-installer-x86_64"));
await chmod(resolve(outputDir, "bin/argus-installer-x86_64"), 0o755);
await writeFile(
  resolve(outputDir, "manifest.json"),
  `${JSON.stringify({ revision, installer_sha256: digest })}\n`,
  { mode: 0o644 },
);

console.log(`Built installer site for ${revision.slice(0, 12)} with native installer sha256 ${digest}`);
