# Logitech Yeti GX — first implementation

Known USB model: **046d:0afc**, HID++ 2.0 vendor collection **0xff43**.
The microphone is not available on the development PC. This implementation is
ready for a first hardware test; it is not yet verified against the user's mic.
Yeti X, original Yeti and Yeti Nano are different devices and are not included.

## Included

- USB detection/hotplug and a dedicated Yeti GX page in English.
- Hardware gain 0–100, hardware mute/unmute and Smart Audio Lock.
- Readback after each audio write, including bounded retries for firmware latency.
- Physical gain/mute/lock updates read from the mic, rather than inferred from
  USB notifications (their payload layout differs from command replies).
- Passive Windows capture endpoint level and Windows input mute indication.
  A software peak meter needs an active shared-mode stream: open a recording or
  voice app if it stays idle. G Nexus does not record audio or play it back.
- Experimental RGB zone colors, only for zones that advertise a static effect.
  Zone IDs/effect indexes are discovered, not assigned assumed ring/logo names.
- Saved RGB preferences and return to microphone-managed lighting.

The published audio packets were captured and device-tested on firmware 2.00
by [yeti-gx-linux](https://github.com/hicham-bouchikhi/yeti-gx-linux). The app
resolves feature **0x8370** dynamically and uses short HID++ requests for its
fields. Mute has inverted wire polarity (0 muted, 1 active). Smart Audio Lock
reads field 2 but writes field 5 with values 6/7. Unsupported fields stay disabled.
RGB uses the existing generic 0x8071 implementation and needs physical validation.

Blue VO!CE EQ, noise reduction, de-essing and other voice processing are not
implemented in this version. Smart Audio Lock is the exposed hardware control;
it does not imply a replacement for the Blue VO!CE processing suite.

## First test at home

1. Quit G HUB from its tray menu.
2. Connect the Yeti GX by USB. Open G Nexus and choose **Yeti GX**.
3. Confirm gain, mute and Smart Audio Lock reflect their actual device state.
4. Adjust gain, apply, and check that the value is read back. Test mute/unmute
   and lock, including the physical buttons/wheel, using a voice/recording app.
5. Test RGB zones separately; note which zone changes the ring or logo. Those
   physical names can be added once confirmed on the real device.
6. For the meter, use a shared-mode recording stream. A software peak meter
   cannot show an exclusive-mode stream, per the Windows audio API.

To get a read-only diagnostic report, quit G Nexus and G HUB first, then run:

```powershell
cargo run -p g13-daemon --bin yeti_gx_probe
```

Or run the built `yeti_gx_probe.exe` without Rust installed. The report contains
USB collections, feature IDs, raw microphone field replies, RGB information and
Windows input state. It excludes USB serial numbers and records no audio.
The probe uses a different HID++ software ID from the running app, and never
loads/applies stored settings. Share its JSON if a control or lighting zone is
unavailable or behaves differently. Other Logitech Yeti USB IDs are listed in
the diagnostic inventory but are not automatically treated as a GX.

## Ownership and persistence

The `yeti-gx` worker exclusively owns its HID handle. Audio commands never run on
the input-engine thread. G HUB blocks writes; starting it releases claimed RGB.
Disconnect discards pending writes. Shutdown releases RGB control and does not
change gain, mute or Smart Audio Lock. Audio preferences are not replayed on
startup or reconnect, so a mic is never automatically unmuted by opening the app.
Only RGB is saved in `microphone.json`; backups include that file. A saved RGB
profile can be reapplied after another program released its software control.

Linux uses the same USB control path with the added uaccess rule. The input
level meter is Windows-only in this first version. Standard USB audio remains
on the operating system's audio path.

Sources: [Logitech product page](https://www.logitechg.com/en-gb/shop/p/yeti-gx-gaming-microphone),
[Logitech setup guide](https://www.logitech.com/assets/70062/4/yeti_gx_web_qsg_amr.pdf),
[published device captures](https://github.com/hicham-bouchikhi/yeti-gx-linux),
[Windows peak-meter API](https://learn.microsoft.com/en-us/windows/win32/api/endpointvolume/nn-endpointvolume-iaudiometerinformation).
