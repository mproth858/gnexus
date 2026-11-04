"use strict";

// Keyboard input delivered to the web view can bypass Windows' global hooks
// (for example, a remote-control tool can post window messages directly).
// Keep a local capture while the app has focus, and use it only when the native
// recorder returns no keyboard events. Never combine both copies of a key.
class WindowMacroCapture {
  constructor(changed) {
    this.changed = changed;
    this.events = [];
    this.held = new Set();
    this.started = performance.now();
    this.listener = (event) => this.onKey(event);
    this.blur = () => this.release();
    document.addEventListener("keydown", this.listener, true);
    document.addEventListener("keyup", this.listener, true);
    window.addEventListener("blur", this.blur);
  }

  static key(code) {
    const renamed = { ControlLeft: "LeftCtrl", ControlRight: "RightCtrl", ShiftLeft: "LeftShift", ShiftRight: "RightShift",
      AltLeft: "LeftAlt", AltRight: "RightAlt", MetaLeft: "LeftMeta", MetaRight: "RightMeta", ContextMenu: "Menu",
      ArrowLeft: "Left", ArrowRight: "Right", ArrowUp: "Up", ArrowDown: "Down", Backquote: "Grave",
      BracketLeft: "LeftBracket", BracketRight: "RightBracket", Quote: "Apostrophe" };
    const key = renamed[code] || code.replace(/^Key([A-Z])$/, "$1").replace(/^Digit([0-9])$/, "$1");
    return ALL_CODES.includes(key) ? key : null;
  }

  onKey(event) {
    const key = WindowMacroCapture.key(event.code);
    if (!key || event.isComposing) return;
    // Stop shortcuts such as Ctrl+C and Escape from changing the editor while recording.
    event.preventDefault();
    event.stopImmediatePropagation();
    const pressed = event.type === "keydown";
    if (pressed === this.held.has(key) || this.events.length >= 100000) return;
    if (pressed) this.held.add(key); else this.held.delete(key);
    this.events.push({ at: performance.now() - this.started, step: `${pressed ? "down" : "up"} ${key}` });
    this.changed(this.events.length);
  }

  release() {
    for (const key of [...this.held].reverse()) this.events.push({ at: performance.now() - this.started, step: `up ${key}` });
    this.held.clear();
  }

  stop(delays) {
    document.removeEventListener("keydown", this.listener, true);
    document.removeEventListener("keyup", this.listener, true);
    window.removeEventListener("blur", this.blur);
    this.release();
    const steps = [];
    let previous = null;
    for (const event of this.events) {
      if (previous != null) {
        const wait = delays ? Math.max(0, Math.round(event.at - previous)) : 25;
        if (wait) steps.push(`wait ${wait}`);
      }
      steps.push(event.step);
      previous = event.at;
    }
    return steps;
  }
}
