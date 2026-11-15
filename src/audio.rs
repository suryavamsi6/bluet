use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Map, Value};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{mpsc, oneshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioProfile {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioDevice {
    pub address: String,
    pub volume: i32,
    pub muted: bool,
    pub profiles: Vec<AudioProfile>,
    pub profile: Option<String>,
    pub sink_name: Option<String>,
    pub card_name: Option<String>,
    pub codec: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioSnapshot {
    pub available: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub devices: HashMap<String, AudioDevice>,
}

impl Default for AudioSnapshot {
    fn default() -> Self {
        Self {
            available: false,
            loading: true,
            error: None,
            devices: HashMap::new(),
        }
    }
}

pub type PactlFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
pub type PactlRunner = Arc<dyn Fn(Vec<String>) -> PactlFuture + Send + Sync>;

pub fn default_runner() -> PactlRunner {
    Arc::new(|args: Vec<String>| {
        Box::pin(async move {
            let output = tokio::time::timeout(
                Duration::from_secs(10),
                tokio::process::Command::new("pactl")
                    .args(&args)
                    .env("LC_ALL", "C")
                    .stdin(Stdio::null())
                    .output(),
            )
            .await
            .map_err(|_| "pactl timed out".to_string())?
            .map_err(|error| error.to_string())?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                return Err(if stderr.is_empty() {
                    format!("pactl exited with {}", output.status)
                } else {
                    stderr
                });
            }
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        })
    })
}

fn is_hex(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

fn valid_mac(candidate: &str) -> bool {
    let bytes = candidate.as_bytes();
    bytes.len() == 17
        && (0..6).all(|index| {
            let offset = index * 3;
            is_hex(bytes[offset])
                && is_hex(bytes[offset + 1])
                && (index == 5 || bytes[offset + 2] == b':')
        })
}

fn properties(entry: &Map<String, Value>) -> Option<&Map<String, Value>> {
    entry.get("properties").and_then(Value::as_object)
}

fn entry_name(entry: &Map<String, Value>) -> Option<&str> {
    entry.get("name").and_then(Value::as_str)
}

fn is_bluetooth(entry: &Map<String, Value>) -> bool {
    let api_is_bluez = properties(entry)
        .and_then(|properties| properties.get("device.api"))
        .and_then(Value::as_str)
        == Some("bluez5");
    let name_is_bluez = entry_name(entry).is_some_and(|name| {
        let lower = name.to_ascii_lowercase();
        lower.starts_with("bluez_card.") || lower.starts_with("bluez_output.")
    });
    api_is_bluez || name_is_bluez
}

fn address_of(entry: &Map<String, Value>) -> Option<String> {
    if let Some(properties) = properties(entry) {
        let candidate = properties
            .get("api.bluez5.address")
            .or_else(|| properties.get("device.string"))
            .and_then(Value::as_str);
        if let Some(candidate) = candidate
            && valid_mac(candidate)
        {
            return Some(candidate.to_ascii_uppercase());
        }
    }
    let name = entry_name(entry)?.to_ascii_lowercase();
    let rest = name
        .strip_prefix("bluez_card.")
        .or_else(|| name.strip_prefix("bluez_output."))?;
    let part = rest.split('.').next().unwrap_or_default();
    let address = part.replace('_', ":");
    valid_mac(&address).then(|| address.to_ascii_uppercase())
}

fn empty_device(address: String) -> AudioDevice {
    AudioDevice {
        address,
        volume: 0,
        muted: false,
        profiles: Vec::new(),
        profile: None,
        sink_name: None,
        card_name: None,
        codec: None,
    }
}

pub fn parse_audio_devices(
    cards: &Value,
    sinks: &Value,
) -> Result<HashMap<String, AudioDevice>, String> {
    let cards = cards.as_array().ok_or("Unexpected pactl response")?;
    let sinks = sinks.as_array().ok_or("Unexpected pactl response")?;
    let mut result = HashMap::new();

    for card in cards {
        let Some(card) = card.as_object() else {
            continue;
        };
        if !is_bluetooth(card) {
            continue;
        }
        let Some(address) = address_of(card) else {
            continue;
        };
        let profile = card.get("active_profile").and_then(|profile| {
            profile.as_str().map(str::to_string).or_else(|| {
                profile
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        });
        let mut device = empty_device(address.clone());
        device.card_name = entry_name(card).map(str::to_string);
        device.profile = profile;
        if let Some(profiles) = card.get("profiles").and_then(Value::as_object) {
            for (id, profile) in profiles {
                let available = match profile.get("available") {
                    Some(Value::Bool(value)) => *value,
                    Some(Value::String(value)) => value != "no",
                    _ => true,
                };
                let sinks = profile.get("sinks").and_then(Value::as_u64).unwrap_or(0);
                if available && sinks > 0 {
                    device.profiles.push(AudioProfile {
                        id: id.clone(),
                        label: profile
                            .get("description")
                            .and_then(Value::as_str)
                            .unwrap_or(id)
                            .to_string(),
                    });
                }
            }
        }
        result.insert(address, device);
    }

    for sink in sinks {
        let Some(sink) = sink.as_object() else {
            continue;
        };
        if !is_bluetooth(sink) {
            continue;
        }
        let Some(address) = address_of(sink) else {
            continue;
        };
        let mut levels = Vec::new();
        if let Some(volume) = sink.get("volume").and_then(Value::as_object) {
            for channel in volume.values() {
                if let Some(value) = channel.get("value").and_then(Value::as_f64) {
                    levels.push(value / 65536.0 * 100.0);
                } else if let Some(percent) = channel.get("value_percent").and_then(Value::as_str)
                    && let Ok(value) = percent.trim_end_matches('%').trim().parse::<f64>()
                    && value.is_finite()
                {
                    levels.push(value);
                }
            }
        }
        let volume = if levels.is_empty() {
            0
        } else {
            (levels.iter().sum::<f64>() / levels.len() as f64).round() as i32
        };
        let codec = properties(sink)
            .and_then(|properties| properties.get("api.bluez5.codec"))
            .and_then(Value::as_str)
            .map(str::to_string);
        let device = result
            .entry(address.clone())
            .or_insert_with(|| empty_device(address));
        device.sink_name = entry_name(sink).map(str::to_string);
        device.muted = sink.get("mute").and_then(Value::as_bool).unwrap_or(false);
        device.volume = volume;
        device.codec = codec;
    }

    Ok(result)
}

pub enum AudioCommand {
    Refresh,
    SetVolume { address: String, value: f64 },
    SetMuted { address: String, muted: bool },
    SetProfile { address: String, profile: String },
    Shutdown,
}

struct Request {
    command: AudioCommand,
    reply: Option<oneshot::Sender<Result<(), String>>>,
}

#[derive(Clone)]
pub struct AudioService {
    commands: mpsc::UnboundedSender<Request>,
}

impl AudioService {
    fn send(&self, command: AudioCommand, reply: Option<oneshot::Sender<Result<(), String>>>) {
        let _ = self.commands.send(Request { command, reply });
    }

    pub async fn refresh(&self) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.send(AudioCommand::Refresh, Some(tx));
        rx.await.map_err(|_| "Audio service stopped".to_string())?
    }

    pub async fn set_volume(&self, address: &str, value: f64) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.send(
            AudioCommand::SetVolume {
                address: address.to_string(),
                value,
            },
            Some(tx),
        );
        rx.await.map_err(|_| "Audio service stopped".to_string())?
    }

    pub async fn set_muted(&self, address: &str, muted: bool) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.send(
            AudioCommand::SetMuted {
                address: address.to_string(),
                muted,
            },
            Some(tx),
        );
        rx.await.map_err(|_| "Audio service stopped".to_string())?
    }

    pub async fn set_profile(&self, address: &str, profile: &str) -> Result<(), String> {
        let (tx, rx) = oneshot::channel();
        self.send(
            AudioCommand::SetProfile {
                address: address.to_string(),
                profile: profile.to_string(),
            },
            Some(tx),
        );
        rx.await.map_err(|_| "Audio service stopped".to_string())?
    }

    pub async fn dispose(&self) {
        let (tx, rx) = oneshot::channel();
        self.send(AudioCommand::Shutdown, Some(tx));
        let _ = rx.await;
    }
}

struct AudioActor {
    snapshot: AudioSnapshot,
    runner: PactlRunner,
    events: mpsc::UnboundedSender<AudioSnapshot>,
    monitor: Option<tokio::task::JoinHandle<()>>,
    debounce: Option<tokio::time::Instant>,
}

impl AudioActor {
    fn publish(&self) {
        let _ = self.events.send(self.snapshot.clone());
    }

    async fn refresh(&mut self) {
        let runner = self.runner.clone();
        let cards_future = runner(vec!["--format=json".into(), "list".into(), "cards".into()]);
        let sinks_future = runner(vec!["--format=json".into(), "list".into(), "sinks".into()]);
        let (cards, sinks) = tokio::join!(cards_future, sinks_future);
        match (cards, sinks) {
            (Ok(cards), Ok(sinks)) => match (
                serde_json::from_str::<Value>(&cards),
                serde_json::from_str::<Value>(&sinks),
            ) {
                (Ok(cards), Ok(sinks)) => match parse_audio_devices(&cards, &sinks) {
                    Ok(devices) => {
                        self.snapshot = AudioSnapshot {
                            available: true,
                            loading: false,
                            error: None,
                            devices,
                        };
                    }
                    Err(error) => {
                        self.snapshot = AudioSnapshot {
                            available: false,
                            loading: false,
                            error: Some(format!("Audio service unavailable: {error}")),
                            devices: HashMap::new(),
                        };
                    }
                },
                (cards, sinks) => {
                    let error = cards
                        .err()
                        .map(|error| error.to_string())
                        .or_else(|| sinks.err().map(|error| error.to_string()))
                        .unwrap_or_default();
                    self.snapshot = AudioSnapshot {
                        available: false,
                        loading: false,
                        error: Some(format!("Audio service unavailable: {error}")),
                        devices: HashMap::new(),
                    };
                }
            },
            (cards, sinks) => {
                let error = cards.err().or_else(|| sinks.err()).unwrap_or_default();
                self.snapshot = AudioSnapshot {
                    available: false,
                    loading: false,
                    error: Some(format!("Audio service unavailable: {error}")),
                    devices: HashMap::new(),
                };
            }
        }
        self.publish();
    }

    fn device(&self, address: &str) -> Result<AudioDevice, String> {
        if !self.snapshot.available {
            return Err("No audio device is available for these headphones.".into());
        }
        self.snapshot
            .devices
            .get(&address.to_ascii_uppercase())
            .cloned()
            .ok_or_else(|| "No audio device is available for these headphones.".into())
    }

    async fn run(&mut self, args: &[&str]) -> Result<(), String> {
        (self.runner)(args.iter().map(|arg| arg.to_string()).collect())
            .await
            .map(|_| ())
    }

    async fn handle(&mut self, command: AudioCommand) -> Result<(), String> {
        match command {
            AudioCommand::Refresh => {
                self.refresh().await;
                Ok(())
            }
            AudioCommand::SetVolume { address, value } => {
                if !value.is_finite() {
                    return Err("Volume must be a number.".into());
                }
                let device = self.device(&address)?;
                let sink = device
                    .sink_name
                    .ok_or("No audio output is available for this device.")?;
                let value = value.round().clamp(0.0, 100.0) as i32;
                self.run(&["set-sink-volume", &sink, &format!("{value}%")])
                    .await?;
                self.refresh().await;
                Ok(())
            }
            AudioCommand::SetMuted { address, muted } => {
                let device = self.device(&address)?;
                let sink = device
                    .sink_name
                    .ok_or("No audio output is available for this device.")?;
                self.run(&["set-sink-mute", &sink, if muted { "1" } else { "0" }])
                    .await?;
                self.refresh().await;
                Ok(())
            }
            AudioCommand::SetProfile { address, profile } => {
                let device = self.device(&address)?;
                let Some(card) = device.card_name else {
                    return Err("This audio profile is not available.".into());
                };
                if !device
                    .profiles
                    .iter()
                    .any(|candidate| candidate.id == profile)
                {
                    return Err("This audio profile is not available.".into());
                }
                self.run(&["set-card-profile", &card, &profile]).await?;
                self.refresh().await;
                Ok(())
            }
            AudioCommand::Shutdown => Ok(()),
        }
    }
}

fn spawn_monitor(events: mpsc::UnboundedSender<MonitorEvent>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            let child = tokio::process::Command::new("pactl")
                .arg("subscribe")
                .env("LC_ALL", "C")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn();
            let Ok(mut child) = child else {
                let _ = events.send(MonitorEvent::Closed);
                tokio::time::sleep(Duration::from_millis(2500)).await;
                continue;
            };
            if let Some(stdout) = child.stdout.take() {
                let mut lines = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let lower = line.to_ascii_lowercase();
                    if lower.contains("sink") || lower.contains("card") || lower.contains("server")
                    {
                        let _ = events.send(MonitorEvent::Changed);
                    }
                }
            }
            let _ = child.wait().await;
            let _ = events.send(MonitorEvent::Closed);
            tokio::time::sleep(Duration::from_millis(2500)).await;
        }
    })
}

