use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use futures_util::StreamExt;
use tokio::sync::{mpsc, oneshot};
use zbus::{
    Connection, MatchRule, MessageStream, OwnedObjectPath, OwnedVariant as OwnedValue, Proxy,
};

use crate::model::{Device, DeviceKind, DeviceStatus};

pub const BLUEZ: &str = "org.bluez";
pub const ADAPTER: &str = "org.bluez.Adapter1";
pub const DEVICE: &str = "org.bluez.Device1";
pub const BATTERY: &str = "org.bluez.Battery1";
const AGENT_PATH: &str = "/org/bluet/agent";
const OBJECT_MANAGER: &str = "org.freedesktop.DBus.ObjectManager";
const PROPERTIES: &str = "org.freedesktop.DBus.Properties";

pub type ManagedObjects = HashMap<OwnedObjectPath, HashMap<String, HashMap<String, OwnedValue>>>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adapter {
    pub id: String,
    pub name: String,
    pub address: String,
    pub powered: bool,
    pub discoverable: bool,
    pub discovering: bool,
    pub pairable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    Confirm,
    Pin,
    Passkey,
    Display,
    Authorize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PairingPrompt {
    pub id: u64,
    pub device_id: String,
    pub kind: PromptKind,
    pub message: String,
    pub passkey: Option<String>,
    pub entered: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BluetoothSnapshot {
    pub adapters: Vec<Adapter>,
    pub adapter_id: Option<String>,
    pub devices: Vec<Device>,
    pub loading: bool,
    pub error: Option<String>,
    pub prompt: Option<PairingPrompt>,
    pub available: bool,
}

impl Default for BluetoothSnapshot {
    fn default() -> Self {
        Self {
            adapters: Vec::new(),
            adapter_id: None,
            devices: Vec::new(),
            loading: true,
            error: None,
            prompt: None,
            available: false,
        }
    }
}

fn value<'a>(properties: &'a HashMap<String, OwnedValue>, key: &str) -> Option<&'a OwnedValue> {
    properties.get(key)
}

fn string_value(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    value(properties, key)?.try_clone().ok()?.try_into().ok()
}

