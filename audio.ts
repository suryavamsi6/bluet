import { execFile, spawn, type ChildProcess } from "node:child_process";
import { promisify } from "node:util";

export type AudioDevice = {
  address: string;
  volume: number;
  muted: boolean;
  profiles: { id: string; label: string }[];
  profile: string | null;
  sinkName?: string;
  cardName?: string;
  codec?: string;
};
export type AudioSnapshot = {
  available: boolean;
  loading: boolean;
  error: string | null;
  devices: Record<string, AudioDevice>;
};
export type PactlRunner = (args: string[]) => Promise<string>;
const execute = promisify(execFile);
const runPactl: PactlRunner = async (args) => {
  const { stdout } = await execute("pactl", args, {
    timeout: 10000,
    maxBuffer: 4 * 1024 * 1024,
    env: { ...process.env, LC_ALL: "C" },
  });
  return stdout;
};
type Properties = Record<string, unknown>;
type Card = {
  name?: string;
  properties?: Properties;
  profiles?: Record<
    string,
    { description?: string; available?: boolean | string; sinks?: number }
  >;
  active_profile?: string | { name?: string };
};
type Sink = {
  name?: string;
  properties?: Properties;
  mute?: boolean;
  volume?: Record<string, { value?: number; value_percent?: string }>;
};

function addressOf(entry: {
  name?: string;
  properties?: Properties;
}): string | undefined {
  const p = entry.properties ?? {};
  const candidate = p["api.bluez5.address"] ?? p["device.string"];
  if (
    typeof candidate === "string" &&
    /^([a-f\d]{2}:){5}[a-f\d]{2}$/i.test(candidate)
  )
    return candidate.toUpperCase();
  const match = entry.name?.match(
    /^bluez_(?:card|output)\.(([a-f\d]{2}_){5}[a-f\d]{2})(?:\.|$)/i,
  );
  return match?.[1]?.replaceAll("_", ":").toUpperCase();
}
function isBluetooth(entry: {
  name?: string;
  properties?: Properties;
}): boolean {
  return (
    entry.properties?.["device.api"] === "bluez5" ||
    /^bluez_(card|output)\./.test(entry.name ?? "")
  );
}
export function parseAudioDevices(
  cards: unknown,
  sinks: unknown,
): Record<string, AudioDevice> {
  if (!Array.isArray(cards) || !Array.isArray(sinks))
    throw new Error("Unexpected pactl response");
  const result: Record<string, AudioDevice> = {};
  const empty = (address: string): AudioDevice => ({
    address,
    volume: 0,
    muted: false,
    profiles: [],
    profile: null,
  });
  for (const card of cards as Card[]) {
    if (!isBluetooth(card)) continue;
    const address = addressOf(card);
    if (!address) continue;
    result[address] = {
      ...empty(address),
      cardName: card.name,
      profile:
        typeof card.active_profile === "string"
          ? card.active_profile
          : (card.active_profile?.name ?? null),
      profiles: Object.entries(card.profiles ?? {})
        .filter(
          ([, p]) =>
            p.available !== false && p.available !== "no" && (p.sinks ?? 0) > 0,
        )
        .map(([id, p]) => ({ id, label: p.description ?? id })),
    };
  }
  for (const sink of sinks as Sink[]) {
    if (!isBluetooth(sink)) continue;
    const address = addressOf(sink);
    if (!address) continue;
    const levels = Object.values(sink.volume ?? {})
      .map((v) =>
        typeof v.value === "number"
          ? (v.value / 65536) * 100
          : Number.parseFloat(v.value_percent ?? ""),
      )
      .filter(Number.isFinite);
    const codec = sink.properties?.["api.bluez5.codec"];
    result[address] = {
      ...(result[address] ?? empty(address)),
      sinkName: sink.name,
      muted: sink.mute ?? false,
      volume: levels.length
        ? Math.round(levels.reduce((sum, n) => sum + n, 0) / levels.length)
        : 0,
      codec: typeof codec === "string" ? codec : undefined,
    };
  }
  return result;
}

