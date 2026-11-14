import React, { useEffect, useRef, useState } from "react";
import {
  render,
  createRenderer,
  startFrameLoop,
  useGpuix,
  useWindowSize,
  type PublicInstance,
  motion,
  type StyleDesc,
} from "@gpuix/react";
import {
  Select,
  SelectTrigger,
  SelectValue,
  SelectContent,
  SelectItem,
} from "@gpuix/react/select";
import { readFileSync } from "node:fs";
import interFont from "./assets/fonts/Inter.ttf" with { type: "file" };
import monoFont from "./assets/fonts/GeistMono.ttf" with { type: "file" };
import { icons, type IconName } from "./icons";
import { filterDevices, type Device, type DeviceFilter } from "./model";
import { BluetoothService, type BluetoothSnapshot } from "./bluez";
import { AudioService, type AudioSnapshot } from "./audio";
import { DitherLayer } from "./dither-layer";

const C = {
  bg: "#111317",
  sidebar: "#16181D",
  panel: "#181B21",
  raised: "#22262E",
  border: "#2B3039",
  fg: "#F0F2F6",
  muted: "#969EAE",
  subtle: "#7D8798",
  accent: "#5B92F5",
  primary: "#3370DA",
  selection: "#1C2C45",
  green: "#75D9A3",
  red: "#F29399",
  greenBg: "#193328",
};
const row: StyleDesc = {
  display: "flex",
  flexDirection: "row",
  alignItems: "center",
};
const column: StyleDesc = { display: "flex", flexDirection: "column" };
const line: StyleDesc = {
  height: 1,
  width: "100%",
  backgroundColor: C.border,
  flexShrink: 0,
};
const fill: StyleDesc = { flexGrow: 1, minHeight: 0, minWidth: 0 };
const gradient = (base: string, tint: string): StyleDesc => ({
  background: {
    type: "linear-gradient",
    angle: 120,
    stops: [
      { color: base, position: 0 },
      { color: tint, position: 1 },
    ],
  },
});
function T({
  children,
  size = 13,
  color = C.fg,
  weight = 400,
  style = {},
}: {
  children: React.ReactNode;
  size?: number;
  color?: string;
  weight?: number;
  style?: StyleDesc;
}) {
  return (
    <text
      style={{
        fontFamily: "Inter",
        fontSize: size,
        lineHeight: size * 1.3,
        fontWeight: weight,
        color,
        ...style,
      }}
    >
      {Array.isArray(children) ? children.flat(Infinity).join("") : children}
    </text>
  );
}
function Mono({
  children,
  size = 10,
  color = C.muted,
}: {
  children: React.ReactNode;
  size?: number;
  color?: string;
}) {
  return (
    <text
      style={{
        fontFamily: "Geist Mono",
        fontSize: size,
        color,
        lineHeight: size * 1.3,
      }}
    >
      {Array.isArray(children) ? children.flat(Infinity).join("") : children}
    </text>
  );
}
function Icon({
  name,
  size = 18,
  color = C.muted,
}: {
  name: IconName;
  size?: number;
  color?: string;
}) {
  return (
    <svg
      source={icons[name]}
      style={{ width: size, height: size, color, flexShrink: 0 }}
    />
  );
}
function Button({
  children,
  onClick,
  testId,
  primary = false,
  small = false,
  style = {},
  label,
  disabled = false,
  elementRef,
}: {
  children: React.ReactNode;
  onClick: () => void;
  testId?: string;
  primary?: boolean;
  small?: boolean;
  style?: StyleDesc;
  label?: string;
  disabled?: boolean;
  elementRef?: React.Ref<PublicInstance>;
}) {
  const [hovered, setHovered] = useState(false);
  return (
    <div
      ref={elementRef}
      role="button"
      aria-label={label}
      testId={testId}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      tabIndex={disabled ? undefined : 0}
      onClick={() => !disabled && onClick()}
      onKeyDown={(e) => {
        if (!disabled && (e.key === "enter" || e.key === "space")) {
          e.preventDefault();
          e.stopPropagation();
          onClick();
        }
      }}
      style={{
        ...row,
        position: "relative",
        justifyContent: "center",
        gap: 8,
        paddingLeft: small ? 12 : 16,
        paddingRight: small ? 12 : 16,
        height: small ? 28 : 37,
        flexShrink: 0,
        borderRadius: 6,
        backgroundColor: primary ? C.primary : C.raised,
        borderWidth: primary ? 0 : 1,
        borderColor: C.border,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.4 : 1,
        hover: { backgroundColor: primary ? "#3C7CE9" : "#2B303A" },
        focusVisible: { outlineWidth: 2, outlineColor: C.accent },
        ...style,
      }}
    >
      {primary && (
        <DitherLayer
          variant="button"
          strength={hovered && !disabled ? 1 : 0.7}
          radius={6}
        />
      )}
      {typeof children === "string" ? (
        <T size={small ? 11 : 13} weight={primary ? 600 : 500}>
          {children}
        </T>
      ) : (
        children
      )}
    </div>
  );
}
type MenuAnchor = { x: number; y: number; width: number; height: number };
function IconButton({
  name,
  onClick,
  label,
  testId,
}: {
  name: IconName;
  onClick: (anchor: MenuAnchor) => void;
  label: string;
  testId?: string;
}) {
  const { renderer } = useGpuix();
  const element = useRef<PublicInstance>(null);
  return (
    <Button
      label={label}
      elementRef={element}
      onClick={() =>
        onClick(
          element.current
            ? (renderer?.getElementBounds?.(element.current.id) ?? {
                x: 0,
                y: 0,
                width: 28,
                height: 28,
              })
            : { x: 0, y: 0, width: 28, height: 28 },
        )
      }
      testId={testId}
      style={{
        width: 28,
        height: 28,
        padding: 0,
        borderWidth: 0,
        backgroundColor: "transparent",
      }}
    >
      <Icon name={name} size={16} />
    </Button>
  );
}
function Toggle({
  value,
  onChange,
  testId,
  label,
  disabled = false,
}: {
  value: boolean;
  onChange: () => void;
  testId: string;
  label: string;
  disabled?: boolean;
}) {
  return (
    <div
      testId={testId}
      role="switch"
      aria-label={label}
      aria-valuetext={value ? "On" : "Off"}
      tabIndex={disabled ? undefined : 0}
      onClick={() => !disabled && onChange()}
      onKeyDown={(e) => {
        if (!disabled && (e.key === "space" || e.key === "enter")) {
          e.preventDefault();
          onChange();
        }
      }}
      style={{
        ...row,
        width: 34,
        height: 20,
        padding: 3,
        borderRadius: 12,
        backgroundColor: value ? C.primary : "#3C424D",
        justifyContent: value ? "flex-end" : "flex-start",
        flexShrink: 0,
        cursor: disabled ? "not-allowed" : "pointer",
        opacity: disabled ? 0.4 : 1,
        focusVisible: { outlineWidth: 2, outlineColor: C.accent },
      }}
    >
      <div
        style={{
          width: 14,
          height: 14,
          borderRadius: 7,
          backgroundColor: "#FFFFFF",
          pointerEvents: "none",
        }}
      />
    </div>
  );
}
const kindIcon: Record<Device["kind"], IconName> = {
  headphones: "headphones",
  mouse: "mouse",
  keyboard: "keyboard",
  phone: "smartphone",
  speaker: "speaker",
  gamepad: "gamepad-2",
  device: "bluetooth",
};
const kindTitle: Record<Device["kind"], string> = {
  headphones: "Headphones",
  mouse: "Mouse",
  keyboard: "Keyboard",
  phone: "Phone",
  speaker: "Speaker",
  gamepad: "Game controller",
  device: "Bluetooth device",
};
function Search({
  value,
  onChange,
  testId,
  width = "100%",
}: {
  value: string;
  onChange: (s: string) => void;
  testId: string;
  width?: number | string;
}) {
  return (
    <div
      style={{
        ...row,
        width,
        height: 37,
        paddingLeft: 12,
        paddingRight: 12,
        gap: 10,
        backgroundColor: C.panel,
        borderWidth: 1,
        borderColor: C.border,
        borderRadius: 6,
      }}
    >
      <Icon name="search" size={16} />
      <input
        testId={testId}
        aria-label="Search devices"
        value={value}
        placeholder="Search devices…"
        onChange={(e) => onChange(e.value ?? "")}
        style={{
          ...fill,
          height: 30,
          fontFamily: "Inter",
          fontSize: 13,
          color: C.fg,
          backgroundColor: "transparent",
        }}
      />
    </div>
  );
}
function Radar({ label = "Scanning nearby…" }: { label?: string }) {
  return (
    <div style={{ ...row, gap: 6 }}>
      <motion.div
        animate={{ opacity: 0.4 }}
        transition={{ duration: 1.8, ease: "easeInOut" }}
      >
        <Icon name="radar" size={13} color={C.accent} />
      </motion.div>
      <T size={10} color={C.accent}>
        {label}
      </T>
    </div>
  );
}

