use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bluet::bluez::{
    ADAPTER, BATTERY, BluetoothCommand, BluetoothService, BluetoothSnapshot, DEVICE, PromptKind,
    PromptReply,
};
use tokio::sync::mpsc::UnboundedReceiver;
use zbus::message::Header;
use zbus::object_server::SignalEmitter;
use zbus::{Connection, Proxy, interface};
use zbus::{OwnedObjectPath, OwnedVariant as OwnedValue, Variant};

const A: &str = "/org/bluez/hci0";
const D: &str = "/org/bluez/hci0/dev_11_22_33_44_55_66";

fn ov(value: impl Into<Variant<'static>>) -> OwnedValue {
    OwnedValue::try_from(value.into()).unwrap()
}

#[derive(Default)]
struct State {
    powered: bool,
    discoverable: bool,
    discovering: bool,
    connected: bool,
    paired: bool,
    trusted: bool,
    removed: bool,
    calls: Vec<String>,
    agent_path: String,
    agent_owner: String,
}

impl State {
    fn managed_objects(
        &self,
    ) -> HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> {
        let mut objects = HashMap::new();
        objects.insert(
            A.try_into().unwrap(),
            HashMap::from([(
                ADAPTER.to_string(),
                HashMap::from([
                    ("Alias".into(), ov("Test adapter")),
                    ("Address".into(), ov("AA:BB:CC:DD:EE:FF")),
                    ("Powered".into(), ov(self.powered)),
                    ("Discoverable".into(), ov(self.discoverable)),
                    ("Discovering".into(), ov(self.discovering)),
                    ("Pairable".into(), ov(false)),
                ]),
            )]),
        );
        if !self.removed {
            objects.insert(
                D.try_into().unwrap(),
                HashMap::from([
                    (
                        DEVICE.to_string(),
                        HashMap::from([
                            ("Adapter".into(), ov(OwnedObjectPath::try_from(A).unwrap())),
                            ("Alias".into(), ov("Private bus headset")),
                            ("Address".into(), ov("11:22:33:44:55:66")),
                            ("Icon".into(), ov("audio-headset")),
                            ("Connected".into(), ov(self.connected)),
                            ("Paired".into(), ov(self.paired)),
                            ("Trusted".into(), ov(self.trusted)),
                        ]),
                    ),
                    (
                        BATTERY.to_string(),
                        HashMap::from([("Percentage".into(), ov(88u8))]),
                    ),
                ]),
            );
        }
        objects
    }
}

struct Adapter {
    state: Arc<Mutex<State>>,
}

#[interface(name = "org.bluez.Adapter1")]
impl Adapter {
    #[zbus(property)]
    fn alias(&self) -> String {
        "Test adapter".into()
    }

    #[zbus(property)]
    fn address(&self) -> String {
        "AA:BB:CC:DD:EE:FF".into()
    }

    #[zbus(property)]
    fn powered(&self) -> bool {
        self.state.lock().unwrap().powered
    }

    #[zbus(property)]
    fn set_powered(&mut self, value: bool) -> zbus::fdo::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.calls.push(format!("Powered:{value}"));
        state.powered = value;
        if !value {
            state.discovering = false;
        }
        Ok(())
    }

    #[zbus(property)]
    fn discoverable(&self) -> bool {
        self.state.lock().unwrap().discoverable
    }

    #[zbus(property)]
    fn set_discoverable(&mut self, value: bool) -> zbus::fdo::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.calls.push(format!("Discoverable:{value}"));
        state.discoverable = value;
        Ok(())
    }

    #[zbus(property)]
    fn discovering(&self) -> bool {
        self.state.lock().unwrap().discovering
    }

    #[zbus(property)]
    fn pairable(&self) -> bool {
        false
    }

    async fn start_discovery(
        &mut self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.calls.push("StartDiscovery".into());
            state.discovering = true;
        }
        emit_changed(
            &emitter,
            ADAPTER,
            HashMap::from([("Discovering".to_string(), ov(true))]),
        )
        .await;
        Ok(())
    }

    async fn stop_discovery(
        &mut self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.calls.push("StopDiscovery".into());
            state.discovering = false;
        }
        emit_changed(
            &emitter,
            ADAPTER,
            HashMap::from([("Discovering".to_string(), ov(false))]),
        )
        .await;
        Ok(())
    }

    async fn remove_device(
        &mut self,
        path: OwnedObjectPath,
        #[zbus(connection)] connection: &Connection,
    ) -> zbus::fdo::Result<()> {
        if path.as_str() != D {
            return Err(zbus::fdo::Error::Failed("unexpected device".into()));
        }
        self.state.lock().unwrap().calls.push("RemoveDevice".into());
        self.state.lock().unwrap().removed = true;
        connection
            .emit_signal(
                None::<&str>,
                "/",
                "org.freedesktop.DBus.ObjectManager",
                "InterfacesRemoved",
                &(
                    OwnedObjectPath::try_from(D).unwrap(),
                    vec![DEVICE.to_string()],
                ),
            )
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        Ok(())
    }
}

