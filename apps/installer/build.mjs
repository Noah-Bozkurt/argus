import { createHash } from "node:crypto";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const appDir = dirname(fileURLToPath(import.meta.url));
const rootDir = resolve(appDir, "../..");
const outputDir = resolve(appDir, "dist");
const installer = await readFile(resolve(rootDir, "install.sh"));
const digest = createHash("sha256").update(installer).digest("hex");

await rm(outputDir, { recursive: true, force: true });
await mkdir(outputDir, { recursive: true });
await cp(resolve(appDir, "public"), outputDir, { recursive: true });
await writeFile(resolve(outputDir, "install.sh"), installer, { mode: 0o644 });
await writeFile(resolve(outputDir, "install.sh.sha256"), `${digest}  install.sh\n`);

console.log(`Built installer site with install.sh sha256 ${digest}`);
