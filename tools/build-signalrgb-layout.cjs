// Regenerate bridge metadata when the keyboard layout or protocol IDs change.
const fs = require('node:fs');
const vm = require('node:vm');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root,file), 'utf8');
const context = vm.createContext({ t: text => text });
vm.runInContext(read('crates/g13-gui/ui/catalog.js') + '\n' + read('crates/g13-gui/ui/g915-layout.js') + '\napplyLayout("us","en");this.board=G915Layout.keys();', context);
const rust = read('crates/g13-daemon/src/keycode.rs');
const names = Object.fromEntries([...rust.matchAll(/\b(\w+) => "([^"]+)",/g)].map(match => [match[1],match[2]]));
const ids = {G1:180,G2:181,G3:182,G4:183,G5:184,Logo:210,Brightness:153,
    MediaPlayPause:155,VolumeMute:156,MediaNext:157,MediaPrevious:158};
for (const match of rust.matchAll(/Self::(\w+) => ([KM])\((0x[\dA-F]+)\)/g)) {
    const value = parseInt(match[3],16);
    if (match[2] === 'K' && value >= 4 && value <= 0x65) ids[names[match[1]]] = value - 3;
    if (match[2] === 'M') ids[names[match[1]]] = 104 + Math.log2(value);
}
const keys = context.board.filter(key => ids[key.key] !== undefined).map(key => ({id:ids[key.key], name:key.key,
    position:[Math.round((key.x+key.w/2)*4),Math.round((key.y+key.h/2)*4)]}));
// Include the ISO extra key at its physical position; firmware filters absent LEDs.
keys.push({id:ids.IntlBackslash,name:'IntlBackslash',position:[14,25]});
const layouts = {
    g13:{name:'G Nexus · G13',size:[1,1],leds:[{id:0,name:'Backlight',position:[0,0]}]},
    g915:{name:'G Nexus · G915',size:[102,34],leds:keys},
    mouse:{name:'G Nexus · G502 X PLUS',size:[9,4],leds:Array.from({length:8},(_,id)=>({id,name:`LED ${id+1}`,position:[id,2]}))},
    microphone:{name:'G Nexus · Yeti GX',size:[4,2],leds:[{id:0,name:'Zone 1',position:[0,0]},{id:1,name:'Zone 2',position:[3,0]}]},
};
if (keys.some(key => key.id === undefined) || new Set(keys.map(key=>key.id)).size !== keys.length) throw Error('Invalid G915 bridge LED map');
const target = path.join(root,'deploy/signalrgb/GNexus.js');
fs.writeFileSync(target,fs.readFileSync(target,'utf8').replace(/\/\* GENERATED_LAYOUT \*\/ [^\n]+/, '/* GENERATED_LAYOUT */ ' + JSON.stringify(layouts) + ';'));
console.log(`Generated ${keys.length} G915 LED positions and 3 other lighting devices.`);
