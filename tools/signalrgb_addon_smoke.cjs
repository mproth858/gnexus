const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const code = fs.readFileSync(require('node:path').join(__dirname,'../deploy/signalrgb/GNexus.js'),'utf8')
    .replace(/^import .*;$/m,'const udpModule = {udp:fakeUdp};').replace(/export /g,'');
for (const id of ['g13','g915','mouse','microphone']) {
    let packet, closed = false;
    const controllers = [];
    const context = vm.createContext({ controller:{nexusDevice:id}, fakeUdp:{createSocket:()=>({
        connect:(host,port)=>{assert.equal(host,'127.0.0.1');assert.equal(port,19738);},
        send:bytes=>{packet=JSON.parse(String.fromCharCode(...bytes));}, close:()=>{closed=true;},
    })}, device:{setName:()=>{},setSize:()=>{},setControllableLeds:(names,positions)=>assert.equal(names.length,positions.length),
        color:()=>[20,40,60],pause:()=>{},log:message=>{throw Error(message);}},
        service:{addController:entry=>controllers.push(entry),announceController:()=>{}} });
    vm.runInContext(code+'\nconst discovery = new DiscoveryService();discovery.Initialize();Initialize();Render();Shutdown();',context);
    assert.equal(controllers.length,4);
    assert.equal(packet.device,id); assert.equal(packet.version,1); assert.equal(closed,true);
    assert.ok(Buffer.byteLength(JSON.stringify(packet))<8192);
    assert.equal(new Set(packet.colors.map(color=>color[0])).size,packet.colors.length);
    assert.ok(packet.colors.every(color=>color.length===4 && color.slice(1).join(',')==='20,40,60'));
    if (id==='g915') for (const led of [1,23,104,180,184,155,210]) assert.ok(packet.colors.some(color=>color[0]===led));
}
console.log('SignalRGB addon: four controllers, per-key IDs, loopback frames and shutdown passed.');