struct Device {
    state: Arc<Mutex<State>>,
}

#[interface(name = "org.bluez.Device1")]
impl Device {
    #[zbus(property)]
    fn adapter(&self) -> OwnedObjectPath {
        OwnedObjectPath::try_from(A).unwrap()
    }

    #[zbus(property)]
    fn alias(&self) -> String {
        "Private bus headset".into()
    }

    #[zbus(property)]
    fn address(&self) -> String {
        "11:22:33:44:55:66".into()
    }

    #[zbus(property)]
    fn icon(&self) -> String {
        "audio-headset".into()
    }

    #[zbus(property)]
    fn connected(&self) -> bool {
        self.state.lock().unwrap().connected
    }

    #[zbus(property)]
    fn paired(&self) -> bool {
        self.state.lock().unwrap().paired
    }

    #[zbus(property)]
    fn trusted(&self) -> bool {
        self.state.lock().unwrap().trusted
    }

    #[zbus(property)]
    fn set_trusted(&mut self, value: bool) -> zbus::fdo::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.calls.push(format!("Trusted:{value}"));
        state.trusted = value;
        Ok(())
    }

    async fn connect(
        &mut self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.calls.push("Connect".into());
            state.connected = true;
        }
        emit_changed(
            &emitter,
            DEVICE,
            HashMap::from([("Connected".to_string(), ov(true))]),
        )
        .await;
        Ok(())
    }

    async fn disconnect(
        &mut self,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        {
            let mut state = self.state.lock().unwrap();
            state.calls.push("Disconnect".into());
            state.connected = false;
        }
        emit_changed(
            &emitter,
            DEVICE,
            HashMap::from([("Connected".to_string(), ov(false))]),
        )
        .await;
        Ok(())
    }

    async fn pair(
        &mut self,
        #[zbus(connection)] connection: &Connection,
        #[zbus(signal_emitter)] emitter: SignalEmitter<'_>,
    ) -> zbus::fdo::Result<()> {
        self.state.lock().unwrap().calls.push("Pair".into());
        let (owner, path) = {
            let state = self.state.lock().unwrap();
            (state.agent_owner.clone(), state.agent_path.clone())
        };
        if owner.is_empty() || path.is_empty() {
            return Err(zbus::fdo::Error::Failed("no pairing agent".into()));
        }
        let agent = Proxy::new(
            connection,
            owner.as_str(),
            path.as_str(),
            "org.bluez.Agent1",
        )
        .await
        .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        agent
            .call::<_, _, ()>(
                "RequestConfirmation",
                &(OwnedObjectPath::try_from(D).unwrap(), 123456u32),
            )
            .await
            .map_err(|error| zbus::fdo::Error::Failed(error.to_string()))?;
        {
            let mut state = self.state.lock().unwrap();
            state.paired = true;
        }
        emit_changed(
            &emitter,
            DEVICE,
            HashMap::from([("Paired".to_string(), ov(true))]),
        )
        .await;
        Ok(())
    }

    fn cancel_pairing(&mut self) {
        self.state
            .lock()
            .unwrap()
            .calls
            .push("CancelPairing".into());
    }
}

async fn emit_changed(
    emitter: &SignalEmitter<'_>,
    interface: &str,
    changed: HashMap<String, OwnedValue>,
) {
    emitter
        .emit(
            "org.freedesktop.DBus.Properties",
            "PropertiesChanged",
            &(interface.to_string(), changed, Vec::<String>::new()),
        )
        .await
        .expect("emit PropertiesChanged");
}

struct AgentManager {
    state: Arc<Mutex<State>>,
}

#[interface(name = "org.bluez.AgentManager1")]
impl AgentManager {
    fn register_agent(
        &mut self,
        path: OwnedObjectPath,
        capability: String,
        #[zbus(header)] header: Header<'_>,
    ) {
        let mut state = self.state.lock().unwrap();
        state.calls.push("RegisterAgent".into());
        assert_eq!(capability, "KeyboardDisplay");
        state.agent_path = path.to_string();
        state.agent_owner = header
            .sender()
            .map(|sender| sender.to_string())
            .unwrap_or_default();
    }

    fn unregister_agent(&mut self, path: OwnedObjectPath) {
        let mut state = self.state.lock().unwrap();
        state.calls.push("UnregisterAgent".into());
        assert_eq!(path.to_string(), state.agent_path);
        state.agent_path.clear();
        state.agent_owner.clear();
    }
}

struct Manager {
    state: Arc<Mutex<State>>,
}

#[interface(name = "org.freedesktop.DBus.ObjectManager")]
impl Manager {
    fn get_managed_objects(
        &self,
    ) -> HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>> {
        self.state.lock().unwrap().managed_objects()
    }
}

async fn wait_for(
    events: &mut UnboundedReceiver<BluetoothSnapshot>,
    mut predicate: impl FnMut(&BluetoothSnapshot) -> bool,
) -> BluetoothSnapshot {
    tokio::time::timeout(Duration::from_secs(8), async {
        let mut last = None;
        while let Some(snapshot) = events.recv().await {
            last = Some(snapshot.clone());
            if predicate(&snapshot) {
                return snapshot;
            }
        }
        panic!("Bluetooth snapshot stream ended: {last:?}");
    })
    .await
    .expect("timed out waiting for Bluetooth state")
}

