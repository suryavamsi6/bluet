import { spawn, execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import assert from "node:assert/strict";
import { connectStdio, type App } from "@gpuix/native/automation";

// Exercise GPUI's real layout, hit testing and input dispatch in a native window.
const project = resolve(import.meta.dir, "..");
const artifacts = join(project, "artifacts");
mkdirSync(artifacts, { recursive: true });
// Keep this live smoke test read-only: never click hardware mutation controls.
const managed = () =>
  JSON.parse(
    execFileSync(
      "busctl",
      [
        "--system",
        "--json=short",
        "call",
        "org.bluez",
        "/",
        "org.freedesktop.DBus.ObjectManager",
        "GetManagedObjects",
      ],
      { encoding: "utf8" },
    ),
  ).data[0] as Record<
    string,
    Record<string, Record<string, { data: unknown }>>
  >;
const before = managed();
const adapters = Object.entries(before).filter(
  ([, interfaces]) => interfaces["org.bluez.Adapter1"],
);
const adapterPath =
  adapters.find(
    ([, interfaces]) => interfaces["org.bluez.Adapter1"]?.Powered?.data,
  )?.[0] ?? adapters[0]?.[0];
const expected = Object.entries(before)
  .filter(
    ([, interfaces]) =>
      interfaces["org.bluez.Device1"]?.Adapter?.data === adapterPath,
  )
  .map(([path, interfaces]) => {
    const device = interfaces["org.bluez.Device1"]!;
    return {
      path,
      name: String(
        device.Alias?.data ?? device.Name?.data ?? device.Address?.data,
      ),
      connected: device.Connected?.data === true,
      paired: device.Paired?.data === true,
    };
  });
const binary = process.argv.includes("--binary");
const executableIndex = process.argv.indexOf("--executable");
if (executableIndex !== -1 && !process.argv[executableIndex + 1])
  throw new Error("--executable requires a path");
const executable =
  executableIndex !== -1
    ? resolve(process.argv[executableIndex + 1]!)
    : binary
      ? join(project, "dist/bluet")
      : undefined;
const child = spawn(executable ?? "bun", executable ? [] : ["app.tsx"], {
  cwd: project,
  stdio: ["pipe", "pipe", "pipe"],
});
let stderr = "";
child.stderr.on("data", (chunk) => {
  stderr += chunk.toString();
});
const deadline = setTimeout(() => {
  console.error("Native smoke check timed out.\n" + stderr);
  child.kill();
  process.exit(1);
}, 60000);
let app: App | undefined;
let capturePrepared = false;
let checks = 0;
const settle = () => new Promise((resolve) => setTimeout(resolve, 120));
const pass = (name: string) => {
  checks++;
  console.log("PASS " + name);
};

async function capture(name: string) {
  if (!process.argv.includes("--capture")) return;
  const root = `/run/user/${process.getuid?.() ?? 1000}/hypr`;
  if (!existsSync(root)) return;
  const signature = readdirSync(root).find((entry) =>
    existsSync(join(root, entry, ".socket.sock")),
  );
  if (!signature) return;
  const env = { ...process.env, HYPRLAND_INSTANCE_SIGNATURE: signature };
  const clients = JSON.parse(
    execFileSync("hyprctl", ["clients", "-j"], { env, encoding: "utf8" }),
  );
  const client = clients.find(
    (entry: { pid: number }) => entry.pid === child.pid,
  );
  if (!client) return;
  execFileSync(
    "hyprctl",
    ["eval", `hl.dispatch(hl.dsp.focus({window="address:${client.address}"}))`],
    { env },
  );
  if (!capturePrepared) {
    execFileSync(
      "hyprctl",
      [
        "eval",
        'hl.dispatch(hl.dsp.window.float({action="set"})); hl.dispatch(hl.dsp.window.resize({exact=true,x=1440,y=1000})); hl.dispatch(hl.dsp.window.center())',
      ],
      { env },
    );
    capturePrepared = true;
  }
  await new Promise((resolve) => setTimeout(resolve, 450));
  const updated = JSON.parse(
    execFileSync("hyprctl", ["clients", "-j"], { env, encoding: "utf8" }),
  ).find((entry: { pid: number }) => entry.pid === child.pid);
  execFileSync(
    "grim",
    [
      "-g",
      `${updated.at[0]},${updated.at[1]} ${updated.size[0]}x${updated.size[1]}`,
      join(artifacts, name + ".png"),
    ],
    { env },
  );
}

try {
  app = await connectStdio({
    write: (chunk) => {
      child.stdin.write(chunk);
    },
    feed: (listener) => {
      child.stdout.on("data", (chunk) => listener(chunk.toString()));
    },
    close: async () => {
      child.kill();
    },
  });
  await app.getByText("Your devices").waitFor();
  await settle();
  for (const device of expected) {
    await app
      .getByTestId("device-list")
      .getByText(device.name)
      .waitFor({ timeoutMs: 10000 });
  }
  if (adapterPath) {
    await app.getByTestId("service-status").waitFor();
    assert.match(
      await app.getByTestId("service-status").textContent(),
      /BlueZ/,
    );
  }
  const initial = await app.call("getAllText", {});
  if (!expected.some((device) => device.name === "WH-1000XM5")) {
    assert(
      !initial.text.includes("WH-1000XM5"),
      "Demo devices must not appear in the real service UI",
    );
  }
  pass(
    "startup renders devices from the running BlueZ service without demo data",
  );
  await capture("bluet-native");
  writeFileSync(
    join(artifacts, "native-tree.json"),
    JSON.stringify(await app.call("getTree", {}), null, 2),
  );
  const click = async (id: string) => {
    await app!.getByTestId(id).click();
    await settle();
  };
  const listText = () => app!.getByTestId("device-list").textContent();
  for (const filter of ["connected", "saved"] as const) {
    await click("filter-" + filter);
    const text = await listText();
    for (const device of expected) {
      const shouldShow =
        filter === "connected"
          ? device.connected
          : device.paired && !device.connected;
      if (shouldShow)
        assert(
          text.includes(device.name),
          `Missing ${filter} device: ${device.name}`,
        );
    }
  }
  await click("filter-all");
  if (expected[0]) {
    await app.getByTestId("device-search").fill(expected[0].name);
    await settle();
    assert((await listText()).includes(expected[0].name));
  }
  await app
    .getByTestId("device-search")
    .fill("bluet-nonexistent-device-846283");
  await settle();
  assert.match(await listText(), /No devices found|No matching devices/);
  await app.getByTestId("device-search").fill("");
  await settle();
  pass("search and status filters work on live device data");
  const connectedDevice = expected.find((device) => device.connected);
  if (connectedDevice) {
    const options = app.getByTestId(`device-options-${connectedDevice.path}`);
    await options.click();
    await settle();
    const menu = app.getByTestId("device-menu");
    assert.equal(
      await menu.count(),
      1,
      "Row options must remain open after its click",
    );
    assert.match(await menu.textContent(), /Disconnect/);
    if (connectedDevice.paired)
      assert.match(await menu.textContent(), /Forget device/);
    const [triggerBounds, menuBounds] = await Promise.all([
      options.bounds(),
      menu.bounds(),
    ]);
    assert(
      menuBounds.y >= triggerBounds.y + triggerBounds.height,
      "Options menu should open below its trigger",
    );
    assert(
      menuBounds.y - triggerBounds.y - triggerBounds.height <= 20,
      "Options menu must stay beside the clicked row",
    );
    await capture("bluet-device-options");
    await options.click();
    await settle();
    assert.equal(
      await menu.count(),
      0,
      "Clicking the options trigger again must close it",
    );
    await options.press("enter");
    await settle();
    assert.equal(
      await menu.count(),
      1,
      "Keyboard activation must open row options",
    );
    await app.getByTestId("device-search").click();
    await settle();
    assert.equal(
      await menu.count(),
      0,
      "Clicking outside must dismiss row options",
    );
    pass(
      "row options support pointer/keyboard activation, local anchoring, toggling and outside dismissal",
    );
  }
  await click("nav-activity");
  await click("nav-file-transfers");
  assert(
    !(await app.call("getAllText", {})).text.some((text) =>
      /simulate a file transfer|File transfer complete/i.test(text),
    ),
  );
  await click("nav-settings");
  assert((await app.call("getAllText", {})).text.includes("Bluetooth adapter"));
  await click("nav-devices");
  pass("secondary navigation contains no simulated actions");
  const after = managed();
  for (const path of [
    adapterPath,
    ...expected.map((device) => device.path),
  ].filter(Boolean) as string[]) {
    for (const iface of ["org.bluez.Adapter1", "org.bluez.Device1"]) {
      for (const key of [
        "Powered",
        "Discoverable",
        "Discovering",
        "Pairable",
        "Connected",
        "Paired",
        "Trusted",
      ]) {
        assert.deepEqual(
          after[path]?.[iface]?.[key],
          before[path]?.[iface]?.[key],
          `Read-only smoke unexpectedly changed ${path} ${key}`,
        );
      }
    }
  }
  pass("adapter, pairing, trust and connection states remain unchanged");
  writeFileSync(
    join(artifacts, "smoke-result.json"),
    JSON.stringify({ passed: checks, runtime: "native GPUIX" }, null, 2) + "\n",
  );
  console.log(`${checks} native smoke checks passed.`);
} catch (error) {
  await capture("bluet-failure");
  if (app)
    writeFileSync(
      join(artifacts, "failure-tree.json"),
      JSON.stringify(await app.call("getTree", {}), null, 2),
    );
  throw error;
} finally {
  clearTimeout(deadline);
  const stopped = new Promise<void>((resolve, reject) => {
    if (child.exitCode !== null || child.signalCode !== null) {
      resolve();
      return;
    }
    const timeout = setTimeout(() => {
      child.kill("SIGKILL");
      reject(
        new Error("Native app did not finish service cleanup on shutdown"),
      );
    }, 5000);
    child.once("exit", () => {
      clearTimeout(timeout);
      resolve();
    });
  });
  await app?.close();
  child.kill();
  await stopped;
  writeFileSync(join(artifacts, "native-stderr.log"), stderr);
}
