import { describe, expect, test } from "bun:test";
import { AudioService, parseAudioDevices, type PactlRunner } from "./audio";

const address = "80:C3:BA:51:31:A6";
const cardName = "bluez_card.80_C3_BA_51_31_A6";
const sinkName = "bluez_output.80_C3_BA_51_31_A6.1";
const cards = [
  {
    name: cardName,
    properties: { "api.bluez5.address": address, "device.api": "bluez5" },
    active_profile: "a2dp-sink",
    profiles: {
      "a2dp-sink": {
        description: "High Fidelity (aptX HD)",
        sinks: 1,
        available: true,
      },
      "headset-head-unit": {
        description: "Headset",
        sinks: 1,
        available: true,
      },
      unavailable: { sinks: 1, available: false },
      off: { sinks: 0, available: true },
    },
  },
];
const sinks = [
  {
    name: sinkName,
    properties: {
      "api.bluez5.address": address.toLowerCase(),
      "api.bluez5.codec": "aptx_hd",
    },
    mute: false,
    volume: { left: { value: 32768 }, right: { value: 32768 } },
  },
];

describe("PipeWire/PulseAudio bridge", () => {
  test("maps only Bluetooth sinks/cards by address and exposes actual profiles/codec", () => {
    const devices = parseAudioDevices(
      [...cards, { name: "alsa_card.unrelated" }],
      sinks,
    );
    expect(Object.keys(devices)).toEqual([address]);
    expect(devices[address]).toMatchObject({
      volume: 50,
      muted: false,
      codec: "aptx_hd",
      profile: "a2dp-sink",
      sinkName,
      cardName,
    });
    expect(devices[address]!.profiles.map((p) => p.id)).toEqual([
      "a2dp-sink",
      "headset-head-unit",
    ]);
    expect(
      parseAudioDevices(
        [],
        [{ name: sinkName, volume: { mono: { value_percent: "42%" } } }],
      )[address]!.volume,
    ).toBe(42);
  });
  test("startup only reads; commands target the selected device and refresh actual state", async () => {
    const calls: string[][] = [];
    const fakeCards = structuredClone(cards),
      fakeSinks = structuredClone(sinks);
    const run: PactlRunner = async (args) => {
      calls.push(args);
      if (args[0] === "--format=json")
        return JSON.stringify(args[2] === "cards" ? fakeCards : fakeSinks);
      if (args[0] === "set-sink-volume")
        for (const v of Object.values(fakeSinks[0]!.volume))
          v.value = (Number.parseFloat(args[2]!) / 100) * 65536;
      if (args[0] === "set-sink-mute") fakeSinks[0]!.mute = args[2] === "1";
      if (args[0] === "set-card-profile")
        fakeCards[0]!.active_profile = args[2]!;
      return "";
    };
    const service = new AudioService(run, false);
    let updates = 0;
    const unsubscribe = service.subscribe(() => updates++);
    await service.start();
    expect(calls).toEqual([
      ["--format=json", "list", "cards"],
      ["--format=json", "list", "sinks"],
    ]);
    await service.setVolume(address, 74);
    await service.setMuted(address, true);
    await service.setProfile(address, "headset-head-unit");
    expect(calls.filter((c) => c[0]!.startsWith("set-"))).toEqual([
      ["set-sink-volume", sinkName, "74%"],
      ["set-sink-mute", sinkName, "1"],
      ["set-card-profile", cardName, "headset-head-unit"],
    ]);
    expect(service.getSnapshot().devices[address]).toMatchObject({
      volume: 74,
      muted: true,
      profile: "headset-head-unit",
    });
    expect(updates).toBe(4);
    await expect(service.setProfile(address, "unavailable")).rejects.toThrow(
      "not available",
    );
    await expect(service.setVolume("00:00:00:00:00:00", 30)).rejects.toThrow(
      "No audio device",
    );
    await expect(service.setVolume(address, NaN)).rejects.toThrow("number");
    unsubscribe();
    service.dispose();
  });
  test("reports service failure without manufacturing audio data, then recovers", async () => {
    let failing = true;
    const service = new AudioService(async (args) => {
      if (failing) throw new Error("Connection refused");
      return JSON.stringify(args[2] === "cards" ? cards : sinks);
    }, false);
    await service.start();
    expect(service.getSnapshot()).toMatchObject({
      available: false,
      loading: false,
      devices: {},
    });
    expect(service.getSnapshot().error).toContain("Connection refused");
    failing = false;
    await service.refresh();
    expect(service.getSnapshot().available).toBe(true);
    service.dispose();
  });
  test("write failures are surfaced and never optimistically change the device", async () => {
    const service = new AudioService(async (args) => {
      if (args[0] !== "--format=json") throw new Error("Permission denied");
      return JSON.stringify(args[2] === "cards" ? cards : sinks);
    }, false);
    await service.start();
    await expect(service.setVolume(address, 90)).rejects.toThrow(
      "Permission denied",
    );
    expect(service.getSnapshot().devices[address]!.volume).toBe(50);
    service.dispose();
  });
});
