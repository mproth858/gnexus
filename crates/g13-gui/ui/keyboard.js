"use strict";

const Keyboard = {
  status: null,
  draft: null,
  bank: "M1",
  dirty: false,
  saving: false,

  init() {
    this.editor = new MacroEditor({
      list: $("#kb-macro-list"), edit: $("#kb-macro-edit"),
      get: () => this.draft?.profile.macros,
      set: (macros) => { this.draft.profile.macros = macros; },
      changed: () => this.changed(),
      recordingChanged: () => this.renderButtons(),
      renamed: (old, name) => this.rebind(old, name),
      deleted: (name) => this.rebind(name, null),
    });
    $("#kb-banks").addEventListener("click", (e) => {
      const bank = e.target.closest("[data-bank]")?.dataset.bank;
      if (bank) { this.bank = bank; this.renderKeys(); }
    });
    $("#kb-keys").addEventListener("click", (e) => {
      const key = e.target.closest("[data-key]")?.dataset.key;
      if (!key || !this.draft) return;
      const bank = this.draft.profile.banks[this.bank] ??= {};
      openPickerFor({
        label: key, sub: `G915 · ${this.bank}`, dance: true,
        get: () => bank[key],
        set: (action) => { if (action == null) delete bank[key]; else bank[key] = action; this.changed(); },
        macros: () => this.draft.profile.macros,
        newMacro: () => this.editor.add(),
        openMacros: () => { $("#kb-macros").open = true; this.editor.render(); $("#kb-macro-edit").scrollIntoView({ block: "nearest" }); },
      });
    });
    for (const id of ["kb-lighting", "kb-color", "kb-brightness-on", "kb-brightness", "kb-enabled"]) {
      $("#" + id).addEventListener("input", () => {
        if (!this.draft) return;
        this.draft.color = $("#kb-lighting").checked ? $("#kb-color").value : null;
        this.draft.brightness = $("#kb-brightness-on").checked ? +$("#kb-brightness").value : null;
        this.draft.enabled = $("#kb-enabled").checked;
        $("#kb-brightness-value").textContent = $("#kb-brightness").value + "%";
        this.changed();
      });
    }
    $("#kb-reset").addEventListener("click", () => {
      this.draft = structuredClone(this.status.settings);
      this.dirty = false;
      this.render();
    });
    $("#kb-save").addEventListener("click", async () => {
      this.savingRevision = this.status?.revision ?? 0;
      this.saving = true;
      this.renderButtons();
      try { await invoke("apply_keyboard", { settings: this.draft }); }
      catch (e) { this.saving = false; toast(String(e), true); this.renderButtons(); }
    });
  },

  rebind(old, name) {
    const visit = (a) => {
      if (!a || typeof a !== "object") return a;
      if (a.macro === old) return name ? { macro: name } : null;
      for (const slot of ["tap", "hold", "double_tap", "tap_hold"]) if (a[slot]) a[slot] = visit(a[slot]);
      return a;
    };
    for (const bank of Object.values(this.draft.profile.banks)) {
      for (const key of ["G1", "G2", "G3", "G4", "G5"]) {
        const action = visit(bank[key]);
        if (action == null) delete bank[key]; else bank[key] = action;
      }
    }
  },

  changed() { this.dirty = true; this.renderKeys(); this.renderButtons(); },

  update(status) {
    this.status = status;
    if (!status) return;
    if (this.saving && (status.revision > this.savingRevision || !status.connected)) {
      this.saving = false;
      if (!status.error) this.dirty = false;
      toast(status.message || "G915 disconnected before settings were applied", status.error || !status.connected);
    }
    if (!this.dirty && !this.editor.recording && !this.editor.recordBusy) this.draft = structuredClone(status.settings);
    this.draft.profile.macros ??= {};
    this.render();
  },

  renderKeys() {
    if (!this.draft) return;
    $$("#kb-banks button").forEach((b) => b.classList.toggle("on", b.dataset.bank === this.bank));
    const bank = this.draft.profile.banks[this.bank] || {};
    $("#kb-keys").innerHTML = [1, 2, 3, 4, 5].map((n) => {
      const a = bank[`G${n}`];
      return `<button class="kb-gkey${this.status?.held & (1 << (n - 1)) ? " held" : ""}" data-key="G${n}"><b>G${n}</b><span>${esc(a ? actionLabel(a) : "Unassigned")}</span></button>`;
    }).join("");
  },

  renderButtons() {
    $("#kb-controls").disabled = !this.status?.connected || this.saving;
    const recording = this.editor.recording || this.editor.recordBusy;
    $("#kb-save").disabled = recording || !this.dirty || this.saving || !this.status?.connected;
    $("#kb-reset").disabled = recording || !this.dirty || this.saving;
    $("#kb-feedback").textContent = this.saving ? "Applying…" : this.dirty ? "Unsaved changes" : this.status?.message || "";
  },

  render() {
    const s = this.status;
    $$('.dev-tabs [data-dev="keyboard"]').forEach((tab) => tab.classList.toggle("off", !s?.connected));
    $("#kb-state").textContent = s?.connected
      ? `${s.transport} · ${s.voltage_mv != null ? (s.voltage_mv / 1000).toFixed(2) + " V · " : ""}Active bank: ${s.bank}`
      : "G915 not detected. Wake the keyboard and connect USB, LIGHTSPEED, or Bluetooth.";
    $("#kb-controls").disabled = !s?.connected || this.saving;
    if (!this.draft) return;
    // A saved option can always be turned off after switching transports.
    $("#kb-enabled").disabled = !s?.gkeys_supported && !this.draft.enabled;
    $("#kb-lighting").disabled = !s?.lighting_supported && this.draft.color == null;
    $("#kb-brightness-on").disabled = !s?.brightness_supported && this.draft.brightness == null;
    // Do not replace the macro editor while the user is typing.
    if (!this.dirty && !this.editor.recording && !this.editor.recordBusy) {
      $("#kb-enabled").checked = this.draft.enabled;
      $("#kb-lighting").checked = this.draft.color != null;
      $("#kb-color").value = this.draft.color || "#1e56e4";
      $("#kb-brightness-on").checked = this.draft.brightness != null;
      $("#kb-brightness").value = this.draft.brightness ?? 100;
      $("#kb-brightness-value").textContent = (this.draft.brightness ?? 100) + "%";
      this.editor.render();
    }
    this.renderKeys();
    this.renderButtons();
  },
};