export class AudioService {
  private snapshot: AudioSnapshot = {
    available: false,
    loading: true,
    error: null,
    devices: {},
  };
  private listeners = new Set<() => void>();
  private monitor?: ChildProcess;
  private timer?: ReturnType<typeof setTimeout>;
  private disposed = false;
  private refreshing?: Promise<void>;
  private refreshAgain = false;
  constructor(
    private run: PactlRunner = runPactl,
    private monitorEnabled = true,
  ) {}
  getSnapshot = (): AudioSnapshot => this.snapshot;
  subscribe = (fn: () => void): (() => void) => {
    this.listeners.add(fn);
    return () => {
      this.listeners.delete(fn);
    };
  };
  private publish(snapshot: AudioSnapshot) {
    if (!this.disposed) {
      this.snapshot = snapshot;
      for (const listener of this.listeners) listener();
    }
  }
  async start(): Promise<void> {
    await this.refresh();
    if (this.monitorEnabled && !this.disposed && !this.monitor) this.watch();
  }
  private watch() {
    const monitor = spawn("pactl", ["subscribe"], {
      stdio: ["ignore", "pipe", "ignore"],
      env: { ...process.env, LC_ALL: "C" },
    });
    this.monitor = monitor;
    monitor.stdout?.on("data", (chunk) => {
      if (/sink|card|server/.test(chunk.toString())) this.scheduleRefresh();
    });
    monitor.on("error", () => {
      /* The initial query reports unavailable pactl. */
    });
    monitor.on("close", () => {
      if (this.monitor === monitor) this.monitor = undefined;
      if (this.disposed) return;
      clearTimeout(this.timer);
      this.timer = setTimeout(() => {
        void this.refresh();
        this.watch();
      }, 2500);
    });
  }
  private scheduleRefresh() {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      void this.refresh();
    }, 120);
  }
  async refresh(): Promise<void> {
    if (this.disposed) return;
    if (this.refreshing) {
      this.refreshAgain = true;
      return this.refreshing;
    }
    this.refreshing = (async () => {
      do {
        this.refreshAgain = false;
        try {
          const [cards, sinks] = await Promise.all([
            this.run(["--format=json", "list", "cards"]),
            this.run(["--format=json", "list", "sinks"]),
          ]);
          this.publish({
            available: true,
            loading: false,
            error: null,
            devices: parseAudioDevices(JSON.parse(cards), JSON.parse(sinks)),
          });
        } catch (error) {
          this.publish({
            available: false,
            loading: false,
            error: `Audio service unavailable: ${error instanceof Error ? error.message : String(error)}`,
            devices: {},
          });
        }
      } while (this.refreshAgain && !this.disposed);
    })();
    try {
      await this.refreshing;
    } finally {
      this.refreshing = undefined;
    }
  }
  private device(address: string): AudioDevice {
    const device = this.snapshot.devices[address.toUpperCase()];
    if (this.disposed || !this.snapshot.available || !device)
      throw new Error("No audio device is available for these headphones.");
    return device;
  }
  async setVolume(address: string, value: number): Promise<void> {
    if (!Number.isFinite(value)) throw new Error("Volume must be a number.");
    const device = this.device(address);
    if (!device.sinkName)
      throw new Error("No audio output is available for this device.");
    await this.run([
      "set-sink-volume",
      device.sinkName,
      `${Math.round(Math.max(0, Math.min(100, value)))}%`,
    ]);
    await this.refresh();
  }
  async setMuted(address: string, muted: boolean): Promise<void> {
    const device = this.device(address);
    if (!device.sinkName)
      throw new Error("No audio output is available for this device.");
    await this.run(["set-sink-mute", device.sinkName, muted ? "1" : "0"]);
    await this.refresh();
  }
  async setProfile(address: string, profile: string): Promise<void> {
    const device = this.device(address);
    if (!device.cardName || !device.profiles.some((p) => p.id === profile))
      throw new Error("This audio profile is not available.");
    await this.run(["set-card-profile", device.cardName, profile]);
    await this.refresh();
  }
  dispose(): void {
    this.disposed = true;
    clearTimeout(this.timer);
    this.monitor?.kill();
    this.monitor = undefined;
    this.listeners.clear();
  }
}
