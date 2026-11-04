# SignalRGB idle lighting preview (Windows)

This preview lets SignalRGB supply colors while G Nexus continues to handle
USB devices, keys, profiles, the G13 LCD, and microphone controls. SignalRGB's
native USB plugins must be disabled for these same devices: there must be one
hardware controller, otherwise the applications can overwrite each other.
SignalRGB continues controlling your other PC lighting normally.

The add-on and Windows build have automated coverage. Running the add-on in
SignalRGB and streaming to physical devices still require validation on HAL.
Yeti GX RGB support also awaits its first physical test.

## One-time setup on HAL

1. Extract the complete preview ZIP. Keep `WebView2Loader.dll` beside the app.
2. Quit the old G Nexus through its system-tray menu and open this build.
3. In SignalRGB, disable the original physical G915, G13, G502 X PLUS and
   Yeti GX devices that G Nexus will manage. Do not disable unrelated PC devices.
   Whitelist G Nexus in SignalRGB's conflicting-program settings if it appears.
4. Copy `SignalRGB/GNexus.js` and `SignalRGB/GNexus.qml` from the preview into
   your SignalRGB user plugin directory, normally
   `%USERPROFILE%\Documents\WhirlwindFX\Plugins` (or its OneDrive Documents
   equivalent). This is the directory for persistent custom plugins, not
   SignalRGB's versioned installation directory. Restart SignalRGB.
5. Look under Third Party Services for **G Nexus Lighting Bridge**. Put the
   `G Nexus · …` virtual devices on your SignalRGB layout. Only the virtual
   devices should send their canvas colors to G Nexus. A recent SignalRGB
   release with UDP network module support is required.
6. In G Nexus, open **Settings → Lighting**, enable the bridge, choose which
   devices participate and save. The default timeout is **10 minutes**.
7. Choose **Use SignalRGB** for a quick test; verify the stream names appear
   and the colors animate. Choose **Use G Nexus** and verify your saved colors
   return. Finally choose **Automatic** to enable idle switching.

SignalRGB Pro is available on your HAL setup. This bridge uses the supported
network-plugin transport rather than undocumented per-device REST endpoints;
it does not need your account credentials or change SignalRGB's global canvas.

## Behavior

- **Automatic:** after the configured idle time (1–240 minutes), connected
  streams supply colors for the selected devices. Mouse movement, a click or
  keyboard input restores G Nexus colors, normally within a few tenths of a
  second plus the device's USB update time. Idle detection is Windows-only.
- **Use SignalRGB:** manually selects the incoming stream until another mode
  is chosen, including while you use the keyboard or mouse.
- **Use G Nexus:** manually keeps G Nexus colors until another mode is chosen.
- A stream missing for **2 seconds** falls back to the G Nexus colors for that
  device. Manual SignalRGB mode resumes its stream if it returns; Automatic
  mode resumes it only while idle. Idle detection failure keeps Automatic on
  G Nexus. No incoming frames means your current colors stay in place.
- Settings and device choices are saved in `lighting.json` and included in
  configuration backups. Manual mode is temporary; startup returns to Automatic.
- Lighting source changes do not replace mappings, change DPI, switch profile
  banks, record audio, change mic gain/mute, or write onboard flash.

## Key assignments and Stream Deck

In a G13/G915 G-key or G915 regular-key assignment, open **MACROS**, then select
**Use SignalRGB lighting**, **Use G Nexus lighting**, or **Automatic lighting**.
These are one-shot lighting actions; they do not synthesize a shortcut.

For Stream Deck, use **System → Open** to run your installed `g13-studio.exe`
with one of these arguments (or put the complete command in a shortcut):

```text
"C:\path\to\g13-studio.exe" --lighting signalrgb
"C:\path\to\g13-studio.exe" --lighting nexus
"C:\path\to\g13-studio.exe" --lighting auto
```

G Nexus must already be running. These commands use its local control channel,
return success/failure, and exit before starting any HID threads. They do not
open another main window. Paths in these examples are placeholders.

## Device limits

| Device | SignalRGB input |
| --- | --- |
| G915 original full size | Individual key/media/G-key/logo colors using the existing physical layout; firmware ignores absent LED positions. ANSI layout is used for the preview, with the ISO extra key included. |
| G13 | One backlight color; its LCD stays under G Nexus control. |
| G502 X PLUS | Eight individual LEDs in firmware order; fine orientation on the SignalRGB canvas needs physical validation. |
| Yeti GX | Two numbered zones, filtered to those actually discovered; RGB protocol/zone placement still needs hardware validation. |
| PRO X Wireless | No controllable RGB lighting; audio controls continue normally. |

The first preview updates hardware at up to roughly 10 frames per second,
bounded by USB round trips. It prioritizes keeping device input responsive.
Bluetooth/receiver capabilities remain subject to the device's HID++ support.
Virtual devices appear in SignalRGB even if their hardware is unplugged; only
selected, connected hardware receives colors.

## Troubleshooting

If no stream names appear in Settings → Lighting, check the add-on is loaded
and its virtual devices are enabled in SignalRGB. Review SignalRGB's plugin
console for network-module errors. The receiver binds only to `127.0.0.1:19738`
(UDP); there is no LAN/cloud access. Local commands use TCP `127.0.0.1:19739`.
Port conflicts are reported in Settings or the G Nexus log. No firewall opening
to the LAN is needed. Stop an unrecognized color sender before enabling the
bridge: loopback color packets are local, unauthenticated color data.

For flickering, ensure SignalRGB's original physical device plugin is disabled,
G HUB is closed, and Windows Dynamic Lighting is not also managing the device.
Turning the bridge off restores the G Nexus source; the normal lighting settings
are never replaced by streamed frames.

## Sources and development

- [SignalRGB network communication API](https://docs.signalrgb.com/developer/plugins/advanced-communication/)
- [Persistent user plugin directory](https://docs.signalrgb.com/developer/plugins/how-is-a-plugin-loaded/)
- [SignalRGB plugin lifecycle](https://docs.signalrgb.com/developer/plugins/)
- [SignalRGB REST lighting semantics](https://docs.signalrgb.com/developer/signalrgb-api/lighting/)
- [OpenRGB Bridge author implementation](https://github.com/Fefedu973/SignalRGB-To-OpenRGB-Bridge)
  was consulted for network discovery/controller registration patterns; no
  OpenRGB installation, service, code, or protocol is used by this bridge.

Regenerate the add-on's physical keyboard metadata with
`node tools/build-signalrgb-layout.cjs`. Tests:
`node tools/signalrgb_addon_smoke.cjs`, `node tools/lighting_ui_smoke.cjs`
(UI server/Playwright required), and the Rust workspace tests.
