const command = document.querySelector("#command");
const copy = document.querySelector("#copy-command");
command.textContent = `ARGUS_INSTALL_TMP="$(mktemp -d)"
curl -fsS '${window.location.origin}/install.sh' -o "$ARGUS_INSTALL_TMP/install.sh"
curl -fsS '${window.location.origin}/install.sh.sha256' -o "$ARGUS_INSTALL_TMP/install.sh.sha256"
(cd "$ARGUS_INSTALL_TMP" && sha256sum -c install.sh.sha256)
sudo bash "$ARGUS_INSTALL_TMP/install.sh"
ARGUS_INSTALL_STATUS=$?
rm -rf "$ARGUS_INSTALL_TMP"
(exit "$ARGUS_INSTALL_STATUS")`;
copy.addEventListener("click", async () => {
  try { await navigator.clipboard.writeText(command.textContent); copy.textContent = "Copied"; }
  catch { copy.textContent = "Select and copy the command"; }
});
