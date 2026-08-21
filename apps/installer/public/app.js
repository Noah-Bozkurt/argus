const command = document.querySelector("#command");
const copy = document.querySelector("#copy-command");
const copyLabel = copy.querySelector(".copy-label");
const revision = document.querySelector("#revision");
const revisionStatus = document.querySelector("#revision-status");

const installCommand = `curl -fsSL '${window.location.origin}/install' | sudo bash`;
command.textContent = installCommand;

copy.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(installCommand);
    copy.classList.add("copied");
    copyLabel.textContent = "Copied";
    window.setTimeout(() => {
      copy.classList.remove("copied");
      copyLabel.textContent = "Copy";
    }, 1800);
  } catch {
    copyLabel.textContent = "Select command";
  }
});

async function loadRevision() {
  try {
    const response = await fetch("/manifest.json", { cache: "no-store" });
    if (!response.ok) {
      throw new Error(`manifest returned ${response.status}`);
    }
    const manifest = await response.json();
    if (typeof manifest.revision !== "string" || !/^[0-9a-f]{40}$/.test(manifest.revision)) {
      throw new Error("manifest revision is invalid");
    }
    revision.textContent = manifest.revision.slice(0, 12);
    revision.title = manifest.revision;
    revisionStatus.textContent = "Immutable Git revision published with the installer.";
  } catch {
    revision.textContent = "Published build";
    revisionStatus.textContent = "The installer still verifies its checksum before execution.";
  }
}

loadRevision();
