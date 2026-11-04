"use strict";
(() => {
  const devices = {g13:"G13", g915:"G915", mouse:"G502 X PLUS", microphone:"Yeti GX"};
  let loaded = false;
  $("#light-devices").innerHTML = Object.entries(devices).map(([id,name]) => `<label class="toggle"><input type="checkbox" value="${id}"><i></i><span>${name}</span></label>`).join("");
  async function refresh() {
    if (!$("#settings").classList.contains("open")) return;
    try {
      const status = await invoke("get_lighting");
      if (!loaded) {
        $("#light-enabled").checked = status.settings.enabled;
        $("#light-idle").value = status.settings.idle_minutes;
        $$("#light-devices input").forEach(input => { input.checked = status.settings.devices.includes(input.value); });
        loaded = true;
      }
      $$("#light-modes button").forEach(button => button.classList.toggle("cur", button.dataset.mode === status.mode));
      const streaming = status.streaming.map(id => devices[id]).join(", ") || "none";
      const active = status.streaming.filter(id => status.settings.devices.includes(id));
      const issues = Object.entries(status.device_errors || {}).map(([id,error]) => `${devices[id]}: ${error}`).join(". ");
      $("#light-status").textContent = status.error || issues || (!status.settings.enabled ? "Bridge disabled. G Nexus lighting is active." :
        `${status.signal_requested && active.length ? "SignalRGB colors selected" : "G Nexus colors selected"}. SignalRGB streams: ${streaming}. Idle: ${status.idle_seconds == null ? "unavailable" : Math.floor(status.idle_seconds) + "s"}.${!status.idle_supported ? " Automatic idle switching is available on Windows only." : ""}`);
    } catch (error) { $("#light-status").textContent = String(error); }
  }
  $("#light-save").addEventListener("click", async () => {
    const settings = { enabled:$("#light-enabled").checked, idle_minutes:Number($("#light-idle").value), devices:$$("#light-devices input:checked").map(input => input.value) };
    try { await invoke("set_lighting", {settings}); toast("Lighting bridge settings saved"); await refresh(); }
    catch (error) { toast(String(error), true); }
  });
  $("#light-modes").addEventListener("click", async event => {
    const mode = event.target.closest("[data-mode]")?.dataset.mode;
    if (!mode) return;
    try { await invoke("select_lighting", {mode}); await refresh(); }
    catch (error) { toast(String(error), true); }
  });
  // Loading once per settings opening protects unsaved edits from status polls.
  new MutationObserver(() => { if (!$("#settings").classList.contains("open")) loaded = false; else refresh(); })
    .observe($("#settings"), {attributes:true, attributeFilter:["class"]});
  setInterval(refresh, 1000);
})();
