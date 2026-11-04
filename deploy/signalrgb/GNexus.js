// G Nexus canvas source. This add-on NEVER opens USB/HID devices.
import * as udpModule from "@SignalRGB/udp";
const udp = udpModule.udp || udpModule.default;
export function Name() { return "G Nexus Lighting Bridge"; }
export function Version() { return "1.0.0"; }
export function Publisher() { return "G Nexus"; }
export function Type() { return "network"; }
export function Size() { return [1, 1]; }
export function DefaultPosition() { return [0, 0]; }
export function DefaultScale() { return 1; }

// Generated from G Nexus's physical G915 layout and Rust LED identifiers.
const LAYOUTS = /* GENERATED_LAYOUT */ {"g13":{"name":"G Nexus · G13","size":[1,1],"leds":[{"id":0,"name":"Backlight","position":[0,0]}]},"g915":{"name":"G Nexus · G915","size":[102,34],"leds":[{"id":210,"name":"Logo","position":[4,3]},{"id":153,"name":"Brightness","position":[46,3]},{"id":158,"name":"MediaPrevious","position":[85,7]},{"id":155,"name":"MediaPlayPause","position":[89,7]},{"id":157,"name":"MediaNext","position":[93,7]},{"id":156,"name":"VolumeMute","position":[97,7]},{"id":38,"name":"Escape","position":[9,8]},{"id":55,"name":"F1","position":[16,8]},{"id":56,"name":"F2","position":[20,8]},{"id":57,"name":"F3","position":[24,8]},{"id":58,"name":"F4","position":[28,8]},{"id":59,"name":"F5","position":[34,8]},{"id":60,"name":"F6","position":[38,8]},{"id":61,"name":"F7","position":[42,8]},{"id":62,"name":"F8","position":[46,8]},{"id":63,"name":"F9","position":[53,8]},{"id":64,"name":"F10","position":[57,8]},{"id":65,"name":"F11","position":[61,8]},{"id":66,"name":"F12","position":[65,8]},{"id":67,"name":"PrintScreen","position":[71,8]},{"id":68,"name":"ScrollLock","position":[75,8]},{"id":69,"name":"Pause","position":[79,8]},{"id":180,"name":"G1","position":[4,13]},{"id":181,"name":"G2","position":[4,17]},{"id":182,"name":"G3","position":[4,21]},{"id":183,"name":"G4","position":[4,25]},{"id":184,"name":"G5","position":[4,29]},{"id":50,"name":"Grave","position":[9,13]},{"id":27,"name":"1","position":[13,13]},{"id":28,"name":"2","position":[17,13]},{"id":29,"name":"3","position":[21,13]},{"id":30,"name":"4","position":[25,13]},{"id":31,"name":"5","position":[29,13]},{"id":32,"name":"6","position":[33,13]},{"id":33,"name":"7","position":[37,13]},{"id":34,"name":"8","position":[41,13]},{"id":35,"name":"9","position":[45,13]},{"id":36,"name":"0","position":[49,13]},{"id":42,"name":"Minus","position":[53,13]},{"id":43,"name":"Equal","position":[57,13]},{"id":39,"name":"Backspace","position":[63,13]},{"id":40,"name":"Tab","position":[10,17]},{"id":17,"name":"Q","position":[15,17]},{"id":23,"name":"W","position":[19,17]},{"id":5,"name":"E","position":[23,17]},{"id":18,"name":"R","position":[27,17]},{"id":20,"name":"T","position":[31,17]},{"id":25,"name":"Y","position":[35,17]},{"id":21,"name":"U","position":[39,17]},{"id":9,"name":"I","position":[43,17]},{"id":15,"name":"O","position":[47,17]},{"id":16,"name":"P","position":[51,17]},{"id":44,"name":"LeftBracket","position":[55,17]},{"id":45,"name":"RightBracket","position":[59,17]},{"id":46,"name":"Backslash","position":[64,17]},{"id":54,"name":"CapsLock","position":[10,21]},{"id":1,"name":"A","position":[16,21]},{"id":19,"name":"S","position":[20,21]},{"id":4,"name":"D","position":[24,21]},{"id":6,"name":"F","position":[28,21]},{"id":7,"name":"G","position":[32,21]},{"id":8,"name":"H","position":[36,21]},{"id":10,"name":"J","position":[40,21]},{"id":11,"name":"K","position":[44,21]},{"id":12,"name":"L","position":[48,21]},{"id":48,"name":"Semicolon","position":[52,21]},{"id":49,"name":"Apostrophe","position":[56,21]},{"id":37,"name":"Enter","position":[62,21]},{"id":105,"name":"LeftShift","position":[11,25]},{"id":26,"name":"Z","position":[18,25]},{"id":24,"name":"X","position":[22,25]},{"id":3,"name":"C","position":[26,25]},{"id":22,"name":"V","position":[30,25]},{"id":2,"name":"B","position":[34,25]},{"id":14,"name":"N","position":[38,25]},{"id":13,"name":"M","position":[42,25]},{"id":51,"name":"Comma","position":[46,25]},{"id":52,"name":"Period","position":[50,25]},{"id":53,"name":"Slash","position":[54,25]},{"id":109,"name":"RightShift","position":[61,25]},{"id":104,"name":"LeftCtrl","position":[10,29]},{"id":107,"name":"LeftMeta","position":[15,29]},{"id":106,"name":"LeftAlt","position":[20,29]},{"id":41,"name":"Space","position":[34,29]},{"id":110,"name":"RightAlt","position":[48,29]},{"id":111,"name":"RightMeta","position":[53,29]},{"id":98,"name":"Menu","position":[58,29]},{"id":108,"name":"RightCtrl","position":[64,29]},{"id":70,"name":"Insert","position":[71,13]},{"id":71,"name":"Home","position":[75,13]},{"id":72,"name":"PageUp","position":[79,13]},{"id":73,"name":"Delete","position":[71,17]},{"id":74,"name":"End","position":[75,17]},{"id":75,"name":"PageDown","position":[79,17]},{"id":79,"name":"Up","position":[75,25]},{"id":77,"name":"Left","position":[71,29]},{"id":78,"name":"Down","position":[75,29]},{"id":76,"name":"Right","position":[79,29]},{"id":80,"name":"NumLock","position":[85,13]},{"id":81,"name":"NumpadDivide","position":[89,13]},{"id":82,"name":"NumpadMultiply","position":[93,13]},{"id":83,"name":"NumpadSubtract","position":[97,13]},{"id":92,"name":"Numpad7","position":[85,17]},{"id":93,"name":"Numpad8","position":[89,17]},{"id":94,"name":"Numpad9","position":[93,17]},{"id":89,"name":"Numpad4","position":[85,21]},{"id":90,"name":"Numpad5","position":[89,21]},{"id":91,"name":"Numpad6","position":[93,21]},{"id":84,"name":"NumpadAdd","position":[97,19]},{"id":86,"name":"Numpad1","position":[85,25]},{"id":87,"name":"Numpad2","position":[89,25]},{"id":88,"name":"Numpad3","position":[93,25]},{"id":85,"name":"NumpadEnter","position":[97,27]},{"id":95,"name":"Numpad0","position":[87,29]},{"id":96,"name":"NumpadDecimal","position":[93,29]},{"id":97,"name":"IntlBackslash","position":[14,25]}]},"mouse":{"name":"G Nexus · G502 X PLUS","size":[9,4],"leds":[{"id":0,"name":"LED 1","position":[0,2]},{"id":1,"name":"LED 2","position":[1,2]},{"id":2,"name":"LED 3","position":[2,2]},{"id":3,"name":"LED 4","position":[3,2]},{"id":4,"name":"LED 5","position":[4,2]},{"id":5,"name":"LED 6","position":[5,2]},{"id":6,"name":"LED 7","position":[6,2]},{"id":7,"name":"LED 8","position":[7,2]}]},"microphone":{"name":"G Nexus · Yeti GX","size":[4,2],"leds":[{"id":0,"name":"Zone 1","position":[0,0]},{"id":1,"name":"Zone 2","position":[3,0]}]}};
let socket;
let layout;
export function Initialize() {
    layout = LAYOUTS[controller.nexusDevice];
    if (!layout || !udp || !udp.createSocket) {
        device.log("G Nexus bridge needs SignalRGB UDP network-module support."); return;
    }
    device.setName(layout.name);
    device.setSize(layout.size);
    device.setControllableLeds(layout.leds.map(led => led.name), layout.leds.map(led => led.position));
    socket = udp.createSocket();
    socket.connect("127.0.0.1", 19738);
}
export function Render() {
    if (!socket || !layout) return;
    const colors = layout.leds.map(led => [led.id, ...device.color(led.position[0], led.position[1])]);
    const packet = JSON.stringify({version:1, device:controller.nexusDevice, colors});
    // ASCII JSON as bytes works across network module string/byte variants.
    socket.send(Array.from(packet, character => character.charCodeAt(0)));
    device.pause(50);
}
export function Shutdown() {
    // No shutdown colors: G Nexus restores its settings when the stream expires.
    if (socket) socket.close(); socket = undefined;
}

export function DiscoveryService() {
    this.Initialize = function () {
        for (const id of Object.keys(LAYOUTS)) {
            const entry = {id:"gnexus-" + id, name:LAYOUTS[id].name, nexusDevice:id,
                vendor:"G Nexus", description:"SignalRGB canvas through G Nexus; no direct USB access"};
            service.addController(entry);
            service.announceController(entry);
        }
    };
    this.Update = function () {};
}
