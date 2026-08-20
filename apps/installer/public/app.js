const command = document.querySelector("#command");
const copy = document.querySelector("#copy-command");
command.textContent = `curl -fsSL '${window.location.origin}/install' | sudo bash`;
copy.addEventListener("click", async () => {
  try { await navigator.clipboard.writeText(command.textContent); copy.textContent = "Copied"; }
  catch { copy.textContent = "Select and copy the command"; }
});
