import * as dbus from "dbus-next";
import type { Device } from "./model";

export type Adapter = {
  id: string;
  name: string;
  address: string;
  powered: boolean;
  discoverable: boolean;
  discovering: boolean;
  pairable: boolean;
};
export type PairingPrompt = {
  id: string;
  deviceId: string;
  kind: "confirm" | "pin" | "passkey" | "display" | "authorize";
  message: string;
  passkey?: string;
  entered?: number;
};
export type BluetoothSnapshot = {
  adapters: Adapter[];
  adapterId: string | null;
  devices: Device[];
  loading: boolean;
  error: string | null;
  prompt: PairingPrompt | null;
  available: boolean;
};
type Properties = Record<string, any>;
export type ManagedObjects = Record<string, Record<string, Properties>>;
type Signal = { path: string; interface: string; member: string; body: any[] };
export interface BluetoothTransport {
  start(
    signal: (signal: Signal) => void,
    failed: (error: unknown) => void,
  ): Promise<void>;
  objects(): Promise<ManagedObjects>;
  call(
    path: string,
    iface: string,
    method: string,
    ...args: any[]
  ): Promise<any>;
  set(
    path: string,
    iface: string,
    property: string,
    value: boolean,
  ): Promise<void>;
  registerAgent(handler: AgentHandler): Promise<void>;
  unregisterAgent(): Promise<void>;
  close(): Promise<void>;
}
type PairState = {
  id: string;
  canceled: boolean;
  started: boolean;
  done?: Promise<void>;
};
type AgentHandler = (
  kind: PairingPrompt["kind"],
  device: string,
  passkey?: string,
  entered?: number,
) => Promise<string | boolean | void>;
const BLUEZ = "org.bluez",
  ADAPTER = `${BLUEZ}.Adapter1`,
  DEVICE = `${BLUEZ}.Device1`,
  BATTERY = `${BLUEZ}.Battery1`;
const AGENT_PATH = "/org/bluet/agent";
const unwrap = (v: any): any =>
  v && typeof v === "object" && "signature" in v && "value" in v ? v.value : v;
const errorText = (e: unknown) =>
  e instanceof Error ? e.message || e.name : String(e);