#[tokio::test(flavor = "current_thread")]
async fn real_dbus_transport_supports_bluez_operations_and_pairing_agent() {
    let Ok(session) = std::env::var("DBUS_SESSION_BUS_ADDRESS") else {
        eprintln!("skipping: DBUS_SESSION_BUS_ADDRESS is not set");
        return;
    };
    if std::env::var("DBUS_SYSTEM_BUS_ADDRESS").unwrap_or_default() != session {
        eprintln!(
            "skipping: run under dbus-run-session with DBUS_SYSTEM_BUS_ADDRESS set to the private bus"
        );
        return;
    }

    let connection = Connection::session().await.expect("private bus");
    let state = Arc::new(Mutex::new(State {
        powered: true,
        ..State::default()
    }));

    connection
        .request_name("org.bluez")
        .await
        .expect("request org.bluez");
    connection
        .object_server()
        .at(
            "/",
            Manager {
                state: state.clone(),
            },
        )
        .await
        .expect("object manager");
    connection
        .object_server()
        .at(
            "/org/bluez",
            AgentManager {
                state: state.clone(),
            },
        )
        .await
        .expect("agent manager");
    connection
        .object_server()
        .at(
            A,
            Adapter {
                state: state.clone(),
            },
        )
        .await
        .expect("adapter");
    connection
        .object_server()
        .at(
            D,
            Device {
                state: state.clone(),
            },
        )
        .await
        .expect("device");

    let (service, mut events) = BluetoothService::start();
    let snapshot = wait_for(&mut events, |snapshot| snapshot.available).await;
    assert_eq!(snapshot.devices.len(), 1);
    assert_eq!(snapshot.devices[0].name, "Private bus headset");
    assert_eq!(snapshot.devices[0].battery, Some(88));
    assert!(
        state.lock().unwrap().calls.is_empty(),
        "startup must only read state: {:?}",
        state.lock().unwrap().calls
    );

    service
        .execute(BluetoothCommand::SetPower(false))
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| !snapshot.adapters[0].powered).await;
    service
        .execute(BluetoothCommand::SetPower(true))
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| snapshot.adapters[0].powered).await;

    service
        .execute(BluetoothCommand::SetDiscoverable(true))
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| snapshot.adapters[0].discoverable).await;

    service
        .execute(BluetoothCommand::SetTrusted(D.into(), true))
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| snapshot.devices[0].trusted).await;

    service
        .execute(BluetoothCommand::StartDiscovery)
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| snapshot.adapters[0].discovering).await;
    service
        .execute(BluetoothCommand::StopDiscovery)
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| !snapshot.adapters[0].discovering).await;

    service
        .execute(BluetoothCommand::Connect(D.into()))
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| {
        snapshot.devices[0].status == bluet::model::DeviceStatus::Connected
    })
    .await;
    service
        .execute(BluetoothCommand::Disconnect(D.into()))
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| {
        snapshot.devices[0].status == bluet::model::DeviceStatus::Nearby
    })
    .await;

    let rejected_service = service.clone();
    let rejected = tokio::spawn(async move {
        rejected_service
            .execute(BluetoothCommand::Pair(D.into()))
            .await
    });
    let snapshot = wait_for(&mut events, |snapshot| {
        snapshot
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.kind == PromptKind::Confirm)
    })
    .await;
    assert_eq!(
        snapshot.prompt.as_ref().unwrap().passkey.as_deref(),
        Some("123456")
    );
    service.respond_prompt(PromptReply::Reject);
    assert!(rejected.await.unwrap().is_err());
    assert!(state.lock().unwrap().agent_path.is_empty());

    let accepted_service = service.clone();
    let accepted = tokio::spawn(async move {
        accepted_service
            .execute(BluetoothCommand::Pair(D.into()))
            .await
    });
    wait_for(&mut events, |snapshot| {
        snapshot
            .prompt
            .as_ref()
            .is_some_and(|prompt| prompt.kind == PromptKind::Confirm)
    })
    .await;
    service.respond_prompt(PromptReply::Accept);
    accepted.await.unwrap().unwrap();
    wait_for(&mut events, |snapshot| snapshot.devices[0].paired).await;
    assert!(state.lock().unwrap().agent_path.is_empty());

    service
        .execute(BluetoothCommand::Remove(D.into()))
        .await
        .unwrap();
    wait_for(&mut events, |snapshot| snapshot.devices.is_empty()).await;

    let calls = state.lock().unwrap().calls.clone();
    for expected in [
        "Powered:false",
        "Powered:true",
        "Discoverable:true",
        "Trusted:true",
        "StartDiscovery",
        "StopDiscovery",
        "Connect",
        "Disconnect",
        "RegisterAgent",
        "Pair",
        "UnregisterAgent",
        "RemoveDevice",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "missing {expected} in {calls:?}"
        );
    }

    service.dispose().await;
}
