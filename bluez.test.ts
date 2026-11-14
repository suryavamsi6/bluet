import { describe, expect, test } from "bun:test";
import {
  BluetoothService,
  mapObjects,
  type BluetoothTransport,
  type ManagedObjects,
} from "./bluez";
const v = (value: unknown) => ({ signature: "x", value });
const fixture = (): ManagedObjects => ({
  "/a": {
    "org.bluez.Adapter1": {
      Alias: v("Adapter"),
      Address: v("AA"),
      Powered: v(true),
      Discoverable: v(false),
      Discovering: v(false),
      Pairable: v(true),
    },
  },
  "/b": { "org.bluez.Adapter1": { Name: v("Other"), Address: v("BB") } },
  "/a/d": {
    "org.bluez.Device1": {
      Adapter: v("/a"),
      Alias: v("Mouse"),
      Address: v("12:34"),
      Icon: v("input-mouse"),
      Paired: v(true),
      Connected: v(false),
      Trusted: v(false),
      RSSI: v(-50),
    },
    "org.bluez.Battery1": { Percentage: v(70) },
  },
  "/b/d": {
    "org.bluez.Device1": {
      Adapter: v("/b"),
      Name: v("Headset"),
      Icon: v("audio-headset"),
      Connected: v(true),
    },
  },
});
class Fake implements BluetoothTransport {
  data = fixture();
  calls: any[][] = [];
  signal: (s: any) => void = () => {};
  handler: any;
  failure = false;
  async start(signal: (s: any) => void, _failed: (e: unknown) => void) {
    this.signal = signal;
  }
  async objects() {
    if (this.failure) throw new Error("BlueZ unavailable");
    return structuredClone(this.data);
  }
  async call(...args: any[]) {
    this.calls.push(args);
    if (this.failure) throw new Error("Operation failed");
  }
  async set(...args: [string, string, string, boolean]) {
    this.calls.push(args);
    if (this.failure) throw new Error("Operation failed");
  }
  async registerAgent(handler: any) {
    this.calls.push(["register"]);
    this.handler = handler;
  }
  async unregisterAgent() {
    this.calls.push(["unregister"]);
  }
  async close() {
    this.calls.push(["close"]);
  }
}
describe("BlueZ service", () => {
  test("maps real properties, preserving unknown connection time", () => {
    const { adapters, devices } = mapObjects(fixture());
    expect(adapters[0]).toMatchObject({
      id: "/a",
      powered: true,
      pairable: true,
    });
    expect(devices[0]).toMatchObject({
      kind: "mouse",
      status: "saved",
      paired: true,
      battery: 70,
      rssi: -50,
    });
    expect(devices[1].connectedSince).toBeUndefined();
  });
  test("startup is read-only, signals update state, and commands target BlueZ", async () => {
    const fake = new Fake(),
      service = new BluetoothService(fake);
    await service.start();
    expect(fake.calls).toEqual([]);
    expect(service.getSnapshot().devices.map((d) => d.id)).toEqual(["/a/d"]);
    fake.signal({
      path: "/a/d",
      member: "PropertiesChanged",
      body: ["org.bluez.Device1", { Connected: v(true) }, []],
    });
    expect(service.getSnapshot().devices[0]).toMatchObject({
      status: "connected",
      connectedSince: expect.any(Number),
    });
    await service.setTrusted("/a/d", true);
    await service.connect("/a/d");
    await service.remove("/a/d");
    expect(fake.calls).toEqual([
      ["/a/d", "org.bluez.Device1", "Trusted", true],
      ["/a/d", "org.bluez.Device1", "Connect"],
      ["/a", "org.bluez.Adapter1", "RemoveDevice", "/a/d"],
    ]);
    service.selectAdapter("/b");
    expect(service.getSnapshot().devices[0].name).toBe("Headset");
    await service.dispose();
  });
  test("only stops discovery owned by this client and reports errors", async () => {
    const fake = new Fake(),
      service = new BluetoothService(fake);
    await service.start();
    await service.stopDiscovery();
    await service.startDiscovery();
    service.selectAdapter("/b");
    await service.dispose();
    expect(fake.calls).toContainEqual([
      "/a",
      "org.bluez.Adapter1",
      "StopDiscovery",
    ]);
    expect(fake.calls).not.toContainEqual([
      "/b",
      "org.bluez.Adapter1",
      "StopDiscovery",
    ]);
    const failing = new Fake();
    failing.failure = true;
    const other = new BluetoothService(failing);
    await other.start();
    expect(other.getSnapshot()).toMatchObject({
      available: false,
      error: "BlueZ unavailable",
    });
  });
  test("pairing registers its own agent and releases it, and action failures surface", async () => {
    const fake = new Fake(),
      service = new BluetoothService(fake);
    await service.start();
    await service.pair("/a/d");
    expect(fake.calls).toEqual([
      ["register"],
      ["/a/d", "org.bluez.Device1", "Pair"],
      ["unregister"],
    ]);
    fake.failure = true;
    await expect(service.disconnect("/a/d")).rejects.toThrow(
      "Operation failed",
    );
    expect(service.getSnapshot().error).toBe("Operation failed");
    await service.dispose();
  });
  test("BlueZ owner loss clears devices and owner return refreshes them", async () => {
    const fake = new Fake(),
      service = new BluetoothService(fake);
    await service.start();
    fake.signal({
      member: "NameOwnerChanged",
      body: ["org.bluez", ":1.1", ""],
    });
    expect(service.getSnapshot()).toMatchObject({
      available: false,
      devices: [],
    });
    fake.signal({
      member: "NameOwnerChanged",
      body: ["org.bluez", "", ":1.2"],
    });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(service.getSnapshot().available).toBe(true);
    await service.dispose();
  });
  test("adapter switch and disposal wait for a delayed discovery start", async () => {
    for (const operation of ["switch", "dispose"]) {
      const fake = new Fake();
      let release!: () => void;
      let entered!: () => void;
      const started = new Promise<void>((resolve) => {
        entered = resolve;
      });
      const gate = new Promise<void>((resolve) => {
        release = resolve;
      });
      const original = fake.call.bind(fake);
      fake.call = async (...args: any[]) => {
        await original(...args);
        if (args[2] === "StartDiscovery") {
          entered();
          await gate;
        }
      };
      const service = new BluetoothService(fake);
      await service.start();
      const scan = service.startDiscovery();
      await started;
      let closing: Promise<void> | undefined;
      if (operation === "switch") service.selectAdapter("/b");
      else closing = service.dispose();
      release();
      await scan;
      if (closing) await closing;
      else {
        await new Promise((resolve) => setTimeout(resolve, 0));
        await service.dispose();
      }
      expect(fake.calls.filter((c) => c[2] === "StopDiscovery")).toEqual([
        ["/a", "org.bluez.Adapter1", "StopDiscovery"],
      ]);
    }
  });
  test("cancel while agent registration is pending never starts pairing", async () => {
    const fake = new Fake();
    let release!: () => void;
    let entered!: () => void;
    const started = new Promise<void>((resolve) => {
      entered = resolve;
    });
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    fake.registerAgent = async (handler) => {
      fake.calls.push(["register"]);
      fake.handler = handler;
      entered();
      await gate;
    };
    const service = new BluetoothService(fake);
    await service.start();
    const pairing = service.pair("/a/d");
    const result = pairing.catch((e) => e);
    await started;
    await service.cancelPairing("/a/d");
    release();
    expect((await result).message).toBe("Pairing was canceled");
    expect(fake.calls.some((c) => c[2] === "Pair")).toBe(false);
    expect(fake.calls).toContainEqual(["unregister"]);
    await service.dispose();
  });
  test("bus failure clears stale data and ignores signals until retry refreshes", async () => {
    const fake = new Fake();
    let failed!: (e: unknown) => void;
    fake.start = async (signal, fail) => {
      fake.signal = signal;
      failed = fail;
    };
    const service = new BluetoothService(fake);
    await service.start();
    failed(new Error("lost bus"));
    fake.signal({ member: "InterfacesAdded", body: ["/a", fake.data["/a"]] });
    expect(service.getSnapshot()).toMatchObject({
      available: false,
      adapters: [],
      devices: [],
      error: "lost bus",
    });
    await service.start();
    expect(service.getSnapshot().available).toBe(true);
    await service.dispose();
  });
  test("external scan termination permits a later discovery start", async () => {
    const fake = new Fake(),
      service = new BluetoothService(fake);
    await service.start();
    await service.startDiscovery();
    fake.signal({
      path: "/a",
      member: "PropertiesChanged",
      body: ["org.bluez.Adapter1", { Discovering: v(false) }, []],
    });
    await service.startDiscovery();
    expect(fake.calls.filter((c) => c[2] === "StartDiscovery")).toHaveLength(2);
    await service.dispose();
  });
  test("scan termination during a delayed start does not retain ownership", async () => {
    const fake = new Fake();
    let release!: () => void;
    let entered!: () => void;
    const started = new Promise<void>((resolve) => {
      entered = resolve;
    });
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const original = fake.call.bind(fake);
    fake.call = async (...args: any[]) => {
      await original(...args);
      if (args[2] === "StartDiscovery") {
        entered();
        await gate;
      }
    };
    const service = new BluetoothService(fake);
    await service.start();
    const scan = service.startDiscovery();
    await started;
    fake.signal({
      path: "/a",
      member: "PropertiesChanged",
      body: ["org.bluez.Adapter1", { Powered: v(false) }, []],
    });
    release();
    await scan;
    await service.dispose();
    expect(fake.calls.some((c) => c[2] === "StopDiscovery")).toBe(false);
  });
  test("unavailable services reject device mutations", async () => {
    const fake = new Fake();
    fake.failure = true;
    const service = new BluetoothService(fake);
    await service.start();
    await expect(service.connect("/a/d")).rejects.toThrow("unavailable");
    expect(fake.calls).toEqual([]);
    await service.dispose();
  });
  test("selects a powered adapter when initially available", async () => {
    const fake = new Fake();
    fake.data["/a"]["org.bluez.Adapter1"].Powered = v(false);
    fake.data["/b"]["org.bluez.Adapter1"].Powered = v(true);
    const service = new BluetoothService(fake);
    await service.start();
    expect(service.getSnapshot().adapterId).toBe("/b");
    await service.dispose();
  });
  test("pairing requests explicit confirmation and rejects denial", async () => {
    const fake = new Fake(),
      service = new BluetoothService(fake);
    await service.start();
    await fake.registerAgent((...args: any[]) =>
      (service as any).handlePrompt(...args),
    );
    const answer = fake.handler("confirm", "/a/d", "123456");
    expect(service.getSnapshot().prompt).toMatchObject({
      kind: "confirm",
      passkey: "123456",
    });
    service.respondPrompt(false);
    await expect(answer).rejects.toThrow("rejected");
    const pin = fake.handler("pin", "/a/d");
    service.respondPrompt("1234");
    expect(await pin).toBe("1234");
    await service.dispose();
  });
});