fn bool_value(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<bool> {
    value(properties, key)?.try_into().ok()
}

fn u32_value(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<u32> {
    value(properties, key)?.try_into().ok()
}

fn u8_value(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<u8> {
    value(properties, key)?.try_into().ok()
}

fn i16_value(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<i16> {
    value(properties, key)?.try_into().ok()
}

fn map_value<T>(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<T>
where
    T: TryFrom<OwnedValue>,
{
    value(properties, key)?.try_clone().ok()?.try_into().ok()
}

pub fn kind_for(icon: &str, class: u32) -> DeviceKind {
    let icon = icon.to_ascii_lowercase();
    if icon.contains("headset") || icon.contains("headphone") {
        return DeviceKind::Headphones;
    }
    if icon.contains("mouse") {
        return DeviceKind::Mouse;
    }
    if icon.contains("keyboard") {
        return DeviceKind::Keyboard;
    }
    if icon.contains("phone") {
        return DeviceKind::Phone;
    }
    if icon.contains("speaker") || icon.contains("audio-card") {
        return DeviceKind::Speaker;
    }
    if icon.contains("gaming") || icon.contains("gamepad") {
        return DeviceKind::Gamepad;
    }
    let major = (class >> 8) & 0x1f;
    let minor = (class >> 2) & 0x3f;
    if major == 2 {
        return DeviceKind::Phone;
    }
    if major == 4 {
        if [1, 2, 6].contains(&minor) {
            return DeviceKind::Headphones;
        }
        if [4, 5].contains(&minor) {
            return DeviceKind::Speaker;
        }
    }
    if major == 5 {
        if minor & 0x20 != 0 {
            return DeviceKind::Mouse;
        }
        if minor & 0x10 != 0 {
            return DeviceKind::Keyboard;
        }
        if [1, 2].contains(&(minor & 0x0f)) {
            return DeviceKind::Gamepad;
        }
    }
    DeviceKind::Device
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

pub fn map_objects(
    objects: &ManagedObjects,
    connected_since: &HashMap<String, u64>,
) -> (Vec<Adapter>, Vec<Device>) {
    let mut adapters = Vec::new();
    let mut devices = Vec::new();
    let mut entries: Vec<_> = objects.iter().collect();
    entries.sort_by_key(|(path, _)| path.to_string());

    for (path, interfaces) in entries {
        let id = path.to_string();
        if let Some(adapter) = interfaces.get(ADAPTER) {
            adapters.push(Adapter {
                id: id.clone(),
                name: string_value(adapter, "Alias")
                    .or_else(|| string_value(adapter, "Name"))
                    .or_else(|| string_value(adapter, "Address"))
                    .unwrap_or_else(|| "Bluetooth adapter".to_string()),
                address: string_value(adapter, "Address").unwrap_or_default(),
                powered: bool_value(adapter, "Powered").unwrap_or(false),
                discoverable: bool_value(adapter, "Discoverable").unwrap_or(false),
                discovering: bool_value(adapter, "Discovering").unwrap_or(false),
                pairable: bool_value(adapter, "Pairable").unwrap_or(false),
            });
        }
        let Some(device) = interfaces.get(DEVICE) else {
            continue;
        };
        let kind = kind_for(
            &string_value(device, "Icon").unwrap_or_default(),
            u32_value(device, "Class").unwrap_or(0),
        );
        let connected = bool_value(device, "Connected").unwrap_or(false);
        let paired = bool_value(device, "Paired").unwrap_or(false);
        let battery = interfaces
            .get(BATTERY)
            .and_then(|battery| u8_value(battery, "Percentage"));
        let rssi = i16_value(device, "RSSI");
        let adapter_id = map_value::<OwnedObjectPath>(device, "Adapter")
            .map(|path| path.to_string())
            .or_else(|| string_value(device, "Adapter"))
            .unwrap_or_default();
        devices.push(Device {
            id: id.clone(),
            name: string_value(device, "Alias")
                .or_else(|| string_value(device, "Name"))
                .or_else(|| string_value(device, "Address"))
                .unwrap_or_else(|| "Unknown device".to_string()),
            brand: String::new(),
            kind,
            description: kind.title().to_string(),
            status: if connected {
                DeviceStatus::Connected
            } else if paired {
                DeviceStatus::Saved
            } else {
                DeviceStatus::Nearby
            },
            battery,
            address: string_value(device, "Address").unwrap_or_default(),
            paired,
            trusted: bool_value(device, "Trusted").unwrap_or(false),
            rssi,
            adapter_id,
            connected_since: if connected {
                connected_since.get(&id).copied()
            } else {
                None
            },
        });
    }

    (adapters, devices)
}

#[derive(Clone, Debug)]
pub enum BluetoothCommand {
    Reconnect,
    SetPower(bool),
    SetDiscoverable(bool),
    SelectAdapter(String),
    StartDiscovery,
    StopDiscovery,
    Pair(String),
    CancelPairing(String),
    Connect(String),
    Disconnect(String),
    Remove(String),
    SetTrusted(String, bool),
    RespondPrompt(PromptReply),
    Shutdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptReply {
    Accept,
    Reject,
    Text(String),
}

struct Request {
    command: BluetoothCommand,
    reply: Option<oneshot::Sender<Result<(), String>>>,
}

#[derive(Clone)]
pub struct BluetoothService {
    commands: mpsc::UnboundedSender<Request>,
}

impl BluetoothService {
    fn send(&self, command: BluetoothCommand) -> Option<oneshot::Receiver<Result<(), String>>> {
        let (tx, rx) = oneshot::channel();
        if self
            .commands
            .send(Request {
                command,
                reply: Some(tx),
            })
            .is_err()
        {
            return None;
        }
        Some(rx)
    }

    pub async fn execute(&self, command: BluetoothCommand) -> Result<(), String> {
        let Some(receiver) = self.send(command) else {
            return Err("Bluetooth service is unavailable".into());
        };
        receiver
            .await
            .map_err(|_| "Bluetooth service is unavailable".to_string())?
    }

    pub fn execute_background(&self, command: BluetoothCommand) {
        let _ = self.commands.send(Request {
            command,
            reply: None,
        });
    }

    pub fn respond_prompt(&self, reply: PromptReply) {
        self.execute_background(BluetoothCommand::RespondPrompt(reply));
    }

    pub fn select_adapter(&self, id: String) {
        self.execute_background(BluetoothCommand::SelectAdapter(id));
    }

    pub async fn dispose(&self) {
        let Some(receiver) = self.send(BluetoothCommand::Shutdown) else {
            return;
        };
        let _ = receiver.await;
    }
}

#[derive(Clone, Debug)]
enum SignalEvent {
    InterfacesAdded {
        path: String,
        interfaces: HashMap<String, HashMap<String, OwnedValue>>,
    },
    InterfacesRemoved {
        path: String,
        interfaces: Vec<String>,
    },
    PropertiesChanged {
        path: String,
        interface: String,
        changed: HashMap<String, OwnedValue>,
        invalidated: Vec<String>,
    },
    NameOwnerChanged {
        new_owner: String,
    },
    BusFailed(String),
}

#[derive(Debug)]
struct AgentEvent {
    device_id: String,
    kind: PromptKind,
    passkey: Option<String>,
    entered: Option<u32>,
    reply: Option<oneshot::Sender<PromptReply>>,
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.bluez.Error")]
enum PairingError {
    Canceled(String),
    Rejected(String),
    Failed(String),
    #[zbus(error)]
    ZBus(zbus::Error),
}

fn pairing_error(error: impl std::fmt::Display) -> PairingError {
    PairingError::Failed(error.to_string())
}

struct Agent {
    events: mpsc::UnboundedSender<AgentEvent>,
}

impl Agent {
    async fn prompt(
        &self,
        device: OwnedObjectPath,
        kind: PromptKind,
        passkey: Option<String>,
        entered: Option<u32>,
    ) -> Result<PromptReply, PairingError> {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.events
            .send(AgentEvent {
                device_id: device.to_string(),
                kind,
                passkey,
                entered,
                reply: Some(reply_tx),
            })
            .map_err(pairing_error)?;
        reply_rx
            .await
            .map_err(|_| PairingError::Canceled("Pairing was canceled".into()))
    }
}

#[zbus::interface(name = "org.bluez.Agent1")]
impl Agent {
    async fn request_pin_code(&self, device: OwnedObjectPath) -> Result<String, PairingError> {
        match self.prompt(device, PromptKind::Pin, None, None).await? {
            PromptReply::Text(value) => Ok(value),
            _ => Err(PairingError::Rejected(
                "Pairing request was rejected".into(),
            )),
        }
    }

    async fn request_passkey(&self, device: OwnedObjectPath) -> Result<u32, PairingError> {
        match self.prompt(device, PromptKind::Passkey, None, None).await? {
            PromptReply::Text(value) => value
                .parse()
                .map_err(|_| PairingError::Failed("Enter a numeric passkey".into())),
            _ => Err(PairingError::Rejected(
                "Pairing request was rejected".into(),
            )),
        }
    }

    async fn display_pin_code(
        &self,
        device: OwnedObjectPath,
        pin: String,
    ) -> Result<(), PairingError> {
        self.events
            .send(AgentEvent {
                device_id: device.to_string(),
                kind: PromptKind::Display,
                passkey: Some(pin),
                entered: None,
                reply: None,
            })
            .map_err(pairing_error)
    }

    async fn display_passkey(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
        entered: u16,
    ) -> Result<(), PairingError> {
        self.events
            .send(AgentEvent {
                device_id: device.to_string(),
                kind: PromptKind::Display,
                passkey: Some(format!("{passkey:06}")),
                entered: Some(entered as u32),
                reply: None,
            })
            .map_err(pairing_error)
    }

    async fn request_confirmation(
        &self,
        device: OwnedObjectPath,
        passkey: u32,
    ) -> Result<(), PairingError> {
        match self
            .prompt(
                device,
                PromptKind::Confirm,
                Some(format!("{passkey:06}")),
                None,
            )
            .await?
        {
            PromptReply::Accept => Ok(()),
            _ => Err(PairingError::Rejected(
                "Pairing request was rejected".into(),
            )),
        }
    }

    async fn request_authorization(&self, device: OwnedObjectPath) -> Result<(), PairingError> {
        match self
            .prompt(device, PromptKind::Authorize, None, None)
            .await?
        {
            PromptReply::Accept => Ok(()),
            _ => Err(PairingError::Rejected(
                "Pairing request was rejected".into(),
            )),
        }
    }

    async fn authorize_service(
        &self,
        device: OwnedObjectPath,
        uuid: String,
    ) -> Result<(), PairingError> {
        match self
            .prompt(device, PromptKind::Authorize, Some(uuid), None)
            .await?
        {
            PromptReply::Accept => Ok(()),
            _ => Err(PairingError::Rejected(
                "Pairing request was rejected".into(),
            )),
        }
    }

    async fn cancel(&self) -> Result<(), PairingError> {
        let _ = self.events.send(AgentEvent {
            device_id: String::new(),
            kind: PromptKind::Display,
            passkey: None,
            entered: None,
            reply: None,
        });
        Ok(())
    }

    async fn release(&self) -> Result<(), PairingError> {
        let _ = self.events.send(AgentEvent {
            device_id: String::new(),
            kind: PromptKind::Display,
            passkey: None,
            entered: None,
            reply: None,
        });
        Ok(())
    }
}

struct Pairing {
    id: String,
    canceled: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    reply: Option<oneshot::Sender<Result<(), String>>>,
}

struct PairFinished {
    id: String,
    result: Result<(), String>,
}

struct Actor {
    connection: Option<Connection>,
    objects: ManagedObjects,
    since: HashMap<String, u64>,
    owned_discovery: HashSet<String>,
    snapshot: BluetoothSnapshot,
    events: mpsc::UnboundedSender<BluetoothSnapshot>,
    agent_events: mpsc::UnboundedSender<AgentEvent>,
    finished: mpsc::UnboundedSender<PairFinished>,
    current_reply: Option<oneshot::Sender<PromptReply>>,
    prompt_sequence: u64,
    pairing: Option<Pairing>,
    generation: u64,
}

impl Actor {
    fn publish(&mut self) {
        let (adapters, devices) = map_objects(&self.objects, &self.since);
        let adapter_id = if adapters
            .iter()
            .any(|adapter| Some(&adapter.id) == self.snapshot.adapter_id.as_ref())
        {
            self.snapshot.adapter_id.clone()
        } else {
            adapters
                .iter()
                .find(|adapter| adapter.powered)
                .or_else(|| adapters.first())
                .map(|adapter| adapter.id.clone())
        };
        let devices = devices
            .into_iter()
            .filter(|device| Some(&device.adapter_id) == adapter_id.as_ref())
            .collect();
        self.snapshot = BluetoothSnapshot {
            adapters,
            adapter_id,
            devices,
            loading: false,
            error: self.snapshot.error.clone(),
            prompt: self.snapshot.prompt.clone(),
            available: true,
        };
        let _ = self.events.send(self.snapshot.clone());
    }

    fn fail(&mut self, error: impl std::fmt::Display) {
        self.objects.clear();
        self.since.clear();
        self.owned_discovery.clear();
        self.cancel_prompt();
        self.snapshot = BluetoothSnapshot {
            adapters: Vec::new(),
            adapter_id: None,
            devices: Vec::new(),
            loading: false,
            error: Some(error.to_string()),
            prompt: None,
            available: false,
        };
        let _ = self.events.send(self.snapshot.clone());
    }

    fn cancel_prompt(&mut self) {
        self.current_reply = None;
        self.snapshot.prompt = None;
    }

    fn adapter(&self) -> Result<String, String> {
        self.snapshot
            .adapter_id
            .clone()
            .ok_or_else(|| "No Bluetooth adapter is available".to_string())
    }

    fn require_available(&self) -> Result<(), String> {
        if !self.snapshot.available {
            return Err("Bluetooth service is unavailable".into());
        }
        Ok(())
    }

    async fn proxy(&self, path: &str, interface: &str) -> Result<Proxy<'static>, String> {
        let connection = self
            .connection
            .as_ref()
            .ok_or_else(|| "Bluetooth system bus is unavailable".to_string())?;
        Proxy::new_owned(
            connection.clone(),
            BLUEZ.to_string(),
            path.to_string(),
            interface.to_string(),
        )
        .await
        .map_err(|error| error.to_string())
    }

    async fn set_property(
        &self,
        path: &str,
        interface: &str,
        property: &str,
        value: bool,
    ) -> Result<(), String> {
        self.proxy(path, interface)
            .await?
            .set_property(property, value)
            .await
            .map_err(|error| error.to_string())
    }

    async fn call(&self, path: &str, interface: &str, method: &str) -> Result<(), String> {
        self.proxy(path, interface)
            .await?
            .call::<_, _, ()>(method, &())
            .await
            .map_err(|error| error.to_string())
    }

    async fn refresh(&mut self) -> Result<(), String> {
        let Some(connection) = self.connection.as_ref() else {
            return Err("Bluetooth system bus is unavailable".into());
        };
        let proxy = Proxy::new_owned(
            connection.clone(),
            BLUEZ.to_string(),
            "/".to_string(),
            OBJECT_MANAGER.to_string(),
        )
        .await
        .map_err(|error| error.to_string())?;
        let objects = proxy
            .call::<_, _, ManagedObjects>("GetManagedObjects", &())
            .await
            .map_err(|error| error.to_string())?;
        self.objects = objects;
        self.snapshot.error = None;
        self.publish();
        Ok(())
    }

    async fn connect(&mut self, signals: mpsc::UnboundedSender<(u64, SignalEvent)>) {
        self.generation += 1;
        let generation = self.generation;
        let connection = match Connection::system().await {
            Ok(connection) => connection,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        let rule = match MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .build()
        {
            Ok(rule) => rule,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        let stream = MessageStream::for_match_rule(rule, &connection, None).await;
        let mut stream = match stream {
            Ok(stream) => stream,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        let task_tx = signals.clone();
        tokio::spawn(async move {
            while let Some(message) = stream.next().await {
                let Ok(message) = message else {
                    let _ = task_tx.send((
                        generation,
                        SignalEvent::BusFailed("Bluetooth system bus failed".into()),
                    ));
                    return;
                };
                if message.message_type() != zbus::message::Type::Signal {
                    continue;
                }
                let header = message.header();
                let interface = header.interface().map(|name| name.as_str().to_string());
                let member = header.member().map(|name| name.as_str().to_string());
                let path = header.path().map(|path| path.to_string());
                let Some(interface) = interface else {
                    continue;
                };
                let Some(member) = member else {
                    continue;
                };
                let signal = match (interface.as_str(), member.as_str()) {
                    (OBJECT_MANAGER, "InterfacesAdded") => message
                        .body()
                        .deserialize::<(
                            OwnedObjectPath,
                            HashMap<String, HashMap<String, OwnedValue>>,
                        )>()
                        .ok()
                        .map(|(path, interfaces)| SignalEvent::InterfacesAdded {
                            path: path.to_string(),
                            interfaces,
                        }),
                    (OBJECT_MANAGER, "InterfacesRemoved") => message
                        .body()
                        .deserialize::<(OwnedObjectPath, Vec<String>)>()
                        .ok()
                        .map(|(path, interfaces)| SignalEvent::InterfacesRemoved {
                            path: path.to_string(),
                            interfaces,
                        }),
                    (PROPERTIES, "PropertiesChanged") => {
                        let Some(path) = path else { continue };
                        message
                            .body()
                            .deserialize::<(String, HashMap<String, OwnedValue>, Vec<String>)>()
                            .ok()
                            .map(|(interface, changed, invalidated)| {
                                SignalEvent::PropertiesChanged {
                                    path,
                                    interface,
                                    changed,
                                    invalidated,
                                }
                            })
                    }
                    ("org.freedesktop.DBus", "NameOwnerChanged") => message
                        .body()
                        .deserialize::<(String, String, String)>()
                        .ok()
                        .filter(|(name, _, _)| name == BLUEZ)
                        .map(|(_, _, new_owner)| SignalEvent::NameOwnerChanged { new_owner }),
                    _ => None,
                };
                if let Some(signal) = signal {
                    let _ = task_tx.send((generation, signal));
                }
            }
            let _ = task_tx.send((
                generation,
                SignalEvent::BusFailed("Bluetooth system bus is unavailable".into()),
            ));
        });
        self.connection = Some(connection);
        self.objects.clear();
        self.since.clear();
        self.owned_discovery.clear();
        self.cancel_prompt();
        self.snapshot.loading = true;
        self.snapshot.available = false;
        self.snapshot.error = None;
        let _ = self.events.send(self.snapshot.clone());
        if let Err(error) = self.refresh().await {
            self.fail(error);
        }
    }

    async fn signal(&mut self, generation: u64, signal: SignalEvent) {
        if generation != self.generation {
            return;
        }
        match signal {
            SignalEvent::NameOwnerChanged { new_owner } => {
                if new_owner.is_empty() {
                    self.objects.clear();
                    self.since.clear();
                    self.owned_discovery.clear();
                    self.cancel_prompt();
                    self.snapshot = BluetoothSnapshot {
                        adapters: Vec::new(),
                        adapter_id: None,
                        devices: Vec::new(),
                        loading: false,
                        error: Some("Bluetooth service is unavailable".into()),
                        prompt: None,
                        available: false,
                    };
                    let _ = self.events.send(self.snapshot.clone());
                } else if !self.snapshot.available {
                    let _ = self.refresh().await;
                }
            }
            SignalEvent::BusFailed(error) => {
                self.fail(error);
            }
            SignalEvent::InterfacesAdded { path, interfaces } => {
                if !self.snapshot.available {
                    return;
                }
                if let Ok(path) = OwnedObjectPath::try_from(path) {
                    self.objects.entry(path).or_default().extend(interfaces);
                    self.publish();
                }
            }
            SignalEvent::InterfacesRemoved { path, interfaces } => {
                if !self.snapshot.available {
                    return;
                }
                if let Ok(object_path) = OwnedObjectPath::try_from(path.clone()) {
                    let remove_object = if let Some(object) = self.objects.get_mut(&object_path) {
                        for interface in &interfaces {
                            object.remove(interface);
                        }
                        object.is_empty()
                    } else {
                        false
                    };
                    if remove_object {
                        self.objects.remove(&object_path);
                    }
                }
                if interfaces.iter().any(|interface| interface == DEVICE) {
                    self.since.remove(&path);
                }
                self.publish();
            }
            SignalEvent::PropertiesChanged {
                path,
                interface,
                changed,
                invalidated,
            } => {
                if !self.snapshot.available {
                    return;
                }
                let Ok(object_path) = OwnedObjectPath::try_from(path.clone()) else {
                    return;
                };
                let Some(objects) = self.objects.get_mut(&object_path) else {
                    return;
                };
                let Some(properties) = objects.get_mut(&interface) else {
                    return;
                };
                if interface == ADAPTER
                    && ((changed.contains_key("Powered")
                        && !bool_value(&changed, "Powered").unwrap_or(false))
                        || (changed.contains_key("Discovering")
                            && !bool_value(&changed, "Discovering").unwrap_or(false)))
                {
                    self.owned_discovery.remove(&path);
                }
                if interface == DEVICE && changed.contains_key("Connected") {
                    let was = bool_value(properties, "Connected").unwrap_or(false);
                    let now = bool_value(&changed, "Connected").unwrap_or(false);
                    if !was && now {
                        self.since.insert(path.clone(), now_millis());
                    } else if !now {
                        self.since.remove(&path);
                    }
                }
                for (key, value) in changed {
                    properties.insert(key, value);
                }
                for key in &invalidated {
                    properties.remove(key);
                }
                let refresh = !invalidated.is_empty();
                self.publish();
                if refresh {
                    let _ = self.refresh().await;
                }
            }
        }
    }

    fn finish_action(&mut self, result: Result<(), String>) -> Result<(), String> {
        match result {
            Ok(()) => {
                self.snapshot.error = None;
                self.publish();
                Ok(())
            }
            Err(error) => {
                self.snapshot.error = Some(error.clone());
                self.publish();
                Err(error)
            }
        }
    }

    async fn handle(&mut self, command: BluetoothCommand) -> Result<(), String> {
        match command {
            BluetoothCommand::Reconnect | BluetoothCommand::Shutdown => Ok(()),
            BluetoothCommand::SetPower(value) => {
                self.require_available()?;
                let adapter = self.adapter()?;
                let result = self.set_property(&adapter, ADAPTER, "Powered", value).await;
                self.finish_action(result)
            }
            BluetoothCommand::SetDiscoverable(value) => {
                self.require_available()?;
                let adapter = self.adapter()?;
                let result = self
                    .set_property(&adapter, ADAPTER, "Discoverable", value)
                    .await;
                self.finish_action(result)
            }
            BluetoothCommand::SetTrusted(id, value) => {
                let result = self.set_property(&id, DEVICE, "Trusted", value).await;
                self.finish_action(result)
            }
            BluetoothCommand::SelectAdapter(id) => {
                if !self
                    .snapshot
                    .adapters
                    .iter()
                    .any(|adapter| adapter.id == id)
                {
                    return Ok(());
                }
                let old = self.snapshot.adapter_id.clone();
                if let Some(old) = old.filter(|old| old != &id)
                    && self.owned_discovery.contains(&old)
                {
                    let _ = self.call(&old, ADAPTER, "StopDiscovery").await;
                    self.owned_discovery.remove(&old);
                }
                self.snapshot.adapter_id = Some(id);
                self.publish();
                Ok(())
            }
            BluetoothCommand::StartDiscovery => {
                self.require_available()?;
                let adapter = self.adapter()?;
                if self.owned_discovery.contains(&adapter) {
                    return Ok(());
                }
                let result = self.call(&adapter, ADAPTER, "StartDiscovery").await;
                self.finish_action(result)?;
                self.owned_discovery.insert(adapter);
                Ok(())
            }
            BluetoothCommand::StopDiscovery => {
                self.require_available()?;
                let adapter = self.adapter()?;
                if !self.owned_discovery.contains(&adapter) {
                    return Ok(());
                }
                let result = self.call(&adapter, ADAPTER, "StopDiscovery").await;
                self.owned_discovery.remove(&adapter);
                if let Err(error) = &result {
                    self.snapshot.error = Some(error.clone());
                    self.publish();
                }
                result
            }
            BluetoothCommand::Connect(id) => {
                self.require_available()?;
                let result = self.call(&id, DEVICE, "Connect").await;
                self.finish_action(result)
            }
            BluetoothCommand::Disconnect(id) => {
                self.require_available()?;
                let result = self.call(&id, DEVICE, "Disconnect").await;
                self.finish_action(result)
            }
            BluetoothCommand::Remove(id) => {
                let object_path = OwnedObjectPath::try_from(id.clone())
                    .map_err(|_| "Bluetooth device is unavailable".to_string())?;
                let adapter = self
                    .objects
                    .get(&object_path)
                    .and_then(|interfaces| interfaces.get(DEVICE))
                    .and_then(|device| map_value::<OwnedObjectPath>(device, "Adapter"))
                    .map(|path| path.to_string())
                    .ok_or_else(|| "Bluetooth device is unavailable".to_string())?;
                self.require_available()?;
                let device_path = OwnedObjectPath::try_from(id.clone())
                    .map_err(|_| "Bluetooth device is unavailable".to_string())?;
                let result = async {
                    self.proxy(&adapter, ADAPTER)
                        .await?
                        .call::<_, _, ()>("RemoveDevice", &(device_path,))
                        .await
                        .map_err(|error| error.to_string())
                }
                .await;
                self.finish_action(result)
            }
            BluetoothCommand::Pair(_) => Err("A pairing request is already in progress".into()),
            BluetoothCommand::CancelPairing(id) => self.cancel_pairing(&id).await,
            BluetoothCommand::RespondPrompt(reply) => {
                self.respond_prompt(reply);
                Ok(())
            }
        }
    }

    async fn cancel_pairing(&mut self, id: &str) -> Result<(), String> {
        let mut started = true;
        if let Some(pairing) = self.pairing.as_ref()
            && pairing.id == id
        {
            pairing.canceled.store(true, Ordering::SeqCst);
            started = pairing.started.load(Ordering::SeqCst);
        }
        self.cancel_prompt();
        let _ = self.events.send(self.snapshot.clone());
        if started {
            self.require_available()?;
            match self.call(id, DEVICE, "CancelPairing").await {
                Ok(()) => {
                    self.snapshot.error = None;
                    self.publish();
                    Ok(())
                }
                Err(error) => {
                    self.snapshot.error = Some(error.clone());
                    self.publish();
                    Err(error)
                }
            }
        } else {
            Ok(())
        }
    }

    fn respond_prompt(&mut self, reply: PromptReply) {
        let Some(prompt) = self.snapshot.prompt.clone() else {
            return;
        };
        if prompt.kind == PromptKind::Display {
            self.snapshot.prompt = None;
            let _ = self.events.send(self.snapshot.clone());
            return;
        }
        if reply == PromptReply::Reject {
            if let Some(reply_tx) = self.current_reply.take() {
                let _ = reply_tx.send(PromptReply::Reject);
            }
            self.snapshot.prompt = None;
            let _ = self.events.send(self.snapshot.clone());
            return;
        }
        if let PromptReply::Text(text) = &reply {
            if prompt.kind == PromptKind::Pin && (text.is_empty() || text.len() > 16) {
                self.snapshot.error = Some("Enter a PIN of 1 to 16 characters".into());
                let _ = self.events.send(self.snapshot.clone());
                return;
            }
            if prompt.kind == PromptKind::Passkey
                && (text.is_empty()
                    || text.len() > 6
                    || !text.chars().all(|character| character.is_ascii_digit()))
            {
                self.snapshot.error = Some("Enter a numeric passkey of up to 6 digits".into());
                let _ = self.events.send(self.snapshot.clone());
                return;
            }
        }
        if let Some(reply_tx) = self.current_reply.take() {
            let _ = reply_tx.send(reply);
        }
        self.snapshot.prompt = None;
        self.snapshot.error = None;
        let _ = self.events.send(self.snapshot.clone());
    }

    fn prompt_name(&self, device_id: &str) -> String {
        self.snapshot
            .devices
            .iter()
            .find(|device| device.id == device_id)
            .map(|device| device.name.clone())
            .unwrap_or_else(|| "the device".to_string())
    }

    fn agent_event(&mut self, event: AgentEvent) {
        if event.device_id.is_empty() {
            self.cancel_prompt();
            let _ = self.events.send(self.snapshot.clone());
            return;
        }
        let name = self.prompt_name(&event.device_id);
        let message = match event.kind {
            PromptKind::Confirm => format!("Confirm that {name} shows the same passkey."),
            PromptKind::Pin => format!("Enter the PIN for {name}."),
            PromptKind::Passkey => format!("Enter the passkey for {name}."),
            PromptKind::Authorize => match &event.passkey {
                Some(service) => format!("Allow {name} to connect to {service}?"),
                None => format!("Allow {name} to connect?"),
            },
            PromptKind::Display => format!("Enter this code on {name}."),
        };
        self.prompt_sequence += 1;
        self.snapshot.prompt = Some(PairingPrompt {
            id: self.prompt_sequence,
            device_id: event.device_id,
            kind: event.kind,
            message,
            passkey: event.passkey,
            entered: event.entered,
        });
        self.current_reply = event.reply;
        let _ = self.events.send(self.snapshot.clone());
    }

    async fn shutdown(&mut self, finished_rx: &mut mpsc::UnboundedReceiver<PairFinished>) {
        if let Some(pairing) = self.pairing.as_ref() {
            pairing.canceled.store(true, Ordering::SeqCst);
        }
        self.cancel_prompt();
        if let Some(pairing) = self.pairing.take() {
            if pairing.started.load(Ordering::SeqCst) {
                let _ = self.call(&pairing.id, DEVICE, "CancelPairing").await;
            }
            let _ = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while let Some(finished) = finished_rx.recv().await {
                    if finished.id == pairing.id {
                        break;
                    }
                }
            })
            .await;
            if let Some(reply) = pairing.reply {
                let _ = reply.send(Err("Pairing was canceled".into()));
            }
        }
        let adapters: Vec<_> = self.owned_discovery.drain().collect();
        for adapter in adapters {
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                self.call(&adapter, ADAPTER, "StopDiscovery"),
            )
            .await;
        }
        self.connection = None;
    }
}

async fn run_pair(
    connection: Connection,
    id: String,
    agent_events: mpsc::UnboundedSender<AgentEvent>,
    canceled: Arc<AtomicBool>,
    started: Arc<AtomicBool>,
    finished: mpsc::UnboundedSender<PairFinished>,
) {
    let mut registered = false;
    let result = async {
        connection
            .object_server()
            .at(
                AGENT_PATH,
                Agent {
                    events: agent_events.clone(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
        let manager = Proxy::new(&connection, BLUEZ, "/org/bluez", "org.bluez.AgentManager1")
            .await
            .map_err(|error| error.to_string())?;
        let agent_path =
            OwnedObjectPath::try_from(AGENT_PATH).map_err(|error| error.to_string())?;
        manager
            .call::<_, _, ()>("RegisterAgent", &(agent_path.clone(), "KeyboardDisplay"))
            .await
            .map_err(|error| error.to_string())?;
        registered = true;
        if canceled.load(Ordering::SeqCst) {
            return Err("Pairing was canceled".to_string());
        }
        started.store(true, Ordering::SeqCst);
        let device = Proxy::new(&connection, BLUEZ, id.as_str(), DEVICE)
            .await
            .map_err(|error| error.to_string())?;
        device
            .call::<_, _, ()>("Pair", &())
            .await
            .map_err(|error| error.to_string())
    }
    .await;

    if registered
        && let Ok(manager) =
            Proxy::new(&connection, BLUEZ, "/org/bluez", "org.bluez.AgentManager1").await
        && let Ok(agent_path) = OwnedObjectPath::try_from(AGENT_PATH)
    {
        let _ = manager
            .call::<_, _, ()>("UnregisterAgent", &(agent_path,))
            .await;
    }
    let _ = connection
        .object_server()
        .remove::<Agent, _>(AGENT_PATH)
        .await;
    let _ = finished.send(PairFinished { id, result });
}

async fn bluetooth_loop(
    mut commands: mpsc::UnboundedReceiver<Request>,
    events: mpsc::UnboundedSender<BluetoothSnapshot>,
    mut agent_rx: mpsc::UnboundedReceiver<AgentEvent>,
    mut finished_rx: mpsc::UnboundedReceiver<PairFinished>,
    agent_events: mpsc::UnboundedSender<AgentEvent>,
    finished: mpsc::UnboundedSender<PairFinished>,
) {
    let (signal_tx, mut signal_rx) = mpsc::unbounded_channel();
    let mut actor = Actor {
        connection: None,
        objects: ManagedObjects::new(),
        since: HashMap::new(),
        owned_discovery: HashSet::new(),
        snapshot: BluetoothSnapshot::default(),
        events,
        agent_events,
        finished,
        current_reply: None,
        prompt_sequence: 0,
        pairing: None,
        generation: 0,
    };
    actor.connect(signal_tx.clone()).await;

    loop {
        tokio::select! {
            request = commands.recv() => {
                let Some(request) = request else { break };
                let reply = request.reply;
                let command = request.command;
                match command {
                    BluetoothCommand::Shutdown => {
                        actor.shutdown(&mut finished_rx).await;
                        if let Some(reply) = reply { let _ = reply.send(Ok(())); }
                        break;
                    }
                    BluetoothCommand::Reconnect => {
                        actor.connection = None;
                        actor.connect(signal_tx.clone()).await;
                        if let Some(reply) = reply { let _ = reply.send(Ok(())); }
                    }
                    BluetoothCommand::Pair(id) => {
                        if actor.pairing.is_some() {
                            if let Some(reply) = reply { let _ = reply.send(Err("A pairing request is already in progress".into())); }
                            continue;
                        }
                        if !actor.snapshot.available {
                            if let Some(reply) = reply { let _ = reply.send(Err("Bluetooth service is unavailable".into())); }
                            continue;
                        }
                        let Some(connection) = actor.connection.clone() else {
                            if let Some(reply) = reply { let _ = reply.send(Err("Bluetooth system bus is unavailable".into())); }
                            continue;
                        };
                        let canceled = Arc::new(AtomicBool::new(false));
                        let started = Arc::new(AtomicBool::new(false));
                        actor.pairing = Some(Pairing {
                            id: id.clone(),
                            canceled: canceled.clone(),
                            started: started.clone(),
                            reply,
                        });
                        tokio::spawn(run_pair(
                            connection,
                            id,
                            actor.agent_events.clone(),
                            canceled,
                            started,
                            actor.finished.clone(),
                        ));
                    }
                    command => {
                        let result = actor.handle(command).await;
                        if let Some(reply) = reply { let _ = reply.send(result); }
                    }
                }
            }
            signal = signal_rx.recv() => {
                if let Some((generation, signal)) = signal { actor.signal(generation, signal).await; }
            }
            event = agent_rx.recv() => {
                if let Some(event) = event { actor.agent_event(event); }
            }
            finished = finished_rx.recv() => {
                if let Some(finished) = finished
                    && actor.pairing.as_ref().is_some_and(|pairing| pairing.id == finished.id)
                {
                    actor.cancel_prompt();
                    if let Some(pairing) = actor.pairing.take()
                        && let Some(reply) = pairing.reply
                    {
                        let _ = reply.send(finished.result);
                    }
                    let _ = actor.events.send(actor.snapshot.clone());
                }
            }
        }
    }
}

impl BluetoothService {
    pub fn start() -> (Self, mpsc::UnboundedReceiver<BluetoothSnapshot>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (agent_tx, agent_rx) = mpsc::unbounded_channel();
        let (finished_tx, finished_rx) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("bluet-bluez".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("bluetooth runtime");
                runtime.block_on(bluetooth_loop(
                    command_rx,
                    event_tx,
                    agent_rx,
                    finished_rx,
                    agent_tx,
                    finished_tx,
                ));
            })
            .expect("bluetooth thread");
        (
            Self {
                commands: command_tx,
            },
            event_rx,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use zbus::Variant as Value;

    fn v(value: impl Into<Value<'static>>) -> OwnedValue {
        OwnedValue::try_from(value.into()).unwrap()
    }

    fn fixture() -> ManagedObjects {
        let mut objects = ManagedObjects::new();
        objects.insert(
            "/a".try_into().unwrap(),
            HashMap::from([(
                ADAPTER.to_string(),
                HashMap::from([
                    ("Alias".into(), v("Adapter")),
                    ("Address".into(), v("AA")),
                    ("Powered".into(), v(true)),
                    ("Discoverable".into(), v(false)),
                    ("Discovering".into(), v(false)),
                    ("Pairable".into(), v(true)),
                ]),
            )]),
        );
        objects.insert(
            "/b".try_into().unwrap(),
            HashMap::from([(
                ADAPTER.to_string(),
                HashMap::from([("Name".into(), v("Other")), ("Address".into(), v("BB"))]),
            )]),
        );
        objects.insert(
            "/a/d".try_into().unwrap(),
            HashMap::from([
                (
                    DEVICE.to_string(),
                    HashMap::from([
                        (
                            "Adapter".into(),
                            v(OwnedObjectPath::try_from("/a").unwrap()),
                        ),
                        ("Alias".into(), v("Mouse")),
                        ("Address".into(), v("12:34")),
                        ("Icon".into(), v("input-mouse")),
                        ("Paired".into(), v(true)),
                        ("Connected".into(), v(false)),
                        ("Trusted".into(), v(false)),
                        ("RSSI".into(), v(-50i16)),
                    ]),
                ),
                (
                    BATTERY.to_string(),
                    HashMap::from([("Percentage".into(), v(70u8))]),
                ),
            ]),
        );
        objects.insert(
            "/b/d".try_into().unwrap(),
            HashMap::from([(
                DEVICE.to_string(),
                HashMap::from([
                    (
                        "Adapter".into(),
                        v(OwnedObjectPath::try_from("/b").unwrap()),
                    ),
                    ("Name".into(), v("Headset")),
                    ("Icon".into(), v("audio-headset")),
                    ("Connected".into(), v(true)),
                ]),
            )]),
        );
        objects
    }

    #[test]
    fn maps_real_properties_and_preserves_unknown_connection_time() {
        let (adapters, devices) = map_objects(&fixture(), &HashMap::new());
        assert_eq!(adapters[0].id, "/a");
        assert!(adapters[0].powered);
        assert!(adapters[0].pairable);
        assert_eq!(devices[0].kind, DeviceKind::Mouse);
        assert_eq!(devices[0].status, DeviceStatus::Saved);
        assert!(devices[0].paired);
        assert_eq!(devices[0].battery, Some(70));
        assert_eq!(devices[0].rssi, Some(-50));
        assert_eq!(devices[1].connected_since, None);
    }

    #[test]
    fn maps_device_kinds_from_icons_and_bluetooth_classes() {
        assert_eq!(kind_for("audio-headset", 0), DeviceKind::Headphones);
        assert_eq!(kind_for("input-keyboard", 0), DeviceKind::Keyboard);
        assert_eq!(kind_for("", 0x0200), DeviceKind::Phone);
        assert_eq!(kind_for("", 0x0400 | (6 << 2)), DeviceKind::Headphones);
        assert_eq!(kind_for("", 0x0500 | (0x20 << 2)), DeviceKind::Mouse);
    }
}
