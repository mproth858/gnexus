"use strict";

// Original full-size G915, measured against Logitech's top-view gallery image
// and quick-start guide (65840). Coordinates are keycap units, not a key list:
// the G915 has 1.5u Ctrl keys and a 5.75u spacebar. This is not the G915 X/TKL.
const G915Layout = {
  width: 25.3,
  height: 8.1,

  keys() {
    const keys = [];
    const add = (key, x, y, w = 1, h = 1, label = null, kind = "key") =>
      keys.push({ key, x, y, w, h, label, kind });
    const main = 1.7, nav = 17.2, pad = 20.7;
    const row = (items, y, start = main) => {
      let x = start;
      for (const item of items) {
        const [key, w = 1] = Array.isArray(item) ? item : [item];
        add(key, x, y, w); x += w;
      }
    };
    add("Logo", .4, .2, 1, .85, "G", "logo");
    ["M1", "M2", "M3", "MR"].forEach((key, i) => add(key, main + 1.8 + i * .95, .3, .7, .7, key, "hardware"));
    [["Lightspeed", "◉"], ["Bluetooth", "BT"], ["GameMode", "Game"], ["Brightness", "☀"]]
      .forEach(([key, label], i) => add(key, main + 6.5 + i * .95, .3, .7, .7, label, key === "Brightness" ? "round" : "hardware"));
    add("CapsIndicator", main + 11.5, .3, 1, .7, "CAPS ●", "indicator");
    add("BatteryIndicator", main + 12.8, .3, 1, .7, "BATTERY ●", "indicator");
    add("WirelessLabel", nav + .45, .15, 2.1, .8, "LIGHTSPEED", "indicator");
    add("VolumeRoller", pad + .3, .12, 3.4, .75, "Volume roller", "roller");
    ["MediaPrevious", "MediaPlayPause", "MediaNext", "VolumeMute"].forEach((key, i) =>
      add(key, pad + .15 + i, 1.45, .78, .78, { MediaPrevious: "⏮", MediaPlayPause: "⏯", MediaNext: "⏭", VolumeMute: "Mute" }[key], "round"));
    add("Escape", main, 1.4);
    for (let i = 0; i < 12; i++) add(`F${i + 1}`, main + 1.75 + i + Math.floor(i / 4) * .65, 1.4);
    ["PrintScreen", "ScrollLock", "Pause"].forEach((key, i) => add(key, nav + i, 1.4));
    for (let i = 0; i < 5; i++) add(`G${i + 1}`, .4, 2.8 + i);
    row(["Grave", ...DIGITS, "Minus", "Equal", ["Backspace", 2]], 2.8);
    const iso = KEYBOARD_ROWS.some((r) => r.some(([key]) => key === "IntlBackslash"));
    row([["Tab", 1.5], ...LETTERS1, "LeftBracket", "RightBracket", ...(iso ? [] : [["Backslash", 1.5]])], 3.8);
    row([["CapsLock", 1.75], ...LETTERS2, "Semicolon", "Apostrophe", ...(iso ? ["Backslash"] : [["Enter", 2.25]])], 4.8);
    if (iso) add("Enter", main + 13.5, 3.8, 1.5, 2, "↵", "iso-enter");
    row([["LeftShift", iso ? 1.25 : 2.25], ...(iso ? ["IntlBackslash"] : []), ...LETTERS3, "Comma", "Period", "Slash", ["RightShift", 2.75]], 5.8);
    row([["LeftCtrl", 1.5], ["LeftMeta", 1.25], ["LeftAlt", 1.25], ["Space", 5.75],
      ["RightAlt", 1.25], ["RightMeta", 1.25], ["Menu", 1.25], ["RightCtrl", 1.5]], 6.8);
    row(["Insert", "Home", "PageUp"], 2.8, nav);
    row(["Delete", "End", "PageDown"], 3.8, nav);
    add("Up", nav + 1, 5.8);
    row(["Left", "Down", "Right"], 6.8, nav);
    row(["NumLock", "NumpadDivide", "NumpadMultiply", "NumpadSubtract"], 2.8, pad);
    row(["Numpad7", "Numpad8", "Numpad9"], 3.8, pad);
    row(["Numpad4", "Numpad5", "Numpad6"], 4.8, pad);
    add("NumpadAdd", pad + 3, 3.8, 1, 2);
    row(["Numpad1", "Numpad2", "Numpad3"], 5.8, pad);
    add("NumpadEnter", pad + 3, 5.8, 1, 2);
    row([["Numpad0", 2], "NumpadDecimal"], 6.8, pad);
    return keys;
  },

  label(key) {
    const short = { LeftCtrl: "Ctrl", RightCtrl: "Ctrl", LeftMeta: "Win", RightMeta: "Win", LeftAlt: "Alt", RightAlt: "Alt",
      LeftShift: "Shift", RightShift: "Shift", CapsLock: "Caps", Space: "", Enter: "↵", Menu: "☰",
      NumLock: "Num", NumpadDivide: "/", NumpadMultiply: "*", NumpadSubtract: "−", NumpadAdd: "+", NumpadEnter: "↵", NumpadDecimal: "." };
    return short[key] ?? (key.startsWith("Numpad") ? key.slice(6) : KEY_LABELS[key] || key);
  },

  render(button) {
    return `<div class="g915-board" aria-label="G915 full-size keyboard">${this.keys().map((item) => {
      const { key, x, y, w, h, kind } = item;
      const label = item.label ?? this.label(key);
      const position = `left:${x / this.width * 100}%;top:${y / this.height * 100}%;width:${(w - .1) / this.width * 100}%;height:${(h - .1) / this.height * 100}%;`;
      if (["hardware", "indicator", "roller"].includes(kind)) {
        const name = { Lightspeed: "LIGHTSPEED", Bluetooth: "Bluetooth", GameMode: "Game mode" }[key] || label;
        const description = kind === "roller" ? "Volume roller · no individual RGB LED" : `${name} · hardware control / indicator`;
        return `<span class="g915-cap g915-${kind}" data-hardware="${key}" style="${position}" title="${esc(description)}" aria-label="${esc(description)}">${esc(label)}</span>`;
      }
      return button(key, label, position, `g915-cap g915-${kind}`);
    }).join("")}</div>`;
  },
};
