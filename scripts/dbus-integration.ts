import assert from "node:assert/strict";
import * as dbus from "dbus-next";
import { BluetoothService } from "../bluez";

// Run only against a throwaway private bus, never the actual system bus.
if (
  !process.env.DBUS_SESSION_BUS_ADDRESS ||
  process.env.DBUS_SYSTEM_BUS_ADDRESS !== process.env.DBUS_SESSION_BUS_ADDRESS
) {
  throw new Error(
    "Run with: dbus-run-session -- sh -c 'DBUS_SYSTEM_BUS_ADDRESS=\"$DBUS_SESSION_BUS_ADDRESS\" bun scripts/dbus-integration.ts'",
  );
}
function emitChanged(
  iface: dbus.interface.Interface,
  properties: Record<string, unknown>,
) {
  dbus.interface.Interface.emitPropertiesChanged(iface, properties, []);
}
const bus = dbus.sessionBus();
const A = "/org/bluez/hci0",
  D = A + "/dev_11_22_33_44_55_66";
const calls: string[] = [];
let agentPath = "",
  agentOwner = "",
  removed = false;
bus.on("message", (message) => {
  if (message.type === 1 && message.member === "RegisterAgent")
    agentOwner = message.sender;
});
const v = (signature: string, value: unknown) =>
  new dbus.Variant(signature, value);
class Adapter extends dbus.interface.Interface {
  constructor() {
    super("org.bluez.Adapter1");
  }
  private power = true;
  private visible = false;
  Discovering = false;
  get Powered() {
    return this.power;
  }
  set Powered(value: boolean) {
    calls.push("Powered:" + value);
    this.power = value;
    emitChanged(this, { Powered: value });
  }
  get Discoverable() {
    return this.visible;
  }
  set Discoverable(value: boolean) {
    calls.push("Discoverable:" + value);
    this.visible = value;
    emitChanged(this, {
      Discoverable: value,
    });
  }
  StartDiscovery() {
    calls.push("StartDiscovery");
    this.Discovering = true;
    emitChanged(this, { Discovering: true });
  }
  StopDiscovery() {
    calls.push("StopDiscovery");
    this.Discovering = false;
    emitChanged(this, {
      Discovering: false,
    });
  }
  RemoveDevice(path: string) {
    assert.equal(path, D);
    calls.push("RemoveDevice");
    removed = true;
    Manager.emitSignal(manager, "InterfacesRemoved", D, ["org.bluez.Device1"]);
  }
}
Adapter.configureMembers({
  properties: {
    Powered: { signature: "b" },
    Discoverable: { signature: "b" },
    Discovering: { signature: "b", access: dbus.interface.ACCESS_READ },
  },
  methods: {
    StartDiscovery: {},
    StopDiscovery: {},
    RemoveDevice: { inSignature: "o" },
  },
});
class Device extends dbus.interface.Interface {
  constructor() {
    super("org.bluez.Device1");
  }
  Connected = false;
  Paired = false;
  private trust = false;
  get Trusted() {
    return this.trust;
  }
  set Trusted(value: boolean) {
    calls.push("Trusted:" + value);
    this.trust = value;
    emitChanged(this, { Trusted: value });
  }
  Connect() {
    calls.push("Connect");
    this.Connected = true;
    emitChanged(this, { Connected: true });
  }
  Disconnect() {
    calls.push("Disconnect");
    this.Connected = false;
    emitChanged(this, { Connected: false });
  }
  async Pair() {
    calls.push("Pair");
    const agent = (
      await bus.getProxyObject(agentOwner, agentPath)
    ).getInterface("org.bluez.Agent1");
    await agent.RequestConfirmation(D, 123456);
    this.Paired = true;
    emitChanged(this, { Paired: true });
  }
  CancelPairing() {
    calls.push("CancelPairing");
  }
}
Device.configureMembers({
  properties: {
    Connected: { signature: "b", access: dbus.interface.ACCESS_READ },
    Paired: { signature: "b", access: dbus.interface.ACCESS_READ },
    Trusted: { signature: "b" },
  },
  methods: { Connect: {}, Disconnect: {}, Pair: {}, CancelPairing: {} },
});
class AgentManager extends dbus.interface.Interface {
  constructor() {
    super("org.bluez.AgentManager1");
  }
  RegisterAgent(path: string, capability: string) {
    calls.push("RegisterAgent");
    assert.equal(capability, "KeyboardDisplay");
    agentPath = path;
  }
  UnregisterAgent(path: string) {
    calls.push("UnregisterAgent");
    assert.equal(path, agentPath);
    agentPath = "";
  }
}
AgentManager.configureMembers({
  methods: {
    RegisterAgent: { inSignature: "os" },
    UnregisterAgent: { inSignature: "o" },
  },
});
const adapter = new Adapter(),
  device = new Device();