enum MonitorEvent {
    Changed,
    Closed,
}

async fn audio_loop(
    mut commands: mpsc::UnboundedReceiver<Request>,
    events: mpsc::UnboundedSender<AudioSnapshot>,
    mut monitor_events: mpsc::UnboundedReceiver<MonitorEvent>,
    runner: PactlRunner,
    monitor_enabled: bool,
) {
    let mut actor = AudioActor {
        snapshot: AudioSnapshot::default(),
        runner,
        events,
        monitor: None,
        debounce: None,
    };
    actor.publish();
    actor.refresh().await;
    if monitor_enabled {
        let (tx, rx) = mpsc::unbounded_channel();
        actor.monitor = Some(spawn_monitor(tx));
        monitor_events = rx;
    }

    loop {
        let deadline = actor.debounce;
        tokio::select! {
            request = commands.recv() => {
                let Some(request) = request else { break };
                if matches!(request.command, AudioCommand::Shutdown) {
                    if let Some(monitor) = actor.monitor.take() { monitor.abort(); }
                    if let Some(reply) = request.reply { let _ = reply.send(Ok(())); }
                    break;
                }
                let result = actor.handle(request.command).await;
                if let Some(reply) = request.reply { let _ = reply.send(result); }
            }
            event = monitor_events.recv(), if monitor_enabled => {
                match event {
                    Some(MonitorEvent::Changed) => {
                        actor.debounce = Some(tokio::time::Instant::now() + Duration::from_millis(120));
                    }
                    Some(MonitorEvent::Closed) => {
                        actor.debounce = None;
                        actor.refresh().await;
                    }
                    None => {}
                }
            }
            _ = async {
                match deadline {
                    Some(deadline) => tokio::time::sleep_until(deadline).await,
                    None => std::future::pending::<()>().await,
                }
            } => {
                actor.debounce = None;
                actor.refresh().await;
            }
        }
    }
    if let Some(monitor) = actor.monitor.take() {
        monitor.abort();
    }
}