class SystemTransport implements BluetoothTransport {
  private bus: dbus.MessageBus | null = null;
  private agent: dbus.interface.Interface | null = null;
  async start(signal: (s: Signal) => void, failed: (e: unknown) => void) {
    const bus = (this.bus = dbus.systemBus());
    bus.on("error", failed);
    bus.on("message", (msg) => {
      if (msg.type !== 4) return;
      if (
        msg.interface === "org.freedesktop.DBus" &&
        msg.member === "NameOwnerChanged" &&
        msg.body?.[0] === BLUEZ &&
        !msg.body[2] &&
        this.agent
      ) {
        bus.unexport(AGENT_PATH, this.agent);
        this.agent = null;
      }
      if (
        (msg.interface === "org.freedesktop.DBus.ObjectManager" &&
          (msg.member === "InterfacesAdded" ||
            msg.member === "InterfacesRemoved")) ||
        (msg.interface === "org.freedesktop.DBus.Properties" &&
          msg.member === "PropertiesChanged") ||
        (msg.interface === "org.freedesktop.DBus" &&
          msg.member === "NameOwnerChanged" &&
          msg.body?.[0] === BLUEZ)
      )
        signal({
          path: msg.path,
          interface: msg.interface,
          member: msg.member,
          body: msg.body,
        });
    });
    const daemon = (
      await bus.getProxyObject("org.freedesktop.DBus", "/org/freedesktop/DBus")
    ).getInterface("org.freedesktop.DBus");
    await daemon.AddMatch("type='signal',sender='org.bluez'");
    await daemon.AddMatch(
      "type='signal',interface='org.freedesktop.DBus',member='NameOwnerChanged',arg0='org.bluez'",
    );
  }
  async call(path: string, iface: string, method: string, ...args: any[]) {
    if (!this.bus) throw new Error("Bluetooth system bus is unavailable");
    const obj = await this.bus.getProxyObject(BLUEZ, path);
    return obj.getInterface(iface)[method](...args);
  }
  async objects(): Promise<ManagedObjects> {
    return this.call(
      "/",
      "org.freedesktop.DBus.ObjectManager",
      "GetManagedObjects",
    );
  }
  async set(path: string, iface: string, property: string, value: boolean) {
    await this.call(
      path,
      "org.freedesktop.DBus.Properties",
      "Set",
      iface,
      property,
      new dbus.Variant("b", value),
    );
  }
  async registerAgent(handler: AgentHandler) {
    if (!this.bus) throw new Error("Bluetooth system bus is unavailable");
    if (this.agent) return;
    const transport = this;
    class Agent extends dbus.interface.Interface {
      constructor() {
        super("org.bluez.Agent1");
      }
      async RequestPinCode(device: string) {
        return String(await handler("pin", device));
      }
      async RequestPasskey(device: string) {
        return Number(await handler("passkey", device));
      }
      async DisplayPinCode(device: string, pin: string) {
        await handler("display", device, pin);
      }
      async DisplayPasskey(device: string, key: number, entered: number) {
        await handler("display", device, String(key).padStart(6, "0"), entered);
      }
      async RequestConfirmation(device: string, key: number) {
        await handler("confirm", device, String(key).padStart(6, "0"));
      }
      async RequestAuthorization(device: string) {
        await handler("authorize", device);
      }
      async AuthorizeService(device: string, uuid: string) {
        await handler("authorize", device, uuid);
      }
      async Cancel() {
        await handler("display", "");
      }
      async Release() {
        await handler("display", "");
        if (transport.agent === this) {
          transport.bus?.unexport(AGENT_PATH, this);
          transport.agent = null;
        }
      }
    }
    Agent.configureMembers({
      methods: {
        RequestPinCode: { inSignature: "o", outSignature: "s" },
        RequestPasskey: { inSignature: "o", outSignature: "u" },
        DisplayPinCode: { inSignature: "os" },
        DisplayPasskey: { inSignature: "ouq" },
        RequestConfirmation: { inSignature: "ou" },
        RequestAuthorization: { inSignature: "o" },
        AuthorizeService: { inSignature: "os" },
        Cancel: {},
        Release: {},
      },
    });
    const agent = new Agent();
    this.bus.export(AGENT_PATH, agent);
    this.agent = agent;
    try {
      await this.call(
        "/org/bluez",
        "org.bluez.AgentManager1",
        "RegisterAgent",
        AGENT_PATH,
        "KeyboardDisplay",
      );
    } catch (e) {
      this.bus.unexport(AGENT_PATH, agent);
      this.agent = null;
      throw e;
    }
  }
  async unregisterAgent() {
    const agent = this.agent;
    if (!agent || !this.bus) return;
    try {
      await this.call(
        "/org/bluez",
        "org.bluez.AgentManager1",
        "UnregisterAgent",
        AGENT_PATH,
      );
    } finally {
      this.bus.unexport(AGENT_PATH, agent);
      this.agent = null;
    }
  }
  async close() {
    this.bus?.disconnect();
    this.bus = null;
  }
}

function kindFor(icon: string, klass: number): Device["kind"] {
  const i = icon.toLowerCase();
  if (i.includes("headset") || i.includes("headphone")) return "headphones";
  if (i.includes("mouse")) return "mouse";
  if (i.includes("keyboard")) return "keyboard";
  if (i.includes("phone")) return "phone";
  if (i.includes("speaker") || i.includes("audio-card")) return "speaker";
  if (i.includes("gaming") || i.includes("gamepad")) return "gamepad";
  const major = (klass >> 8) & 0x1f,
    minor = (klass >> 2) & 0x3f;
  if (major === 2) return "phone";
  if (major === 4) {
    if ([1, 2, 6].includes(minor)) return "headphones";
    if ([4, 5].includes(minor)) return "speaker";
  }
  if (major === 5) {
    if (minor & 0x20) return "mouse";
    if (minor & 0x10) return "keyboard";
    if ([1, 2].includes(minor & 0x0f)) return "gamepad";
  }
  return "device";
}
export function mapObjects(
  objects: ManagedObjects,
  connectedSince: Map<string, number> = new Map(),
): { adapters: Adapter[]; devices: Device[] } {
  const adapters: Adapter[] = [],
    devices: Device[] = [];
  for (const [id, interfaces] of Object.entries(objects)) {
    const a = interfaces[ADAPTER];
    if (a)
      adapters.push({
        id,
        name: String(
          unwrap(a.Alias) ||
            unwrap(a.Name) ||
            unwrap(a.Address) ||
            "Bluetooth adapter",
        ),
        address: String(unwrap(a.Address) || ""),
        powered: !!unwrap(a.Powered),
        discoverable: !!unwrap(a.Discoverable),
        discovering: !!unwrap(a.Discovering),
        pairable: !!unwrap(a.Pairable),
      });
    const d = interfaces[DEVICE];
    if (!d) continue;
    const kind = kindFor(
      String(unwrap(d.Icon) || ""),
      Number(unwrap(d.Class) || 0),
    );
    const connected = !!unwrap(d.Connected),
      paired = !!unwrap(d.Paired);
    const battery = unwrap(interfaces[BATTERY]?.Percentage),
      rssi = unwrap(d.RSSI);
    devices.push({
      id,
      name: String(
        unwrap(d.Alias) ||
          unwrap(d.Name) ||
          unwrap(d.Address) ||
          "Unknown device",
      ),
      brand: "",
      kind,
      description: {
        headphones: "Headphones",
        mouse: "Mouse",
        keyboard: "Keyboard",
        phone: "Phone",
        speaker: "Speaker",
        gamepad: "Game controller",
        device: "Bluetooth device",
      }[kind],
      status: connected ? "connected" : paired ? "saved" : "nearby",
      address: String(unwrap(d.Address) || ""),
      paired,
      trusted: !!unwrap(d.Trusted),
      adapterId: String(unwrap(d.Adapter) || ""),
      ...(typeof battery === "number" ? { battery } : {}),
      ...(typeof rssi === "number" ? { rssi } : {}),
      ...(connected && connectedSince.has(id)
        ? { connectedSince: connectedSince.get(id) }
        : {}),
    });
  }
  return { adapters, devices };
}