class Manager extends dbus.interface.Interface {
  constructor() {
    super("org.freedesktop.DBus.ObjectManager");
  }
  GetManagedObjects() {
    return {
      [A]: {
        "org.bluez.Adapter1": {
          Alias: v("s", "Test adapter"),
          Address: v("s", "AA:BB:CC:DD:EE:FF"),
          Powered: v("b", adapter.Powered),
          Discoverable: v("b", adapter.Discoverable),
          Discovering: v("b", adapter.Discovering),
          Pairable: v("b", false),
        },
      },
      ...(!removed
        ? {
            [D]: {
              "org.bluez.Device1": {
                Adapter: v("o", A),
                Alias: v("s", "Private bus headset"),
                Address: v("s", "11:22:33:44:55:66"),
                Icon: v("s", "audio-headset"),
                Connected: v("b", device.Connected),
                Paired: v("b", device.Paired),
                Trusted: v("b", device.Trusted),
              },
            },
          }
        : {}),
    };
  }
  InterfacesRemoved(path: string, interfaces: string[]) {
    return [path, interfaces];
  }
  static emitSignal(
    target: Manager,
    member: "InterfacesRemoved",
    path: string,
    interfaces: string[],
  ) {
    target[member](path, interfaces);
  }
}
Manager.configureMembers({
  methods: { GetManagedObjects: { outSignature: "a{oa{sa{sv}}}" } },
  signals: { InterfacesRemoved: { signature: "oas" } },
});
const manager = new Manager();
bus.export("/", manager);
bus.export("/org/bluez", new AgentManager());
bus.export(A, adapter);
bus.export(D, device);
const service = new BluetoothService();
const waitFor = async (condition: () => boolean) => {
  const until = Date.now() + 4000;
  while (!condition()) {
    if (Date.now() > until)
      throw new Error("Timed out waiting for D-Bus signal");
    await Bun.sleep(10);
  }
};
try {
  await bus.requestName("org.bluez", 0);
  await service.start();
  assert.equal(service.getSnapshot().devices[0]?.name, "Private bus headset");
  assert.deepEqual(calls, [], "Startup must only read state");
  await service.setPower(false);
  await waitFor(() => service.getSnapshot().adapters[0]?.powered === false);
  await service.setPower(true);
  await service.setDiscoverable(true);
  await waitFor(() => service.getSnapshot().adapters[0]?.discoverable === true);
  await service.setTrusted(D, true);
  await waitFor(() => service.getSnapshot().devices[0]?.trusted === true);
  await service.startDiscovery();
  await waitFor(() => service.getSnapshot().adapters[0]?.discovering === true);
  await service.stopDiscovery();
  await waitFor(() => service.getSnapshot().adapters[0]?.discovering === false);
  await service.connect(D);
  await waitFor(() => service.getSnapshot().devices[0]?.status === "connected");
  await service.disconnect(D);
  await waitFor(() => service.getSnapshot().devices[0]?.status === "nearby");
  const rejected = service.pair(D).then(
    () => null,
    (error) => error,
  );
  await waitFor(() => service.getSnapshot().prompt?.kind === "confirm");
  service.respondPrompt(false);
  assert(await rejected, "Denied pairing must reject the service call");
  const pairing = service.pair(D);
  await waitFor(() => service.getSnapshot().prompt?.kind === "confirm");
  assert.equal(service.getSnapshot().prompt?.passkey, "123456");
  service.respondPrompt(true);
  await pairing;
  await waitFor(() => service.getSnapshot().devices[0]?.paired === true);
  assert.equal(agentPath, "", "Pairing agent must be unregistered afterward");
  await service.remove(D);
  await waitFor(() => service.getSnapshot().devices.length === 0);
  console.log(
    "PASS real D-Bus transport: read-only startup, properties, discovery, connection, pairing agent confirmation/rejection, removal and signals (private bus only).",
  );
} finally {
  await service.dispose();
  bus.disconnect();
}