impl AudioService {
    pub fn start_default() -> (Self, mpsc::UnboundedReceiver<AudioSnapshot>) {
        Self::start(default_runner(), true)
    }

    pub fn start(
        runner: PactlRunner,
        monitor_enabled: bool,
    ) -> (Self, mpsc::UnboundedReceiver<AudioSnapshot>) {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let (monitor_tx, monitor_rx) = mpsc::unbounded_channel();
        if !monitor_enabled {
            drop(monitor_tx);
        }
        std::thread::Builder::new()
            .name("bluet-audio".into())
            .spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("audio runtime");
                runtime.block_on(audio_loop(
                    command_rx,
                    event_tx,
                    monitor_rx,
                    runner,
                    monitor_enabled,
                ));
            })
            .expect("audio thread");
        (
            Self {
                commands: command_tx,
            },
            event_rx,
        )
    }
}

/// Pure helper used by the UI to display compact profile names.
pub fn short_profile_label(label: &str) -> String {
    let lower = label.to_ascii_lowercase();
    let codec = lower.find("codec").and_then(|index| {
        let rest = &label[index + "codec".len()..];
        let rest = rest.trim_start();
        let end = rest.find([',', ')']).unwrap_or(rest.len());
        let codec = rest[..end].trim();
        (!codec.is_empty()).then(|| codec.to_string())
    });
    let kind = if lower.contains("high fidelity") || lower.contains("a2dp") {
        "High fidelity".to_string()
    } else if lower.contains("headset")
        || lower.contains("hands.free")
        || lower.contains("hfp")
        || lower.contains("hsp")
    {
        "Headset".to_string()
    } else {
        label.to_string()
    };
    match codec {
        Some(codec) => format!("{kind} · {codec}"),
        None => kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ADDRESS: &str = "80:C3:BA:51:31:A6";
    const CARD: &str = "bluez_card.80_C3_BA_51_31_A6";
    const SINK: &str = "bluez_output.80_C3_BA_51_31_A6.1";

    fn cards() -> Value {
        json!([{
            "name": CARD,
            "properties": { "api.bluez5.address": ADDRESS, "device.api": "bluez5" },
            "active_profile": "a2dp-sink",
            "profiles": {
                "a2dp-sink": { "description": "High Fidelity (aptX HD)", "sinks": 1, "available": true },
                "headset-head-unit": { "description": "Headset", "sinks": 1, "available": true },
                "unavailable": { "sinks": 1, "available": false },
                "off": { "sinks": 0, "available": true }
            }
        }, { "name": "alsa_card.unrelated" }])
    }

    fn sinks() -> Value {
        json!([{
            "name": SINK,
            "properties": { "api.bluez5.address": ADDRESS.to_lowercase(), "api.bluez5.codec": "aptx_hd" },
            "mute": false,
            "volume": { "left": { "value": 32768 }, "right": { "value": 32768 } }
        }])
    }

    #[test]
    fn maps_only_bluetooth_by_address_and_exposes_profiles_and_codec() {
        let devices = parse_audio_devices(&cards(), &sinks()).unwrap();
        assert_eq!(devices.len(), 1);
        let device = &devices[ADDRESS];
        assert_eq!(device.volume, 50);
        assert!(!device.muted);
        assert_eq!(device.codec.as_deref(), Some("aptx_hd"));
        assert_eq!(device.profile.as_deref(), Some("a2dp-sink"));
        assert_eq!(device.sink_name.as_deref(), Some(SINK));
        assert_eq!(device.card_name.as_deref(), Some(CARD));
        assert_eq!(
            device
                .profiles
                .iter()
                .map(|p| p.id.as_str())
                .collect::<Vec<_>>(),
            ["a2dp-sink", "headset-head-unit"]
        );
        let percent = parse_audio_devices(
            &json!([]),
            &json!([{ "name": SINK, "volume": { "mono": { "value_percent": "42%" } } }]),
        )
        .unwrap();
        assert_eq!(percent[ADDRESS].volume, 42);
    }

    #[test]
    fn compacts_profile_labels() {
        assert_eq!(
            short_profile_label("High Fidelity Playback (A2DP Sink, codec aptX HD)"),
            "High fidelity · aptX HD"
        );
        assert_eq!(
            short_profile_label("Headset Head Unit (HSP/HFP)"),
            "Headset"
        );
    }

    #[test]
    fn service_reads_then_targets_selected_device() {
        use std::sync::Mutex;
        let calls = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let cards_state = cards();
        let sinks_state = sinks();
        let calls_for_runner = calls.clone();
        let runner: PactlRunner = Arc::new(move |args| {
            calls_for_runner.lock().unwrap().push(args.clone());
            let cards_state = cards_state.clone();
            let sinks_state = sinks_state.clone();
            Box::pin(async move {
                if args.first().map(String::as_str) == Some("--format=json") {
                    Ok((if args[2] == "cards" {
                        cards_state
                    } else {
                        sinks_state
                    })
                    .to_string())
                } else {
                    Ok(String::new())
                }
            })
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let (service, mut events) = AudioService::start(runner, false);
            let _first = events.recv().await.unwrap();
            let snapshot = events.recv().await.unwrap();
            assert!(snapshot.available);
            service.set_volume(ADDRESS, 74.0).await.unwrap();
            service.set_muted(ADDRESS, true).await.unwrap();
            service
                .set_profile(ADDRESS, "headset-head-unit")
                .await
                .unwrap();
            assert_eq!(
                calls
                    .lock()
                    .unwrap()
                    .iter()
                    .filter(|call| call[0].starts_with("set-"))
                    .cloned()
                    .collect::<Vec<_>>(),
                vec![
                    vec!["set-sink-volume", SINK, "74%"],
                    vec!["set-sink-mute", SINK, "1"],
                    vec!["set-card-profile", CARD, "headset-head-unit"],
                ]
            );
            assert!(service.set_profile(ADDRESS, "unavailable").await.is_err());
            assert!(service.set_volume("00:00:00:00:00:00", 30.0).await.is_err());
            assert!(service.set_volume(ADDRESS, f64::NAN).await.is_err());
            service.dispose().await;
        });
    }

    #[test]
    fn reports_failures_without_manufacturing_data() {
        let runner: PactlRunner =
            Arc::new(|_| Box::pin(async { Err("Connection refused".to_string()) }));
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let (service, mut events) = AudioService::start(runner, false);
            let _first = events.recv().await.unwrap();
            let snapshot = events.recv().await.unwrap();
            assert!(!snapshot.available);
            assert!(!snapshot.loading);
            assert!(snapshot.devices.is_empty());
            assert!(snapshot.error.unwrap().contains("Connection refused"));
            service.dispose().await;
        });
    }

    #[test]
    fn recovers_after_the_audio_service_returns() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let failing = Arc::new(AtomicBool::new(true));
        let failing_for_runner = failing.clone();
        let runner: PactlRunner = Arc::new(move |args| {
            let failing = failing_for_runner.clone();
            let cards = cards();
            let sinks = sinks();
            Box::pin(async move {
                if failing.load(Ordering::SeqCst) {
                    return Err("Connection refused".to_string());
                }
                Ok((if args[2] == "cards" { cards } else { sinks }).to_string())
            })
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let (service, mut events) = AudioService::start(runner, false);
            let _first = events.recv().await.unwrap();
            let failed = events.recv().await.unwrap();
            assert!(!failed.available);
            failing.store(false, Ordering::SeqCst);
            service.refresh().await.unwrap();
            let recovered = events.recv().await.unwrap();
            assert!(recovered.available);
            assert_eq!(recovered.devices[ADDRESS].volume, 50);
            service.dispose().await;
        });
    }

    #[test]
    fn write_failures_surface_without_optimistic_changes() {
        let runner: PactlRunner = Arc::new(|args| {
            let cards = cards();
            let sinks = sinks();
            Box::pin(async move {
                if args[0] != "--format=json" {
                    return Err("Permission denied".to_string());
                }
                Ok((if args[2] == "cards" { cards } else { sinks }).to_string())
            })
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let (service, mut events) = AudioService::start(runner, false);
            let _first = events.recv().await.unwrap();
            let snapshot = events.recv().await.unwrap();
            assert!(snapshot.available);
            let error = service.set_volume(ADDRESS, 90.0).await.unwrap_err();
            assert!(error.contains("Permission denied"));
            // A failed write must not publish an optimistic snapshot, so the
            // stored volume stays at the last known-good value.
            assert!(
                events.try_recv().is_err(),
                "a failed write must not publish an optimistic snapshot"
            );
            service.dispose().await;
        });
    }
}
