"use strict";

const Microphone = {
  status: null, draft: { colors: null }, dirty: false, gainDirty: false,
  sending: false, revision: 0, lightingSignature: "",

  init() {
    $("#mic-gain").addEventListener("input", e => {
      this.gainDirty = true;
      $("#mic-gain-value").textContent = e.target.value + "%";
      this.render();
    });
    $("#mic-gain-apply").addEventListener("click", () => this.control("gain", +$("#mic-gain").value));
    $("#mic-mute").addEventListener("click", () => this.control("mute", this.status?.muted ? 0 : 1));
    $("#mic-lock").addEventListener("change", e => this.control("smart_lock", +e.target.checked));
    $("#mic-lighting-enabled").addEventListener("change", e => {
      this.draft.colors = e.target.checked ? Object.fromEntries((this.status?.zones || []).map(zone => [zone.id, this.status.settings.colors?.[zone.id] || "#1e56e4"])) : null;
      this.dirty = true;
      this.renderLighting(true);
      this.renderButtons();
    });
    $("#mic-zones").addEventListener("input", e => {
      const zone = e.target.dataset.zone;
      if (zone == null || this.draft.colors == null) return;
      this.draft.colors[zone] = e.target.value;
      this.lightingSignature = JSON.stringify([this.status?.zones || [], this.draft]);
      this.dirty = true;
      this.renderButtons();
    });
    $("#mic-lighting-save").addEventListener("click", () => this.send("set_microphone_lighting", { settings: this.draft }, "lighting"));
    $("#mic-lighting-reset").addEventListener("click", () => {
      this.draft = structuredClone(this.status?.settings || { colors: null });
      this.dirty = false;
      this.renderLighting(true);
      this.renderButtons();
    });
  },

  control(control, value) { return this.send("set_microphone_control", { control, value }, control); },
  async send(command, args, kind) {
    if (this.sending || !this.status?.connected) return;
    this.sending = true;
    this.pendingKind = kind;
    this.revision = this.status.revision;
    this.renderLighting();
    this.renderButtons();
    try { await invoke(command, args); }
    catch (error) { this.sending = false; toast(String(error), true); this.render(); }
  },

  update(status) {
    if (!status) return;
    this.status = status;
    if (this.sending && (status.revision > this.revision || !status.connected)) {
      this.sending = false;
      if (!status.error && status.connected) {
        if (this.pendingKind === "lighting") this.dirty = false;
        if (this.pendingKind === "gain") this.gainDirty = false;
      }
      toast(status.message || "Yeti GX disconnected before the setting was confirmed", status.error || !status.connected);
    }
    if (!this.dirty) this.draft = structuredClone(status.settings);
    this.render();
  },

  renderLighting(force = false) {
    const zones = this.status?.zones || [];
    const signature = JSON.stringify([zones, this.draft]);
    if (force || signature !== this.lightingSignature) {
      this.lightingSignature = signature;
      $("#mic-lighting-enabled").checked = this.draft.colors != null;
      $("#mic-zones").innerHTML = zones.map(zone => `<label>Lighting zone ${zone.id + 1} <input type="color" data-zone="${zone.id}" value="${esc(this.draft.colors?.[zone.id] || "#1e56e4")}"></label>`).join("") || '<p>No compatible RGB lighting zones reported.</p>';
    }
    $("#mic-lighting-enabled").disabled = !this.status?.connected || this.sending || (!zones.length && this.draft.colors == null);
    $$("#mic-zones input").forEach(input => { input.disabled = !this.status?.connected || this.sending || this.draft.colors == null; });
  },

  renderButtons() {
    const status = this.status;
    const unavailable = !status?.connected || this.sending;
    $("#mic-gain").disabled = unavailable || status?.gain == null;
    $("#mic-gain-apply").disabled = unavailable || status?.gain == null || !this.gainDirty;
    $("#mic-mute").disabled = unavailable || status?.muted == null;
    $("#mic-lock").disabled = unavailable || status?.smart_lock == null;
    $("#mic-lighting-save").disabled = unavailable || (!this.dirty && !(this.draft.colors != null && !status?.lighting_active));
    $("#mic-lighting-reset").disabled = this.sending || !this.dirty;
    $("#mic-feedback").textContent = this.sending ? "Waiting for microphone confirmation…" : this.dirty ? "Unsaved lighting changes" : status?.message || "";
    $("#mic-feedback").classList.toggle("error", !!status?.error);
  },

  render() {
    const status = this.status;
    $$('.dev-tabs [data-dev="microphone"]').forEach(tab => tab.classList.toggle("off", !status?.connected));
    $("#mic-state").textContent = status?.connected ? "USB connected · Yeti GX" : status?.message || "Yeti GX not connected. Connect the microphone using USB.";
    if (!this.gainDirty && !this.sending) {
      $("#mic-gain").value = status?.gain ?? 50;
      $("#mic-gain-value").textContent = status?.gain == null ? "Unavailable" : status.gain + "%";
    }
    $("#mic-hardware-gain").textContent = this.gainDirty && status?.gain != null && +$("#mic-gain").value !== status.gain ? `On mic: ${status.gain}%` : "";
    $("#mic-mute").textContent = status?.muted === true ? "Unmute microphone" : "Mute microphone";
    $("#mic-mute-state").textContent = status?.muted == null ? "Hardware mute unavailable" : status.muted ? "Hardware muted" : "Hardware unmuted";
    if (!this.sending) $("#mic-lock").checked = status?.smart_lock || false;
    const peak = status?.input?.peak;
    const value = Number.isFinite(peak) ? Math.max(0, Math.min(1, peak)) : null;
    $("#mic-meter").value = value == null ? 0 : value * 100;
    $("#mic-meter-value").textContent = value == null ? "Unavailable" : value === 0 ? "−∞ dBFS" : `${(20 * Math.log10(value)).toFixed(1)} dBFS`;
    $("#mic-meter-state").textContent = status?.input_error || (status?.input?.muted ? "The microphone input is muted in Windows." : status?.input?.device || (status?.input_supported ? "Waiting for the Yeti GX audio input." : "Input metering is available on Windows in this version."));
    $("#mic-lighting-state").textContent = this.draft.colors == null ? "Using microphone lighting" : status?.lighting_active ? "Custom colors active" : "Custom colors can be applied when G HUB is closed";
    this.renderLighting();
    this.renderButtons();
  },
};