type BluetRuntime = typeof globalThis & {
  __bluetRenderer?: ReturnType<typeof createRenderer>;
  __bluetCleanup?: () => Promise<void>;
  __bluetSignals?: boolean;
  __bluetShuttingDown?: boolean;
};
const runtime = globalThis as BluetRuntime;
async function shutdown() {
  if (runtime.__bluetShuttingDown) return;
  runtime.__bluetShuttingDown = true;
  try {
    await Promise.race([
      runtime.__bluetCleanup?.(),
      new Promise((resolve) => setTimeout(resolve, 2500)),
    ]);
  } finally {
    process.exit(0);
  }
}

export function BluetApp() {
  const { renderer } = useGpuix();
  const windowSize = useWindowSize();
  const [menuAnchor, setMenuAnchor] = useState<MenuAnchor>({
    x: 0,
    y: 0,
    width: 28,
    height: 28,
  });
  const [bluetooth] = useState(() => new BluetoothService());
  const [audio] = useState(() => new AudioService());
  const [bt, setBt] = useState<BluetoothSnapshot>(() =>
    bluetooth.getSnapshot(),
  );
  const [sound, setSound] = useState<AudioSnapshot>(() => audio.getSnapshot());
  const [selected, setSelected] = useState<string | null>(null),
    [filter, setFilter] = useState<DeviceFilter>("all"),
    [query, setQuery] = useState(""),
    [page, setPage] = useState("Devices");
  const [dialog, setDialog] = useState<"pair" | "forget" | null>(null),
    [pairId, setPairId] = useState<string | null>(null),
    [pairQuery, setPairQuery] = useState(""),
    [promptInput, setPromptInput] = useState("");
  const [menu, setMenu] = useState(false),
    [pending, setPending] = useState<string | null>(null),
    [error, setError] = useState<string | null>(null),
    [toast, setToast] = useState(""),
    [activity, setActivity] = useState<{ message: string; time: number }[]>([]);
  const previous = useRef<BluetoothSnapshot | null>(null);
  const pendingRef = useRef<string | null>(null);
  const log = (message: string) =>
    setActivity((a) => [{ message, time: Date.now() }, ...a].slice(0, 100));
  useEffect(() => {
    let alive = true;
    const unsubscribe = bluetooth.subscribe((next) => {
      if (!alive) return;
      const old = previous.current;
      if (old && !next.loading) {
        for (const d of next.devices) {
          const before = old.devices.find((x) => x.id === d.id);
          if (before && before.status !== d.status)
            log(
              `${d.name} ${d.status === "connected" ? "connected" : d.status === "saved" ? "disconnected" : "discovered"}`,
            );
        }
        if (old.available !== next.available)
          log(
            next.available
              ? "Bluetooth service available"
              : "Bluetooth service unavailable",
          );
      }
      previous.current = next;
      setBt(next);
    });
    const unAudio = audio.subscribe(() => {
      if (alive) setSound(audio.getSnapshot());
    });
    void bluetooth
      .start()
      .catch((e) => alive && setError(String(e.message ?? e)));
    void audio.start().catch(() => {});
    const cleanup = async () => {
      alive = false;
      unsubscribe();
      unAudio();
      audio.dispose();
      await bluetooth.dispose();
    };
    runtime.__bluetCleanup = cleanup;
    return () => {
      if (runtime.__bluetCleanup === cleanup)
        runtime.__bluetCleanup = undefined;
      void cleanup();
    };
  }, [bluetooth, audio]);
  useEffect(() => {
    if (!bt.devices.some((d) => d.id === selected))
      setSelected(
        bt.devices.find((d) => d.status === "connected")?.id ??
          bt.devices[0]?.id ??
          null,
      );
  }, [bt.devices, selected]);
  useEffect(() => setPromptInput(""), [bt.prompt?.id]);
  const devices = bt.devices,
    adapter = bt.adapters.find((a) => a.id === bt.adapterId),
    power = adapter?.powered ?? false,
    visible = adapter?.discoverable ?? false;
  const device = devices.find((d) => d.id === selected),
    connected = device?.status === "connected";
  const audioDevice = device
    ? sound.devices[device.address.toUpperCase()]
    : undefined;
  const busy = !!pending,
    canControl = !!adapter && bt.available && !bt.loading;
  const notify = (message: string) => {
    setToast(message);
    log(message);
    setTimeout(() => setToast(""), 4500);
  };
  const run = async (
    label: string,
    operation: () => Promise<void>,
    success?: string,
  ) => {
    if (pendingRef.current) return;
    pendingRef.current = label;
    setPending(label);
    setError(null);
    try {
      await operation();
      if (success) notify(success);
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      setError(message);
      log(`${label}: ${message}`);
    } finally {
      pendingRef.current = null;
      setPending(null);
    }
  };
  const changePower = () =>
    void run("Changing Bluetooth power", () => bluetooth.setPower(!power));
  const connect = (d: Device) =>
    void run(`Connecting ${d.name}`, () => bluetooth.connect(d.id));
  const disconnect = (d: Device) =>
    void run(`Disconnecting ${d.name}`, () => bluetooth.disconnect(d.id));
  const changeVisible = () =>
    void run("Changing discoverability", () =>
      bluetooth.setDiscoverable(!visible),
    );
  const openPair = (id?: string) => {
    if (!power || !canControl) return;
    setPairId(id ?? devices.find((d) => d.status === "nearby")?.id ?? null);
    setPairQuery("");
    setDialog("pair");
    void run("Discovering nearby devices", () => bluetooth.startDiscovery());
  };
  const closeDialog = () => {
    setDialog(null);
    if (dialog === "pair") {
      if (pairId && pending?.startsWith("Pairing"))
        void bluetooth.cancelPairing(pairId).catch((e) => setError(e.message));
      void bluetooth.stopDiscovery().catch((e) => setError(e.message));
    }
  };
  const pair = () => {
    const d = devices.find((d) => d.id === pairId);
    if (!d) return;
    void run(
      `Pairing ${d.name}`,
      async () => {
        await bluetooth.pair(d.id);
        setSelected(d.id);
        setDialog(null);
        await bluetooth.stopDiscovery();
      },
      `${d.name} paired`,
    );
  };
  const nav = (p: string) => {
    setPage(p);
    setMenu(false);
  };
  const visibleDevices = filterDevices(devices, filter, query);
  const [draftVolume, setDraftVolume] = useState<{
    address: string;
    value: number;
  } | null>(null);
  const volumeDraftRef = useRef<{ address: string; value: number } | null>(
    null,
  );
  const volume =
    draftVolume && draftVolume.address === device?.address
      ? draftVolume!.value
      : (audioDevice?.volume ?? 0);
  const setVolume = (value: number) => {
    if (device && audioDevice && !busy)
      void run("Changing volume", () =>
        audio.setVolume(device.address, Math.max(0, Math.min(100, value))),
      );
  };
  const [volumeBounds, setVolumeBounds] = useState({ x: 0, width: 266 }),
    [dragging, setDragging] = useState(false);
  const changeVolume = (event: { x?: number }) => {
    if (!device || !audioDevice?.sinkName || busy) return;
    const value = Math.max(
      0,
      Math.min(
        100,
        Math.round(
          (((event.x ?? volumeBounds.x) - volumeBounds.x) /
            volumeBounds.width) *
            100,
        ),
      ),
    );
    volumeDraftRef.current = { address: device.address, value };
    setDraftVolume(volumeDraftRef.current);
  };
  const commitVolume = () => {
    const draft = volumeDraftRef.current;
    volumeDraftRef.current = null;
    setDragging(false);
    if (!draft) return;
    void run("Changing volume", () =>
      audio.setVolume(draft.address, draft.value),
    ).finally(() => setDraftVolume(null));
  };
  const rejectPrompt = () => {
    const prompt = bt.prompt;
    if (!prompt) return;
    bluetooth.respondPrompt(false);
    if (prompt.kind === "display") {
      void bluetooth
        .cancelPairing(prompt.deviceId)
        .catch((e) => setError(e.message));
    }
  };
  const failure = error ?? bt.error;
  return (
    <div
      testId="bluet-app"
      onMouseUp={commitVolume}
      onKeyDown={(e) => {
        if (e.key === "escape") {
          if (bt.prompt) rejectPrompt();
          else closeDialog();
          setMenu(false);
        }
      }}
      style={{
        ...column,
        width: "100%",
        height: "100%",
        backgroundColor: C.bg,
        overflow: "hidden",
        userSelect: "none",
      }}
    >
      <div
        style={{
          ...row,
          height: 44,
          paddingLeft: 16,
          paddingRight: 16,
          justifyContent: "space-between",
          backgroundColor: C.sidebar,
          borderBottomWidth: 1,
          borderColor: C.border,
          flexShrink: 0,
        }}
      >
        <div style={{ ...row, gap: 8 }}>
          <Icon name="bluetooth" size={15} color={C.accent} />
          <T size={12} color={C.muted} weight={600}>
            Bluet
          </T>
        </div>
        <div style={{ ...row, gap: 8 }}>
          <IconButton
            name="minus"
            label="Minimize window"
            onClick={() => renderer?.minimizeWindow?.()}
          />
          <IconButton
            name="square"
            label="Maximize window"
            onClick={() => renderer?.zoomWindow?.()}
          />
          <IconButton
            name="x"
            label="Close Bluet"
            onClick={() => void shutdown()}
          />
        </div>
      </div>
      <div style={{ ...row, alignItems: "stretch", ...fill }}>
        <div
          style={{
            ...column,
            width: 220,
            position: "relative",
            flexShrink: 0,
            paddingTop: 28,
            paddingLeft: 18,
            paddingRight: 18,
            paddingBottom: 20,
            gap: 28,
            backgroundColor: C.sidebar,
            borderRightWidth: 1,
            borderColor: C.border,
          }}
        >
          <DitherLayer variant="ambient" strength={0.7} opacity={0.75} />
          <div style={{ ...row, gap: 12 }}>
            <div
              style={{
                ...row,
                width: 36,
                height: 40,
                justifyContent: "center",
                borderRadius: 9,
                backgroundColor: C.primary,
              }}
            >
              <Icon name="bluetooth" size={24} color="#FFFFFF" />
            </div>
            <div style={{ ...column, gap: 2 }}>
              <T size={21} weight={600}>
                Bluet
              </T>
              <T size={10} color={C.muted}>
                Bluetooth manager
              </T>
            </div>
          </div>
          <div
            style={{
              ...column,
              padding: 14,
              gap: 10,
              backgroundColor: C.panel,
              borderWidth: 1,
              borderColor: C.border,
              borderRadius: 8,
            }}
          >
            <div style={{ ...row, justifyContent: "space-between" }}>
              <T size={13} weight={500}>
                Bluetooth
              </T>
              <Toggle
                testId="bluetooth-power"
                label="Bluetooth power"
                value={power}
                disabled={!canControl || busy}
                onChange={changePower}
              />
            </div>
            <div testId="service-status" style={{ ...row, gap: 6 }}>
              <div
                style={{
                  width: 5,
                  height: 5,
                  borderRadius: 3,
                  backgroundColor: power ? C.green : C.subtle,
                }}
              />
              <T size={10} color={C.muted}>
                {bt.loading
                  ? "Connecting to service…"
                  : !bt.available
                    ? "Service unavailable"
                    : !adapter
                      ? "No adapter found"
                      : power
                        ? "Connected to BlueZ"
                        : "Bluetooth off"}
              </T>
            </div>
          </div>
          <div style={{ ...column, gap: 6 }}>
            {(["Devices", "File transfers", "Activity"] as const).map(
              (p, i) => (
                <Button
                  key={p}
                  testId={`nav-${p.toLowerCase().replaceAll(" ", "-")}`}
                  onClick={() => nav(p)}
                  style={{
                    height: 38,
                    justifyContent: "flex-start",
                    paddingLeft: 12,
                    borderWidth: 0,
                    backgroundColor: page === p ? C.selection : "transparent",
                    gap: 10,
                  }}
                >
                  <Icon
                    name={
                      (["bluetooth", "arrow-left-right", "clock-3"] as const)[i]
                    }
                    color={page === p ? C.accent : C.muted}
                  />
                  <T size={13} color={page === p ? C.fg : C.muted} weight={500}>
                    {p}
                  </T>
                </Button>
              ),
            )}
          </div>
          <div style={fill} />
          <div
            style={{
              ...column,
              borderTopWidth: 1,
              borderColor: C.border,
              paddingTop: 16,
              paddingBottom: 16,
              paddingLeft: 4,
              paddingRight: 4,
              gap: 12,
            }}
          >
            <Mono size={9} color={C.subtle}>
              THIS COMPUTER
            </Mono>
            <div style={{ ...row, gap: 10 }}>
              <Icon name="laptop" size={20} />
              <div style={{ ...column, gap: 4 }}>
                <T size={12} weight={500}>
                  {adapter?.name ?? "No Bluetooth adapter"}
                </T>
                <T size={10} color={C.muted}>
                  {adapter
                    ? `${adapter.id.split("/").at(-1)} · ${adapter.address}`
                    : "BlueZ system service"}
                </T>
              </div>
            </div>
          </div>
          <Button
            testId="nav-settings"
            onClick={() => nav("Settings")}
            style={{
              justifyContent: "flex-start",
              height: 38,
              paddingLeft: 12,
              borderWidth: 0,
              backgroundColor:
                page === "Settings" ? C.selection : "transparent",
              gap: 10,
            }}
          >
            <Icon name="settings-2" />
            <T color={C.muted}>Settings</T>
          </Button>
        </div>
        <div
          style={{
            ...column,
            ...fill,
            paddingTop: 30,
            paddingLeft: 32,
            paddingRight: 32,
            paddingBottom: 24,
            gap: 22,
            ...gradient(C.bg, "#121820"),
          }}
        >
          <div style={{ ...row, gap: 16, justifyContent: "space-between" }}>
            <div style={{ ...column, gap: 8, ...fill }}>
              <T size={28} weight={600}>
                {page === "Devices" ? "Your devices" : page}
              </T>
              <T size={12} color={C.muted}>
                {page === "Devices"
                  ? "Manage your connections and discover nearby devices."
                  : page === "File transfers"
                    ? "Bluetooth file transfers."
                    : page === "Activity"
                      ? "Bluetooth changes observed during this session."
                      : "Manage your system Bluetooth adapter."}
              </T>
            </div>
            {page === "Devices" && (
              <Button
                testId="add-device"
                primary
                disabled={!canControl || !power || busy}
                onClick={() => openPair()}
              >
                <Icon name="plus" size={16} color="#FFFFFF" />
                <T weight={600}>Add device</T>
              </Button>
            )}
          </div>
          {failure && (
            <div
              testId="service-error"
              role="alert"
              style={{
                ...column,
                gap: 8,
                padding: 12,
                borderWidth: 1,
                borderColor: "#6D3B43",
                borderRadius: 6,
                backgroundColor: "#321E26",
                flexShrink: 0,
              }}
            >
              <T size={12} color={C.red} style={{ lineHeight: 18 }}>
                {failure}
              </T>
              {error && (
                <Button
                  small
                  onClick={() => setError(null)}
                  style={{ alignSelf: "flex-start" }}
                >
                  Dismiss
                </Button>
              )}
            </div>
          )}
          {pending && (
            <div
              testId="pending-action"
              style={{ ...row, gap: 8, flexShrink: 0 }}
            >
              <Icon name="radar" size={14} color={C.accent} />
              <T size={11} color={C.accent}>
                {pending}…
              </T>
            </div>
          )}
          {page === "Devices" ? (
            <>
              <div style={{ ...row, gap: 20, justifyContent: "space-between" }}>
                <div
                  style={{
                    ...row,
                    gap: 4,
                    padding: 3,
                    borderRadius: 7,
                    backgroundColor: C.bg,
                  }}
                >
                  {(["all", "connected", "saved"] as DeviceFilter[]).map(
                    (f) => (
                      <Button
                        key={f}
                        testId={`filter-${f}`}
                        onClick={() => setFilter(f)}
                        style={{
                          height: 31,
                          paddingLeft: 12,
                          paddingRight: 12,
                          borderWidth: 0,
                          backgroundColor:
                            filter === f ? C.raised : "transparent",
                        }}
                      >
                        <T size={12} color={filter === f ? C.fg : C.muted}>
                          {f === "all"
                            ? "All devices"
                            : f === "connected"
                              ? "Connected"
                              : "Saved"}
                        </T>
                      </Button>
                    ),
                  )}
                </div>
                <Search
                  value={query}
                  onChange={setQuery}
                  testId="device-search"
                  width={250}
                />
              </div>
              <div
                testId="device-list"
                style={{
                  ...column,
                  ...fill,
                  gap: 24,
                  overflowY: "scroll",
                  scrollbarColor: C.border,
                  scrollbarWidth: 3,
                }}
              >
                {bt.loading ? (
                  <EmptyState
                    icon="bluetooth"
                    title="Loading Bluetooth devices"
                    description="Connecting to the system Bluetooth service…"
                  />
                ) : !bt.available ? (
                  <EmptyState
                    icon="bluetooth-off"
                    title="Bluetooth service unavailable"
                    description="Check that the system Bluetooth service is running."
                  >
                    <Button
                      disabled={busy}
                      onClick={() =>
                        void run("Reconnecting to Bluetooth", () =>
                          bluetooth.start(),
                        )
                      }
                    >
                      Reconnect
                    </Button>
                  </EmptyState>
                ) : !adapter ? (
                  <EmptyState
                    icon="bluetooth-off"
                    title="No Bluetooth adapter"
                    description="Connect a Bluetooth adapter to get started."
                  />
                ) : !power ? (
                  <EmptyState
                    icon="bluetooth-off"
                    title="Bluetooth is off"
                    description="Turn on Bluetooth to connect your devices."
                  >
                    <Button primary disabled={busy} onClick={changePower}>
                      Turn on Bluetooth
                    </Button>
                  </EmptyState>
                ) : (
                  <>
                    {(["connected", "saved", "nearby"] as const).map(
                      (status) => {
                        const list = visibleDevices.filter(
                          (d) => d.status === status,
                        );
                        return list.length ? (
                          <div
                            key={status}
                            style={{ ...column, flexShrink: 0 }}
                          >
                            <div
                              style={{
                                ...row,
                                justifyContent: "space-between",
                                marginBottom: 12,
                              }}
                            >
                              <div style={{ ...row, gap: 9 }}>
                                <T size={12} weight={500}>
                                  {status === "connected"
                                    ? "Connected"
                                    : status === "saved"
                                      ? "Saved"
                                      : "Nearby"}
                                </T>
                                <Mono size={10}>{list.length}</Mono>
                              </div>
                              {status === "nearby" && adapter.discovering && (
                                <Radar />
                              )}
                            </div>
                            {list.map((d) => (
                              <div
                                key={d.id}
                                testId={`device-${d.id}`}
                                style={{
                                  ...row,
                                  position: "relative",
                                  gap: 14,
                                  height: status === "connected" ? 72 : 68,
                                  flexShrink: 0,
                                  paddingLeft: 16,
                                  paddingRight: 16,
                                  borderRadius: d.id === selected ? 7 : 0,
                                  borderWidth: d.id === selected ? 1 : 0,
                                  borderBottomWidth: 1,
                                  borderColor:
                                    d.id === selected ? C.primary : C.border,
                                  backgroundColor:
                                    d.id === selected
                                      ? C.selection
                                      : "transparent",
                                  cursor: "pointer",
                                  hover: {
                                    backgroundColor:
                                      d.id === selected ? "#233755" : "#191E26",
                                  },
                                  focusVisible: {
                                    outlineWidth: 1,
                                    outlineColor: C.accent,
                                  },
                                }}
                              >
                                {d.id === selected && (
                                  <DitherLayer
                                    variant="selection"
                                    strength={0.9}
                                    radius={7}
                                  />
                                )}
                                <div
                                  role="button"
                                  testId={`device-select-${d.id}`}
                                  aria-label={`Select ${d.name}`}
                                  tabIndex={0}
                                  onClick={() => {
                                    setSelected(d.id);
                                    setMenu(false);
                                  }}
                                  onKeyDown={(e) => {
                                    if (
                                      e.key === "enter" ||
                                      e.key === "space"
                                    ) {
                                      e.preventDefault();
                                      e.stopPropagation();
                                      setSelected(d.id);
                                      setMenu(false);
                                    }
                                  }}
                                  style={{
                                    ...row,
                                    ...fill,
                                    gap: 14,
                                    height: "100%",
                                    cursor: "pointer",
                                    borderRadius: 6,
                                    focusVisible: {
                                      outlineWidth: 1,
                                      outlineColor: C.accent,
                                    },
                                  }}
                                >
                                  <div
                                    style={{
                                      ...row,
                                      width: 42,
                                      height: 42,
                                      borderRadius: 8,
                                      backgroundColor: C.raised,
                                      justifyContent: "center",
                                      flexShrink: 0,
                                    }}
                                  >
                                    <Icon
                                      name={kindIcon[d.kind]}
                                      size={22}
                                      color={
                                        d.id === selected ? C.accent : C.muted
                                      }
                                    />
                                  </div>
                                  <div style={{ ...column, gap: 6, ...fill }}>
                                    <T size={13} weight={500}>
                                      {d.name}
                                    </T>
                                    <T size={11} color={C.muted}>
                                      {kindTitle[d.kind]} ·{" "}
                                      {status === "connected"
                                        ? "Connected"
                                        : d.paired
                                          ? "Paired"
                                          : "Not paired"}
                                    </T>
                                  </div>
                                </div>
                                {status === "connected" ? (
                                  <>
                                    {d.battery !== undefined && (
                                      <div style={{ ...row, gap: 6 }}>
                                        <Icon
                                          name="battery-medium"
                                          size={17}
                                          color={C.green}
                                        />
                                        <Mono size={11}>{d.battery}%</Mono>
                                      </div>
                                    )}
                                    <IconButton
                                      name="ellipsis"
                                      testId={`device-options-${d.id}`}
                                      label={`Options for ${d.name}`}
                                      onClick={(anchor) => {
                                        setSelected(d.id);
                                        setMenuAnchor(anchor);
                                        setMenu((current) => !current);
                                      }}
                                    />
                                  </>
                                ) : (
                                  <Button
                                    testId={`${status === "nearby" ? "pair" : "connect"}-${d.id}`}
                                    small
                                    disabled={busy}
                                    onClick={() =>
                                      status === "nearby"
                                        ? openPair(d.id)
                                        : connect(d)
                                    }
                                    style={
                                      status === "nearby"
                                        ? {
                                            backgroundColor: C.selection,
                                            borderWidth: 0,
                                          }
                                        : {}
                                    }
                                  >
                                    <T
                                      size={11}
                                      color={
                                        status === "nearby" ? C.accent : C.fg
                                      }
                                    >
                                      {status === "nearby" ? "Pair" : "Connect"}
                                    </T>
                                  </Button>
                                )}
                              </div>
                            ))}
                          </div>
                        ) : null;
                      },
                    )}
                    {!visibleDevices.length && (
                      <EmptyState
                        icon="search"
                        title={
                          query ? "No devices found" : "No devices to show"
                        }
                        description={
                          query
                            ? "Try a different search or filter."
                            : "Use Add device to discover nearby Bluetooth devices."
                        }
                      >
                        {query || filter !== "all" ? (
                          <Button
                            onClick={() => {
                              setQuery("");
                              setFilter("all");
                            }}
                          >
                            Clear filters
                          </Button>
                        ) : null}
                      </EmptyState>
                    )}
                  </>
                )}
              </div>
              <div
                style={{
                  ...row,
                  borderTopWidth: 1,
                  borderColor: C.border,
                  paddingTop: 16,
                  justifyContent: "space-between",
                  flexShrink: 0,
                }}
              >
                <div style={{ ...row, gap: 7 }}>
                  <Icon name={visible ? "eye" : "eye-off"} size={13} />
                  <T size={11} color={C.muted}>
                    {visible
                      ? `Visible as ${adapter?.name}`
                      : "Hidden from nearby devices"}
                  </T>
                </div>
                <Button
                  testId="discoverable"
                  disabled={!canControl || !power || busy}
                  onClick={changeVisible}
                  style={{
                    height: 16,
                    borderWidth: 0,
                    backgroundColor: "transparent",
                    padding: 0,
                  }}
                >
                  <T size={11} color={C.accent}>
                    {visible ? "Stop sharing" : "Make discoverable"}
                  </T>
                </Button>
              </div>
            </>
          ) : page === "Activity" ? (
            <div style={{ ...column, gap: 12, ...fill, overflowY: "scroll" }}>
              {activity.length ? (
                activity.map((a, i) => (
                  <div
                    key={`${a.time}-${i}`}
                    style={{
                      ...row,
                      padding: 18,
                      gap: 14,
                      borderWidth: 1,
                      borderColor: C.border,
                      borderRadius: 7,
                      backgroundColor: C.panel,
                    }}
                  >
                    <Icon name="clock-3" color={i === 0 ? C.accent : C.muted} />
                    <div style={{ ...column, gap: 5, ...fill }}>
                      <T>{a.message}</T>
                      <T size={11} color={C.muted}>
                        {new Date(a.time).toLocaleTimeString()} ·{" "}
                        {adapter?.name ?? "BlueZ"}
                      </T>
                    </div>
                  </div>
                ))
              ) : (
                <EmptyState
                  icon="clock-3"
                  title="No activity yet"
                  description="Connection changes and completed actions will appear here."
                />
              )}
            </div>
          ) : page === "Settings" ? (
            <div
              style={{
                ...column,
                gap: 24,
                padding: 24,
                borderRadius: 8,
                borderWidth: 1,
                borderColor: C.border,
                backgroundColor: C.panel,
              }}
            >
              <T size={17} weight={600}>
                Bluetooth adapter
              </T>
              {bt.adapters.length > 1 && (
                <Select
                  value={bt.adapterId ?? ""}
                  onValueChange={(id) => bluetooth.selectAdapter(id)}
                >
                  <SelectTrigger
                    style={{
                      ...row,
                      padding: 12,
                      backgroundColor: C.raised,
                      borderRadius: 6,
                    }}
                  >
                    <SelectValue />
                    <Icon name="chevron-down" size={14} />
                  </SelectTrigger>
                  <SelectContent
                    style={{ backgroundColor: C.raised, padding: 4 }}
                  >
                    {bt.adapters.map((a) => (
                      <SelectItem
                        key={a.id}
                        value={a.id}
                        style={{ padding: 10 }}
                      >
                        <T>{a.name}</T>
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
              <Info
                label="Computer name"
                value={adapter?.name ?? "Unavailable"}
              />
              <Info
                label="Adapter"
                value={adapter?.id.split("/").at(-1) ?? "Unavailable"}
              />
              <Info label="Address" value={adapter?.address ?? "Unavailable"} />
              <Info
                label="Service"
                value={bt.available ? "BlueZ · system D-Bus" : "Unavailable"}
              />
              <div style={line} />
              <Preference
                title="Bluetooth"
                subtitle="Allow Bluetooth connections"
                value={power}
                disabled={!canControl || busy}
                testId="settings-power"
                onChange={changePower}
              />
              <Preference
                title="Discoverable"
                subtitle="Let nearby devices find this computer"
                value={visible}
                disabled={!canControl || !power || busy}
                testId="settings-discoverable"
                onChange={changeVisible}
              />
              <T size={11} color={C.muted}>
                Connected to the Bluetooth service installed on this computer.
              </T>
            </div>
          ) : (
            <EmptyState
              icon="arrow-left-right"
              title="File transfers are unavailable"
              description="Bluetooth file transfer support has not been configured in Bluet. Device connections and pairing use the system service."
            />
          )}
        </div>
        <div
          style={{
            ...column,
            width: 340,
            position: "relative",
            flexShrink: 0,
            padding: 24,
            gap: 22,
            borderLeftWidth: 1,
            borderColor: C.border,
            ...gradient(C.panel, "#1B222D"),
            overflowY: "scroll",
            scrollbarColor: C.border,
            scrollbarWidth: 3,
          }}
        >
          <DitherLayer variant="ambient" strength={1} opacity={0.8} />
          <div
            style={{
              ...row,
              justifyContent: "space-between",
              height: 28,
              flexShrink: 0,
            }}
          >
            <T size={12} color={C.muted}>
              Device details
            </T>
            {device && (
              <IconButton
                testId="device-options"
                name="ellipsis"
                label="Device options"
                onClick={(anchor) => {
                  setMenuAnchor(anchor);
                  setMenu((current) => !current);
                }}
              />
            )}
          </div>
          {device ? (
            <>
              <div
                style={{
                  ...column,
                  alignItems: "center",
                  gap: 10,
                  paddingTop: 4,
                  flexShrink: 0,
                }}
              >
                <div
                  style={{
                    ...row,
                    width: 88,
                    position: "relative",
                    height: 88,
                    borderRadius: 44,
                    justifyContent: "center",
                    backgroundColor: "#1C2D46",
                    borderWidth: 7,
                    borderColor: "#20314B",
                  }}
                >
                  <DitherLayer variant="halo" strength={1} radius={44} />
                  <div
                    style={{
                      ...row,
                      width: 68,
                      height: 68,
                      borderRadius: 34,
                      borderWidth: 3,
                      borderColor: "#2A4672",
                      justifyContent: "center",
                    }}
                  >
                    <Icon
                      name={kindIcon[device.kind]}
                      size={38}
                      color={C.accent}
                    />
                  </div>
                </div>
                <div
                  style={{
                    ...column,
                    alignItems: "center",
                    gap: 6,
                    paddingTop: 6,
                  }}
                >
                  <T size={22} weight={600}>
                    {device.name}
                  </T>
                  <T size={11} color={C.muted}>
                    {device.description}
                  </T>
                </div>
                <div
                  style={{
                    ...row,
                    gap: 6,
                    paddingTop: 4,
                    paddingBottom: 4,
                    paddingLeft: 8,
                    paddingRight: 8,
                    borderRadius: 4,
                    backgroundColor: connected ? C.greenBg : C.raised,
                  }}
                >
                  <div
                    style={{
                      width: 5,
                      height: 5,
                      borderRadius: 3,
                      backgroundColor: connected ? C.green : C.muted,
                    }}
                  />
                  <T size={11} color={connected ? C.green : C.muted}>
                    {connected
                      ? "Connected"
                      : device.paired
                        ? "Paired · disconnected"
                        : "Not paired"}
                  </T>
                </div>
              </div>
              {device.battery !== undefined ? (
                <div
                  style={{
                    ...column,
                    gap: 10,
                    paddingBottom: 2,
                    flexShrink: 0,
                  }}
                >
                  <div style={{ ...row, justifyContent: "space-between" }}>
                    <div style={{ ...row, gap: 7 }}>
                      <Icon name="battery-medium" size={15} color={C.green} />
                      <T size={11} color={C.muted}>
                        Battery
                      </T>
                    </div>
                    <T size={12} weight={500}>
                      {device.battery}%
                    </T>
                  </div>
                  <div
                    style={{
                      height: 5,
                      position: "relative",
                      backgroundColor: C.raised,
                      borderRadius: 3,
                    }}
                  >
                    <div
                      style={{
                        height: 5,
                        width: `${device.battery}%`,
                        borderRadius: 3,
                        backgroundColor: "#315D49",
                      }}
                    />
                    <DitherLayer
                      variant="meter"
                      value={device.battery / 100}
                      radius={3}
                    />
                  </div>
                </div>
              ) : (
                <Info label="Battery" value="Not reported" />
              )}
              <div style={line} />
              {(device.kind === "headphones" || device.kind === "speaker") && (
                <>
                  <div style={{ ...column, gap: 14, flexShrink: 0 }}>
                    <div style={{ ...row, justifyContent: "space-between" }}>
                      <T size={13} weight={500}>
                        Audio
                      </T>
                      <Mono size={8}>
                        {audioDevice?.codec?.toUpperCase() ??
                          (sound.available ? "SYSTEM AUDIO" : "UNAVAILABLE")}
                      </Mono>
                    </div>
                    {audioDevice ? (
                      <>
                        <div style={{ ...column, gap: 8 }}>
                          <T size={11} color={C.muted}>
                            Output profile
                          </T>
                          <Select
                            style={{ width: "100%" }}
                            value={audioDevice.profile ?? ""}
                            disabled={busy || !audioDevice.profiles.length}
                            onValueChange={(profile) =>
                              void run("Changing audio profile", () =>
                                audio.setProfile(device.address, profile),
                              )
                            }
                          >
                            <SelectTrigger
                              testId="audio-profile"
                              style={{
                                ...row,
                                width: "100%",
                                justifyContent: "space-between",
                                height: 38,
                                paddingLeft: 12,
                                paddingRight: 12,
                                backgroundColor: C.raised,
                                borderWidth: 1,
                                borderColor: C.border,
                                borderRadius: 6,
                              }}
                            >
                              <SelectValue>
                                <T size={12}>
                                  {shortProfileLabel(
                                    audioDevice.profiles.find(
                                      (p) => p.id === audioDevice.profile,
                                    )?.label ??
                                      audioDevice.profile ??
                                      "No active profile",
                                  )}
                                </T>
                              </SelectValue>
                              <Icon name="chevron-down" size={14} />
                            </SelectTrigger>
                            <SelectContent
                              style={{
                                ...column,
                                width: 292,
                                padding: 4,
                                gap: 4,
                                borderRadius: 6,
                                backgroundColor: C.raised,
                                borderWidth: 1,
                                borderColor: C.border,
                              }}
                            >
                              {audioDevice.profiles.map((profile) => (
                                <SelectItem
                                  key={profile.id}
                                  value={profile.id}
                                  aria-label={profile.label}
                                  style={{
                                    padding: 10,
                                    borderRadius: 4,
                                    hover: { backgroundColor: C.selection },
                                  }}
                                >
                                  <T
                                    size={12}
                                    style={{
                                      lineClamp: 1,
                                      textOverflow: "ellipsis",
                                    }}
                                  >
                                    {shortProfileLabel(profile.label)}
                                  </T>
                                </SelectItem>
                              ))}
                            </SelectContent>
                          </Select>
                        </div>
                        <div style={{ ...column, gap: 10 }}>
                          <Info
                            label="Volume"
                            value={
                              audioDevice.sinkName
                                ? `${audioDevice.muted ? 0 : volume}%`
                                : "Unavailable"
                            }
                          />
                          <div style={{ ...row, gap: 10, height: 16 }}>
                            <Button
                              testId="audio-mute"
                              label={audioDevice.muted ? "Unmute" : "Mute"}
                              disabled={busy || !audioDevice.sinkName}
                              onClick={() =>
                                void run("Changing mute", () =>
                                  audio.setMuted(
                                    device.address,
                                    !audioDevice.muted,
                                  ),
                                )
                              }
                              style={{
                                width: 16,
                                height: 16,
                                padding: 0,
                                borderWidth: 0,
                                backgroundColor: "transparent",
                              }}
                            >
                              <Icon
                                name={
                                  audioDevice.muted ? "volume-x" : "volume-2"
                                }
                                size={15}
                              />
                            </Button>
                            <div
                              testId="volume-slider"
                              role="slider"
                              aria-label="Volume"
                              aria-valuetext={`${volume}%`}
                              tabIndex={audioDevice.sinkName ? 0 : undefined}
                              onLayout={(e) =>
                                setVolumeBounds({
                                  x: e.x ?? 0,
                                  width: e.width ?? 266,
                                })
                              }
                              onMouseDown={(e) => {
                                setDragging(true);
                                changeVolume(e);
                              }}
                              onMouseMove={(e) => {
                                if (dragging) changeVolume(e);
                              }}
                              onMouseUp={commitVolume}
                              onKeyDown={(e) => {
                                if (e.key === "left" || e.key === "down")
                                  setVolume(volume - 5);
                                if (e.key === "right" || e.key === "up")
                                  setVolume(volume + 5);
                              }}
                              style={{
                                ...row,
                                height: 16,
                                ...fill,
                                cursor: audioDevice.sinkName
                                  ? "pointer"
                                  : "not-allowed",
                                position: "relative",
                                opacity: audioDevice.sinkName ? 1 : 0.4,
                              }}
                            >
                              <div
                                style={{
                                  height: 4,
                                  width: "100%",
                                  backgroundColor: C.border,
                                  borderRadius: 3,
                                }}
                              >
                                <div
                                  style={{
                                    height: 4,
                                    width: `${audioDevice.muted ? 0 : volume}%`,
                                    backgroundColor: C.primary,
                                    borderRadius: 3,
                                  }}
                                />
                              </div>
                              <div
                                style={{
                                  pointerEvents: "none",
                                  position: "absolute",
                                  left: Math.max(
                                    0,
                                    ((volumeBounds.width - 10) *
                                      (audioDevice.muted ? 0 : volume)) /
                                      100,
                                  ),
                                  width: 10,
                                  height: 10,
                                  borderRadius: 5,
                                  backgroundColor: C.fg,
                                }}
                              />
                            </div>
                          </div>
                        </div>
                      </>
                    ) : (
                      <T size={11} color={C.muted} style={{ lineHeight: 17 }}>
                        {sound.error ??
                          (connected
                            ? "No system audio output is available for this device."
                            : "Connect this device to access system audio controls.")}
                      </T>
                    )}
                  </div>
                  <div style={line} />
                </>
              )}
              <div style={{ ...column, gap: 20, flexShrink: 0 }}>
                <Preference
                  title="Trusted device"
                  subtitle="Allow connections without authorization"
                  value={device.trusted}
                  disabled={busy || !canControl}
                  testId="trusted-device"
                  onChange={() =>
                    void run("Changing device trust", () =>
                      bluetooth.setTrusted(device.id, !device.trusted),
                    )
                  }
                />
                <Info label="Paired" value={device.paired ? "Yes" : "No"} />
              </div>
              <div style={line} />
              <div style={{ ...column, gap: 12, flexShrink: 0 }}>
                <Info
                  label="Signal strength"
                  value={
                    device.rssi === undefined
                      ? "Not reported"
                      : `${device.rssi} dBm`
                  }
                  color={device.rssi === undefined ? C.muted : C.green}
                  size={10}
                />
                <Info
                  label="Connected for"
                  value={
                    connected
                      ? device.connectedSince
                        ? `${Math.max(0, Math.floor((Date.now() - device.connectedSince) / 60000))} minutes`
                        : "Not reported"
                      : "—"
                  }
                  size={10}
                />
                <div style={{ ...row, justifyContent: "space-between" }}>
                  <T size={10} color={C.muted}>
                    Device address
                  </T>
                  <Mono size={10} color={C.fg}>
                    {device.address}
                  </Mono>
                </div>
              </div>
              <div style={{ ...fill, minHeight: 20 }} />
              <div
                style={{
                  ...column,
                  gap: 14,
                  alignItems: "center",
                  flexShrink: 0,
                }}
              >
                <Button
                  testId="connection-action"
                  disabled={busy || !canControl || !power}
                  onClick={() =>
                    connected
                      ? disconnect(device)
                      : !device.paired
                        ? openPair(device.id)
                        : connect(device)
                  }
                  style={{ width: "100%" }}
                >
                  {connected
                    ? "Disconnect"
                    : !device.paired
                      ? "Pair device"
                      : "Connect"}
                </Button>
                {device.paired && (
                  <Button
                    testId="forget-device"
                    disabled={busy}
                    onClick={() => setDialog("forget")}
                    style={{
                      height: 14,
                      padding: 0,
                      borderWidth: 0,
                      backgroundColor: "transparent",
                    }}
                  >
                    <T size={11} color={C.red}>
                      Forget device
                    </T>
                  </Button>
                )}
              </div>
            </>
          ) : (
            <EmptyState
              icon="bluetooth"
              title="No device selected"
              description="Select a device to see its details."
            />
          )}
        </div>
      </div>
      {menu && device && (
        <div
          testId="device-menu"
          style={{
            ...column,
            position: "absolute",
            top: Math.min(
              menuAnchor.y + menuAnchor.height + 6,
              (windowSize.height || 1000) - 104,
            ),
            left: Math.max(
              8,
              Math.min(
                menuAnchor.x + menuAnchor.width - 200,
                (windowSize.width || 1440) - 208,
              ),
            ),
            width: 200,
            padding: 6,
            borderRadius: 7,
            backgroundColor: C.raised,
            borderWidth: 1,
            borderColor: C.border,
            boxShadow: {
              offsetX: 0,
              offsetY: 8,
              blurRadius: 24,
              spreadRadius: 0,
              color: "#00000060",
            },
          }}
          onMouseDownOutside={(event) => {
            // The trigger owns repeated clicks. Dismissing on its pointer-down
            // would race its click handler and reopen the menu immediately.
            if (
              event.x !== undefined &&
              event.y !== undefined &&
              event.x >= menuAnchor.x &&
              event.x <= menuAnchor.x + menuAnchor.width &&
              event.y >= menuAnchor.y &&
              event.y <= menuAnchor.y + menuAnchor.height
            )
              return;
            setMenu(false);
          }}
        >
          <Button
            disabled={busy || !power}
            onClick={() => {
              connected
                ? disconnect(device)
                : device.paired
                  ? connect(device)
                  : openPair(device.id);
              setMenu(false);
            }}
            style={{ justifyContent: "flex-start", borderWidth: 0 }}
          >
            {connected
              ? "Disconnect"
              : !device.paired
                ? "Pair device"
                : "Connect"}
          </Button>
          {device.paired && (
            <Button
              disabled={busy}
              onClick={() => {
                setDialog("forget");
                setMenu(false);
              }}
              style={{ justifyContent: "flex-start", borderWidth: 0 }}
            >
              <T color={C.red}>Forget device</T>
            </Button>
          )}
        </div>
      )}
      {toast && (
        <div
          testId="toast"
          style={{
            ...row,
            position: "absolute",
            bottom: 24,
            left: 252,
            padding: 14,
            gap: 10,
            borderWidth: 1,
            borderColor: C.border,
            borderRadius: 8,
            backgroundColor: C.raised,
          }}
        >
          <Icon name="circle-check" size={17} color={C.green} />
          <T size={12}>{toast}</T>
        </div>
      )}
      {dialog && (
        <div
          testId="dialog-backdrop"
          style={{
            ...row,
            position: "absolute",
            left: 0,
            right: 0,
            top: 44,
            bottom: 0,
            justifyContent: "center",
            backgroundColor: "#06080DB8",
            pointerEvents: "auto",
          }}
        >
          <div
            testId="pair-dialog"
            role="dialog"
            aria-label={dialog === "pair" ? "Add a device" : "Forget device"}
            style={{
              ...column,
              position: "relative",
              width: 480,
              padding: 28,
              gap: 24,
              borderRadius: 12,
              borderWidth: 1,
              borderColor: C.border,
              ...gradient(C.panel, "#1B2533"),
              boxShadow: {
                offsetX: 0,
                offsetY: 16,
                blurRadius: 56,
                spreadRadius: 0,
                color: "#00000040",
              },
            }}
          >
            <DitherLayer
              variant="ambient"
              strength={1}
              radius={12}
              opacity={0.9}
            />
            <div style={{ ...row, justifyContent: "space-between" }}>
              <T size={21} weight={600}>
                {dialog === "pair" ? "Add a device" : "Forget device?"}
              </T>
              <IconButton
                testId="dialog-close"
                name="x"
                label="Close dialog"
                onClick={closeDialog}
              />
            </div>
            {dialog === "pair" ? (
              <>
                <T size={13} color={C.muted} style={{ lineHeight: 19.5 }}>
                  Make sure your device is turned on and in pairing mode.
                </T>
                <Search
                  value={pairQuery}
                  onChange={setPairQuery}
                  testId="pair-search"
                />
                <div
                  style={{
                    ...column,
                    gap: 10,
                    maxHeight: 300,
                    overflowY: "scroll",
                    scrollbarColor: C.border,
                  }}
                >
                  <div
                    style={{
                      ...row,
                      justifyContent: "space-between",
                      paddingBottom: 4,
                    }}
                  >
                    <Mono size={10}>
                      NEARBY DEVICES ·{" "}
                      {
                        devices.filter(
                          (d) =>
                            !d.paired &&
                            filterDevices([d], "all", pairQuery).length,
                        ).length
                      }
                    </Mono>
                    {adapter?.discovering ? (
                      <Radar label="Scanning…" />
                    ) : (
                      <T size={11} color={C.muted}>
                        Discovery stopped
                      </T>
                    )}
                  </div>
                  {devices
                    .filter(
                      (d) =>
                        !d.paired &&
                        filterDevices([d], "all", pairQuery).length,
                    )
                    .map((d) => (
                      <div
                        key={d.id}
                        testId={`pair-option-${d.id}`}
                        role="button"
                        tabIndex={0}
                        onClick={() => setPairId(d.id)}
                        onKeyDown={(e) => {
                          if (e.key === "enter") setPairId(d.id);
                        }}
                        style={{
                          ...row,
                          height: 76,
                          flexShrink: 0,
                          paddingLeft: 16,
                          paddingRight: 16,
                          gap: 14,
                          borderRadius: 7,
                          borderWidth: 1,
                          borderColor: pairId === d.id ? C.primary : C.border,
                          backgroundColor:
                            pairId === d.id ? C.selection : C.panel,
                          cursor: "pointer",
                        }}
                      >
                        <div
                          style={{
                            ...row,
                            width: 42,
                            height: 42,
                            borderRadius: 8,
                            justifyContent: "center",
                            backgroundColor: C.raised,
                          }}
                        >
                          <Icon
                            name={kindIcon[d.kind]}
                            size={22}
                            color={pairId === d.id ? C.accent : C.muted}
                          />
                        </div>
                        <div style={{ ...column, gap: 6, ...fill }}>
                          <T weight={500}>{d.name}</T>
                          <T size={11} color={C.muted}>
                            {d.description}
                          </T>
                        </div>
                        <Icon
                          name={pairId === d.id ? "circle-check" : "circle"}
                          size={18}
                          color={pairId === d.id ? C.accent : C.subtle}
                        />
                      </div>
                    ))}
                  {!devices.some(
                    (d) =>
                      !d.paired && filterDevices([d], "all", pairQuery).length,
                  ) && (
                    <div
                      style={{
                        ...column,
                        padding: 20,
                        gap: 8,
                        alignItems: "center",
                      }}
                    >
                      <T size={13} color={C.muted}>
                        No nearby devices found
                      </T>
                      <T size={11} color={C.subtle}>
                        {adapter?.discovering
                          ? "Searching for devices in pairing mode…"
                          : "Use Add device again to restart discovery."}
                      </T>
                    </div>
                  )}
                </div>
                <div style={{ ...row, gap: 10, alignItems: "flex-start" }}>
                  <Icon name="info" size={15} />
                  <T
                    size={11}
                    color={C.muted}
                    style={{ ...fill, lineHeight: 16.5 }}
                  >
                    Your computer is{" "}
                    {visible
                      ? `visible as ${adapter?.name}`
                      : "hidden from nearby devices"}
                    . Discovery does not change your visibility setting.
                  </T>
                </div>
                {failure && (
                  <T size={12} color={C.red} style={{ lineHeight: 18 }}>
                    {failure}
                  </T>
                )}
                {pending && (
                  <T size={12} color={C.accent}>
                    {pending}…
                  </T>
                )}
                <div
                  style={{
                    ...row,
                    borderTopWidth: 1,
                    borderColor: C.border,
                    paddingTop: 18,
                    gap: 10,
                    justifyContent: "flex-end",
                  }}
                >
                  <Button testId="pair-cancel" onClick={closeDialog}>
                    Cancel
                  </Button>
                  <Button
                    testId="pair-confirm"
                    primary
                    disabled={
                      busy || !devices.some((d) => d.id === pairId && !d.paired)
                    }
                    onClick={pair}
                  >
                    <Icon name="link" size={15} color="#FFFFFF" />
                    <T weight={600}>Pair device</T>
                  </Button>
                </div>
              </>
            ) : (
              <>
                <T color={C.muted} style={{ lineHeight: 20 }}>
                  Remove {device?.name} from your system's paired devices? This
                  removes its saved pairing keys.
                </T>
                <div style={{ ...row, gap: 10, justifyContent: "flex-end" }}>
                  <Button testId="forget-cancel" onClick={closeDialog}>
                    Cancel
                  </Button>
                  <Button
                    testId="forget-confirm"
                    disabled={busy || !device}
                    onClick={() => {
                      if (device)
                        void run(
                          `Forgetting ${device.name}`,
                          async () => {
                            await bluetooth.remove(device.id);
                            setDialog(null);
                          },
                          `${device.name} forgotten`,
                        );
                    }}
                    style={{ backgroundColor: "#823F4C", borderWidth: 0 }}
                  >
                    Forget device
                  </Button>
                </div>
                {failure && (
                  <T size={12} color={C.red}>
                    {failure}
                  </T>
                )}
              </>
            )}
          </div>
        </div>
      )}
      {bt.prompt && (
        <div
          style={{
            ...row,
            position: "absolute",
            left: 0,
            right: 0,
            top: 44,
            bottom: 0,
            justifyContent: "center",
            backgroundColor: "#06080DE8",
            pointerEvents: "auto",
          }}
        >
          <div
            testId="pairing-prompt"
            role="dialog"
            aria-label="Bluetooth pairing request"
            style={{
              ...column,
              position: "relative",
              width: 480,
              padding: 28,
              gap: 22,
              borderRadius: 12,
              borderWidth: 1,
              borderColor: C.primary,
              backgroundColor: C.panel,
            }}
          >
            <DitherLayer
              variant="ambient"
              strength={1}
              radius={12}
              opacity={0.9}
            />
            <T size={21} weight={600}>
              Bluetooth pairing request
            </T>
            <T size={13} color={C.muted} style={{ lineHeight: 20 }}>
              {bt.prompt.message}
            </T>
            {bt.prompt.passkey && (
              <Mono size={30} color={C.fg}>
                {bt.prompt.passkey}
              </Mono>
            )}
            {(bt.prompt.kind === "pin" || bt.prompt.kind === "passkey") && (
              <input
                testId="pairing-code"
                autoFocus
                value={promptInput}
                onChange={(e) => setPromptInput(e.value ?? "")}
                placeholder={
                  bt.prompt.kind === "pin"
                    ? "Enter PIN code"
                    : "Enter 6-digit passkey"
                }
                style={{
                  height: 42,
                  padding: 12,
                  fontFamily: "Geist Mono",
                  fontSize: 18,
                  color: C.fg,
                  backgroundColor: C.raised,
                  borderWidth: 1,
                  borderColor: C.border,
                  borderRadius: 6,
                }}
              />
            )}
            {failure && (
              <T size={12} color={C.red} style={{ lineHeight: 18 }}>
                {failure}
              </T>
            )}
            {bt.prompt.entered !== undefined && (
              <T size={12} color={C.muted}>
                {bt.prompt.entered} digits entered on the device
              </T>
            )}
            <div style={{ ...row, justifyContent: "flex-end", gap: 10 }}>
              <Button testId="pairing-reject" onClick={rejectPrompt}>
                Cancel
              </Button>
              {bt.prompt.kind !== "display" && (
                <Button
                  testId="pairing-accept"
                  primary
                  disabled={
                    (bt.prompt.kind === "pin" && !promptInput.length) ||
                    (bt.prompt.kind === "passkey" &&
                      !/^\d{1,6}$/.test(promptInput))
                  }
                  onClick={() =>
                    bluetooth.respondPrompt(
                      bt.prompt?.kind === "pin" || bt.prompt?.kind === "passkey"
                        ? promptInput
                        : true,
                    )
                  }
                >
                  {bt.prompt.kind === "confirm"
                    ? "Confirm"
                    : bt.prompt.kind === "authorize"
                      ? "Authorize"
                      : "Continue"}
                </Button>
              )}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
function shortProfileLabel(label: string): string {
  const codec = label.match(/codec\s+([^),]+)/i)?.[1]?.trim();
  const kind = /high fidelity|a2dp/i.test(label)
    ? "High fidelity"
    : /headset|hands.free|hfp|hsp/i.test(label)
      ? "Headset"
      : label;
  return codec ? `${kind} · ${codec}` : kind;
}
function EmptyState({
  icon,
  title,
  description,
  children,
}: {
  icon: IconName;
  title: string;
  description: string;
  children?: React.ReactNode;
}) {
  return (
    <div
      style={{
        ...column,
        ...fill,
        alignItems: "center",
        justifyContent: "center",
        gap: 16,
        padding: 24,
      }}
    >
      <Icon name={icon} size={40} color={C.subtle} />
      <T size={20} weight={600}>
        {title}
      </T>
      <T
        size={12}
        color={C.muted}
        style={{ textAlign: "center", lineHeight: 18, maxWidth: 420 }}
      >
        {description}
      </T>
      {children}
    </div>
  );
}
function Info({
  label,
  value,
  color = C.fg,
  size = 11,
}: {
  label: string;
  value: string;
  color?: string;
  size?: number;
}) {
  return (
    <div style={{ ...row, justifyContent: "space-between" }}>
      <T size={size} color={C.muted}>
        {label}
      </T>
      <T size={size} color={color}>
        {value}
      </T>
    </div>
  );
}
function Preference({
  title,
  subtitle,
  value,
  onChange,
  testId,
  disabled = false,
}: {
  title: string;
  subtitle: string;
  value: boolean;
  onChange: () => void;
  testId: string;
  disabled?: boolean;
}) {
  return (
    <div style={{ ...row, justifyContent: "space-between", gap: 12 }}>
      <div style={{ ...column, gap: 6, ...fill }}>
        <T size={12} weight={500}>
          {title}
        </T>
        <T size={10} color={C.muted}>
          {subtitle}
        </T>
      </div>
      <Toggle
        value={value}
        onChange={onChange}
        testId={testId}
        label={title}
        disabled={disabled}
      />
    </div>
  );
}

if (Bun.isStandaloneExecutable || Bun.main === import.meta.path) {
  const host = runtime;
  if (!runtime.__bluetSignals) {
    process.on("SIGTERM", () => void shutdown());
    process.on("SIGINT", () => void shutdown());
    runtime.__bluetSignals = true;
  }
  if (!host.__bluetRenderer) {
    const renderer = createRenderer();
    renderer.init({
      title: "Bluet",
      appName: "Bluet",
      appId: "dev.bluet.manager",
      width: 1440,
      height: 1000,
      minWidth: 1200,
      minHeight: 760,
      titlebarTransparent: true,
      windowBackground: "opaque",
    });
    renderer.registerFont(readFileSync(interFont));
    renderer.registerFont(readFileSync(monoFont));
    startFrameLoop(renderer, { onTerminated: () => void shutdown() });
    host.__bluetRenderer = renderer;
  }
  render(<BluetApp />, { renderer: host.__bluetRenderer });
}
