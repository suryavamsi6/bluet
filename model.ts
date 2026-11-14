export type Device = {
  id: string;
  name: string;
  brand: string;
  kind: "headphones" | "mouse" | "keyboard" | "phone" | "speaker" | "gamepad" | "device";
  description: string;
  status: "connected" | "saved" | "nearby";
  battery?: number;
  address: string;
  paired: boolean;
  trusted: boolean;
  rssi?: number;
  adapterId: string;
  connectedSince?: number;
};
export type DeviceFilter = "all" | "connected" | "saved";

export function filterDevices(devices: Device[], filter: DeviceFilter, query: string): Device[] {
  const term = query.trim().toLowerCase();
  return devices.filter(device =>
    (filter === "all" || device.status === filter) &&
    (!term || [device.name, device.brand, device.kind, device.description, device.address].some(value => value.toLowerCase().includes(term)))
  );
}
