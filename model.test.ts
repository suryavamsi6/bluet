import { describe, expect, test } from "bun:test";
import { filterDevices, type Device } from "./model";
const devices: Device[] = [
  { id: "/a", name: "Headset", brand: "", kind: "headphones", description: "Headphones", status: "connected", address: "AA:BB", paired: true, trusted: true, adapterId: "/adapter" },
  { id: "/b", name: "Keyboard", brand: "", kind: "keyboard", description: "Keyboard", status: "saved", address: "CC:DD", paired: true, trusted: false, adapterId: "/adapter" },
];
describe("device filtering", () => {
  test("combines status and case-insensitive search", () => {
    expect(filterDevices(devices, "saved", " KEY ").map(d => d.id)).toEqual(["/b"]);
    expect(filterDevices(devices, "connected", "cc:dd")).toEqual([]);
    expect(filterDevices(devices, "all", "aa:bb").map(d => d.id)).toEqual(["/a"]);
  });
});
