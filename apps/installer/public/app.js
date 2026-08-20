const form = document.querySelector("#installer-form");
const domainInput = document.querySelector("#domain");
const contentInput = document.querySelector("#content-domain");
const userInput = document.querySelector("#registry-user");
const commandPanel = document.querySelector("#command-panel");
const commandOutput = document.querySelector("#command");
const errorOutput = document.querySelector("#form-error");
const copyButton = document.querySelector("#copy-command");
const copyStatus = document.querySelector("#copy-status");

const domainPattern = /^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?\.[a-z0-9-]+$/;
const usernamePattern = /^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/;

domainInput.addEventListener("input", () => {
  if (!contentInput.dataset.edited) contentInput.value = `content.${domainInput.value}`;
});
contentInput.addEventListener("input", () => { contentInput.dataset.edited = "true"; });

form.addEventListener("submit", (event) => {
  event.preventDefault();
  const domain = domainInput.value.trim().toLowerCase();
  const contentDomain = contentInput.value.trim().toLowerCase();
  const username = userInput.value.trim();
  let error = "";
  if (!domainPattern.test(domain) || !domainPattern.test(contentDomain)) error = "Enter valid fully qualified domain names.";
  else if (domain === contentDomain) error = "The Argus and content domains must differ.";
  else if (!usernamePattern.test(username)) error = "Enter a valid GitHub username.";

  errorOutput.hidden = !error;
  errorOutput.textContent = error;
  if (error) return;

  const origin = window.location.origin;
  commandOutput.textContent = `export ARGUS_DOMAIN='${domain}'\nexport ARGUS_CONTENT_DOMAIN='${contentDomain}'\nexport ARGUS_REGISTRY_USERNAME='${username}'\nread -rsp 'GitHub package token: ' ARGUS_REGISTRY_TOKEN && echo\nexport ARGUS_REGISTRY_TOKEN\nARGUS_INSTALL_TMP="$(mktemp -d)"\ncurl -fsS '${origin}/install.sh' -o "$ARGUS_INSTALL_TMP/install.sh"\ncurl -fsS '${origin}/install.sh.sha256' -o "$ARGUS_INSTALL_TMP/install.sh.sha256"\n(cd "$ARGUS_INSTALL_TMP" && sha256sum -c install.sh.sha256)\nsudo -E bash "$ARGUS_INSTALL_TMP/install.sh"\nARGUS_INSTALL_STATUS=$?\nunset ARGUS_REGISTRY_TOKEN\nrm -rf "$ARGUS_INSTALL_TMP"\n(exit "$ARGUS_INSTALL_STATUS")`;
  commandPanel.hidden = false;
  commandPanel.scrollIntoView({ behavior: "smooth", block: "start" });
});

copyButton.addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(commandOutput.textContent);
    copyStatus.textContent = "Copied to clipboard.";
  } catch {
    copyStatus.textContent = "Copy failed. Select the command manually.";
  }
});