export class BluetoothService {
  private transport: BluetoothTransport;
  private snapshot: BluetoothSnapshot = {
    adapters: [],
    adapterId: null,
    devices: [],
    loading: true,
    error: null,
    prompt: null,
    available: false,
  };
  private listeners = new Set<(s: BluetoothSnapshot) => void>();
  private objects: ManagedObjects = {};
  private since = new Map<string, number>();
  private ownedDiscovery = new Set<string>();
  private pending: {
    resolve: (value: string | boolean) => void;
    reject: (error: Error) => void;
  } | null = null;
  private started = false;
  private transportStarted = false;
  private disposed = false;
  private promptSequence = 0;
  private closing = false;
  private discoveryQueue = new Map<string, Promise<void>>();
  private discoveryGeneration = new Map<string, number>();
  private pairState: PairState | null = null;
  constructor(transport: BluetoothTransport = new SystemTransport()) {
    this.transport = transport;
  }
  getSnapshot() {
    return this.snapshot;
  }
  subscribe(listener: (s: BluetoothSnapshot) => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private update(patch: Partial<BluetoothSnapshot>) {
    if (this.disposed) return;
    this.snapshot = { ...this.snapshot, ...patch };
    for (const listener of this.listeners) listener(this.snapshot);
  }
  private publish() {
    const { adapters, devices } = mapObjects(this.objects, this.since);
    const adapterId = adapters.some((a) => a.id === this.snapshot.adapterId)
      ? this.snapshot.adapterId
      : (adapters.find((a) => a.powered)?.id ?? adapters[0]?.id ?? null);
    this.update({
      adapters,
      adapterId,
      devices: devices.filter((d) => d.adapterId === adapterId),
      available: true,
      loading: false,
    });
  }
  async start() {
    if (this.started || this.closing || this.disposed) return;
    this.started = true;
    try {
      if (this.transportStarted) await this.transport.close();
      this.transportStarted = true;
      await this.transport.start(
        (s) => this.signal(s),
        (e) => this.fail(e),
      );
      await this.refresh();
    } catch (e) {
      this.fail(e);
      try {
        await this.transport.close();
      } catch {}
    }
  }
  private async refresh() {
    try {
      this.objects = await this.transport.objects();
      this.publish();
      this.update({ error: null });
    } catch (e) {
      this.fail(e);
    }
  }
  private fail(e: unknown) {
    this.started = false;
    this.objects = {};
    this.since.clear();
    this.ownedDiscovery.clear();
    if (this.pairState) this.pairState.canceled = true;
    this.cancelPrompt();
    this.update({
      adapters: [],
      adapterId: null,
      devices: [],
      available: false,
      loading: false,
      error: errorText(e),
    });
  }
  private signal(s: Signal) {
    if (this.disposed) return;
    if (s.member === "NameOwnerChanged") {
      if (!s.body[2]) {
        this.objects = {};
        this.since.clear();
        this.ownedDiscovery.clear();
        this.cancelPrompt();
        this.update({
          adapters: [],
          adapterId: null,
          devices: [],
          available: false,
          error: "Bluetooth service is unavailable",
        });
      } else void this.refresh();
      return;
    }
    if (!this.snapshot.available) return;
    if (s.member === "InterfacesAdded") {
      const [path, interfaces] = s.body;
      this.objects[path] = { ...this.objects[path], ...interfaces };
    } else if (s.member === "InterfacesRemoved") {
      const [path, interfaces] = s.body;
      for (const iface of interfaces) delete this.objects[path]?.[iface];
      if (!Object.keys(this.objects[path] || {}).length)
        delete this.objects[path];
      if (interfaces.includes(DEVICE)) this.since.delete(path);
    } else if (s.member === "PropertiesChanged") {
      const [iface, changed, invalidated] = s.body;
      const props = this.objects[s.path]?.[iface];
      if (!props) {
        void this.refresh();
        return;
      }
      if (
        iface === ADAPTER &&
        (("Powered" in changed && !unwrap(changed.Powered)) ||
          ("Discovering" in changed && !unwrap(changed.Discovering)))
      ) {
        this.ownedDiscovery.delete(s.path);
        this.discoveryGeneration.set(
          s.path,
          (this.discoveryGeneration.get(s.path) || 0) + 1,
        );
      }
      if (iface === DEVICE && "Connected" in changed) {
        const was = !!unwrap(props.Connected),
          now = !!unwrap(changed.Connected);
        if (!was && now) this.since.set(s.path, Date.now());
        else if (!now) this.since.delete(s.path);
      }
      Object.assign(props, changed);
      for (const key of invalidated) delete props[key];
      if (invalidated.length) void this.refresh();
    }
    this.publish();
  }
  selectAdapter(id: string) {
    if (!this.snapshot.adapters.some((a) => a.id === id) || this.closing)
      return;
    const old = this.snapshot.adapterId;
    if (old && old !== id)
      void this.queueDiscovery(old, false).catch((e) =>
        this.update({ error: errorText(e) }),
      );
    this.update({ adapterId: id });
    this.publish();
  }
  private adapter() {
    const id = this.snapshot.adapterId;
    if (!id) throw new Error("No Bluetooth adapter is available");
    return id;
  }
  private async action(fn: () => Promise<unknown>) {
    try {
      if (this.closing || this.disposed)
        throw new Error("Bluetooth service is closing");
      if (!this.snapshot.available)
        throw new Error("Bluetooth service is unavailable");
      await fn();
      this.update({ error: null });
    } catch (e) {
      this.update({ error: errorText(e) });
      throw e;
    }
  }
  async setPower(value: boolean) {
    await this.action(() =>
      this.transport.set(this.adapter(), ADAPTER, "Powered", value),
    );
  }
  async setDiscoverable(value: boolean) {
    await this.action(() =>
      this.transport.set(this.adapter(), ADAPTER, "Discoverable", value),
    );
  }
  private queueDiscovery(id: string, start: boolean): Promise<void> {
    const prior = this.discoveryQueue.get(id) || Promise.resolve();
    const next = prior
      .catch(() => {})
      .then(async () => {
        if (start) {
          if (this.closing || this.disposed || this.ownedDiscovery.has(id))
            return;
          const generation = this.discoveryGeneration.get(id) || 0;
          await this.transport.call(id, ADAPTER, "StartDiscovery");
          if ((this.discoveryGeneration.get(id) || 0) === generation)
            this.ownedDiscovery.add(id);
        } else if (this.ownedDiscovery.has(id)) {
          await this.transport.call(id, ADAPTER, "StopDiscovery");
          this.ownedDiscovery.delete(id);
        }
      });
    this.discoveryQueue.set(id, next);
    void next
      .finally(() => {
        if (this.discoveryQueue.get(id) === next)
          this.discoveryQueue.delete(id);
      })
      .catch(() => {});
    return next;
  }
  async startDiscovery() {
    await this.action(() => this.queueDiscovery(this.adapter(), true));
  }
  async stopDiscovery() {
    await this.action(() => this.queueDiscovery(this.adapter(), false));
  }
  async pair(id: string) {
    if (this.closing || this.disposed)
      throw new Error("Bluetooth service is closing");
    if (this.pairState)
      throw new Error("A pairing request is already in progress");
    const state: PairState = { id, canceled: false, started: false };
    this.pairState = state;
    const done = this.action(async () => {
      let registered = false,
        failure: unknown;
      try {
        await this.transport.registerAgent((...args) =>
          this.handlePrompt(...args),
        );
        registered = true;
        if (
          state.canceled ||
          this.closing ||
          this.disposed ||
          !this.snapshot.available
        )
          throw new Error("Pairing was canceled");
        state.started = true;
        await this.transport.call(id, DEVICE, "Pair");
      } catch (e) {
        failure = e;
      } finally {
        this.cancelPrompt();
        if (registered)
          try {
            await this.transport.unregisterAgent();
          } catch (e) {
            if (!failure) failure = e;
          }
      }
      if (failure) throw failure;
    });
    state.done = done;
    try {
      await done;
    } finally {
      if (this.pairState === state) this.pairState = null;
    }
  }
  async cancelPairing(id: string) {
    const state = this.pairState;
    if (state?.id === id) state.canceled = true;
    this.cancelPrompt();
    if (!state || state.id !== id || state.started)
      await this.action(() => this.transport.call(id, DEVICE, "CancelPairing"));
  }
  async connect(id: string) {
    await this.action(() => this.transport.call(id, DEVICE, "Connect"));
  }
  async disconnect(id: string) {
    await this.action(() => this.transport.call(id, DEVICE, "Disconnect"));
  }
  async remove(id: string) {
    const device = this.objects[id]?.[DEVICE];
    const adapter = unwrap(device?.Adapter);
    if (!adapter) throw new Error("Bluetooth device is unavailable");
    await this.action(() =>
      this.transport.call(adapter, ADAPTER, "RemoveDevice", id),
    );
  }
  async setTrusted(id: string, value: boolean) {
    await this.action(() => this.transport.set(id, DEVICE, "Trusted", value));
  }
  private cancelPrompt() {
    const pending = this.pending;
    this.pending = null;
    pending?.reject(
      new dbus.DBusError("org.bluez.Error.Canceled", "Pairing was canceled"),
    );
    this.update({ prompt: null });
  }
  private async handlePrompt(
    kind: PairingPrompt["kind"],
    deviceId: string,
    passkey?: string,
    entered?: number,
  ): Promise<string | boolean | void> {
    if (!deviceId) {
      this.cancelPrompt();
      return;
    }
    this.cancelPrompt();
    const name =
      this.snapshot.devices.find((d) => d.id === deviceId)?.name ||
      "the device";
    const message =
      kind === "confirm"
        ? `Confirm that ${name} shows the same passkey.`
        : kind === "pin"
          ? `Enter the PIN for ${name}.`
          : kind === "passkey"
            ? `Enter the passkey for ${name}.`
            : kind === "authorize"
              ? `Allow ${name} to connect${passkey ? ` to ${passkey}` : ""}?`
              : `Enter this code on ${name}.`;
    this.update({
      prompt: {
        id: String(++this.promptSequence),
        deviceId,
        kind,
        message,
        ...(passkey !== undefined ? { passkey } : {}),
        ...(entered !== undefined ? { entered } : {}),
      },
    });
    if (kind === "display") return;
    return new Promise((resolve, reject) => {
      this.pending = { resolve, reject };
    });
  }
  respondPrompt(value: string | boolean) {
    const prompt = this.snapshot.prompt,
      pending = this.pending;
    if (!prompt) return;
    if (!pending) {
      this.update({ prompt: null });
      return;
    }
    if (value === false) {
      this.pending = null;
      this.update({ prompt: null });
      pending.reject(
        new dbus.DBusError(
          "org.bluez.Error.Rejected",
          "Pairing request was rejected",
        ),
      );
      return;
    }
    if (
      prompt.kind === "pin" &&
      (typeof value !== "string" || value.length < 1 || value.length > 16)
    ) {
      this.update({ error: "Enter a PIN of 1 to 16 characters" });
      return;
    }
    if (
      prompt.kind === "passkey" &&
      (typeof value !== "string" || !/^\d{1,6}$/.test(value))
    ) {
      this.update({ error: "Enter a numeric passkey of up to 6 digits" });
      return;
    }
    if (
      (prompt.kind === "confirm" || prompt.kind === "authorize") &&
      value !== true
    )
      return;
    this.pending = null;
    this.update({ prompt: null, error: null });
    pending.resolve(value);
  }
  async dispose() {
    if (this.disposed || this.closing) return;
    this.closing = true;
    const state = this.pairState;
    if (state) {
      state.canceled = true;
      this.cancelPrompt();
      if (state.started)
        try {
          await this.transport.call(state.id, DEVICE, "CancelPairing");
        } catch {}
    }
    const ids = new Set([
      ...this.ownedDiscovery,
      ...this.discoveryQueue.keys(),
    ]);
    await Promise.all(
      [...ids].map((id) => this.queueDiscovery(id, false).catch(() => {})),
    );
    if (state?.done)
      try {
        await state.done;
      } catch {}
    try {
      await this.transport.unregisterAgent();
    } catch {}
    await this.transport.close();
    this.disposed = true;
    this.listeners.clear();
  }
}
