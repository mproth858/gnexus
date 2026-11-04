"""Read-only G915 HID++ probe. Requires Python package hidapi.

Run once per connection (USB, LIGHTSPEED, Bluetooth). No device settings are written.
"""
import json
import time

import hid


def call(device, index, feature, function, params=()):
    header = [0x11, index, feature, (function << 4) | 0x0E]
    device.write(header + list(params) + [0] * (16 - len(params)))
    deadline = time.monotonic() + 0.3
    while time.monotonic() < deadline:
        data = device.read(64, 30)
        if len(data) >= 7 and data[:4] == header:
            return data[4:20]
        if len(data) >= 7 and data[1:5] == [index, 0xFF, feature, header[3]]:
            return None
    return None


def probe():
    results = []
    for info in hid.enumerate(0x046D):
        if info['usage_page'] != 0xFF00 or info['usage'] != 2:
            continue
        device = hid.device()
        try:
            device.open_path(info['path'])
            for index in (0xFF, 1, 2, 3, 4, 5, 6):
                ping = call(device, index, 0, 1, (0, 0, 0x5A))
                if not ping or ping[0] < 2 or ping[2] != 0x5A:
                    continue
                row = {'usb_pid': f'{info["product_id"]:04X}', 'index': index,
                       'protocol': f'{ping[0]}.{ping[1]}', 'features': []}
                fs = call(device, index, 0, 0, (0, 1))
                count = call(device, index, fs[0], 0) if fs and fs[0] else None
                if count:
                    for slot in range(1, count[0] + 1):
                        entry = call(device, index, fs[0], 1, (slot,))
                        if entry:
                            row['features'].append({'index': slot, 'id': f'{entry[0]:02X}{entry[1]:02X}', 'version': entry[3]})
                name = call(device, index, 0, 0, (0, 5))
                length = call(device, index, name[0], 0) if name and name[0] else None
                if length:
                    text = []
                    for offset in range(0, length[0], 16):
                        chunk = call(device, index, name[0], 1, (offset,))
                        if chunk:
                            text.extend(chunk)
                    row['name'] = bytes(text[:length[0]]).decode('utf-8', errors='replace')
                results.append(row)
        except OSError as error:
            results.append({'usb_pid': f'{info["product_id"]:04X}', 'error': str(error)})
        finally:
            device.close()
    print(json.dumps(results, indent=2))


if __name__ == '__main__':
    probe()
