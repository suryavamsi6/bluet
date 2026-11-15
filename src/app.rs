use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};
use gpui::{
    AnyElement, Bounds, ClickEvent, Context, CursorStyle, Div, Entity, FocusHandle, Focusable,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Render, SharedString, Window, WindowControlArea, div, linear_color_stop,
    linear_gradient, point, prelude::*, px, relative, size,
};

use crate::audio::{AudioService, AudioSnapshot, short_profile_label};
use crate::bluez::{
    BluetoothCommand, BluetoothService, BluetoothSnapshot, PromptKind, PromptReply,
};
use crate::dither::DitherVariant;
use crate::icons::Icon;
use crate::input::{InputEvent, InputFont, InputKind, TextInput};
use crate::model::{Device, DeviceFilter, DeviceKind, DeviceStatus, filter_devices};
use crate::theme::C;
use crate::widgets::{
    ButtonOptions, button, dither_layer, dither_meter, divider, empty_state, icon, icon_button,
    info_row, m, preference, radar, search_field, shadow, t, text_button, toggle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Page {
    Devices,
    FileTransfers,
    Activity,
    Settings,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Self::Devices => "Your devices",
            Self::FileTransfers => "File transfers",
            Self::Activity => "Activity",
            Self::Settings => "Settings",
        }
    }

    fn subtitle(self) -> &'static str {
        match self {
            Self::Devices => "Manage your connections and discover nearby devices.",
            Self::FileTransfers => "Bluetooth file transfers.",
            Self::Activity => "Bluetooth changes observed during this session.",
            Self::Settings => "Manage your system Bluetooth adapter.",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Devices => "Devices",
            Self::FileTransfers => "File transfers",
            Self::Activity => "Activity",
            Self::Settings => "Settings",
        }
    }

    fn icon(self) -> Icon {
        match self {
            Self::Devices => Icon::Bluetooth,
            Self::FileTransfers => Icon::ArrowLeftRight,
            Self::Activity => Icon::Clock3,
            Self::Settings => Icon::Settings2,
        }
    }

    fn test_id(self) -> &'static str {
        match self {
            Self::Devices => "nav-devices",
            Self::FileTransfers => "nav-file-transfers",
            Self::Activity => "nav-activity",
            Self::Settings => "nav-settings",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Dialog {
    Pair,
    Forget,
}

#[derive(Clone, Debug)]
enum DropdownKind {
    AudioProfile { address: String },
    Adapter,
}

#[derive(Clone, Debug)]
struct Dropdown {
    kind: DropdownKind,
    top: f32,
    left: f32,
    width: f32,
    trigger: Point<Pixels>,
}

#[derive(Clone, Debug)]
struct ActivityEntry {
    message: String,
    time: u64,
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

fn kind_icon(kind: DeviceKind) -> Icon {
    match kind {
        DeviceKind::Headphones => Icon::Headphones,
        DeviceKind::Mouse => Icon::Mouse,
        DeviceKind::Keyboard => Icon::Keyboard,
        DeviceKind::Phone => Icon::Smartphone,
        DeviceKind::Speaker => Icon::Speaker,
        DeviceKind::Gamepad => Icon::Gamepad2,
        DeviceKind::Device => Icon::Bluetooth,
    }
}

fn status_label(device: &Device) -> &'static str {
    if device.status == DeviceStatus::Connected {
        "Connected"
    } else if device.paired {
        "Paired"
    } else {
        "Not paired"
    }
}

pub struct BluetApp {
    bluetooth: BluetoothService,
    audio: AudioService,
    bt: BluetoothSnapshot,
    sound: AudioSnapshot,
    previous: Option<BluetoothSnapshot>,
    page: Page,
    selected: Option<String>,
    filter: DeviceFilter,
    query: String,
    pair_query: String,
    dialog: Option<Dialog>,
    pair_id: Option<String>,
    menu: bool,
    menu_anchor: Point<Pixels>,
    menu_trigger: Point<Pixels>,
    dropdown: Option<Dropdown>,
    pending: Option<String>,
    error: Option<String>,
    toast: String,
    toast_generation: u64,
    activity: Vec<ActivityEntry>,
    draft_volume: Option<(String, i32)>,
    dragging_volume: bool,
    window_width: f32,
    window_height: f32,
    search: Entity<TextInput>,
    pair_search: Entity<TextInput>,
    code_input: Entity<TextInput>,
    focus_prompt: bool,
    focus_root: bool,
    last_prompt_id: Option<u64>,
    root_focus: FocusHandle,
    shutting_down: bool,
}

impl BluetApp {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let (bluetooth, mut bt_events) = BluetoothService::start();
        let (audio, mut sound_events) = AudioService::start_default();
        let (input_tx, input_events) = flume::unbounded();

        let search = cx.new(|cx| {
            TextInput::new(
                cx,
                "Search devices…",
                InputFont::Inter,
                13.0,
                30.0,
                InputKind::Query,
                input_tx.clone(),
            )
        });
        let pair_search = cx.new(|cx| {
            TextInput::new(
                cx,
                "Search devices…",
                InputFont::Inter,
                13.0,
                30.0,
                InputKind::PairQuery,
                input_tx.clone(),
            )
        });
        let code_input = cx.new(|cx| {
            TextInput::new(
                cx,
                "Enter PIN code",
                InputFont::Mono,
                18.0,
                24.0,
                InputKind::PromptCode,
                input_tx.clone(),
            )
        });

        let entity = Self {
            bluetooth,
            audio,
            bt: BluetoothSnapshot::default(),
            sound: AudioSnapshot::default(),
            previous: None,
            page: Page::Devices,
            selected: None,
            filter: DeviceFilter::All,
            query: String::new(),
            pair_query: String::new(),
            dialog: None,
            pair_id: None,
            menu: false,
            menu_anchor: point(px(0.0), px(0.0)),
            menu_trigger: point(px(0.0), px(0.0)),
            dropdown: None,
            pending: None,
            error: None,
            toast: String::new(),
            toast_generation: 0,
            activity: Vec::new(),
            draft_volume: None,
            dragging_volume: false,
            window_width: 1440.0,
            window_height: 1000.0,
            search,
            pair_search,
            code_input,
            focus_prompt: false,
            focus_root: true,
            last_prompt_id: None,
            root_focus: cx.focus_handle(),
            shutting_down: false,
        };

        cx.spawn(async move |this, cx| {
            while let Some(snapshot) = bt_events.recv().await {
                if this
                    .update(cx, |this, cx| this.apply_bluetooth(snapshot, cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            while let Some(snapshot) = sound_events.recv().await {
                if this
                    .update(cx, |this, cx| {
                        this.sound = snapshot;
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        let (shutdown_tx, shutdown_rx) = flume::bounded(1);
        if let Ok(mut signals) = signal_hook::iterator::Signals::new([
            signal_hook::consts::SIGTERM,
            signal_hook::consts::SIGINT,
        ]) {
            std::thread::Builder::new()
                .name("bluet-signals".into())
                .spawn(move || {
                    for _ in signals.forever() {
                        let _ = shutdown_tx.send(());
                    }
                })
                .ok();
        }
        cx.spawn(async move |this, cx| {
            while let Ok(event) = input_events.recv_async().await {
                let InputEvent::Changed(kind, text) = event;
                if this
                    .update(cx, |this, cx| {
                        match kind {
                            InputKind::Query => this.query = text.to_string(),
                            InputKind::PairQuery => this.pair_query = text.to_string(),
                            InputKind::PromptCode => {}
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            if shutdown_rx.recv_async().await.is_ok() {
                this.update(cx, |this, cx| this.begin_shutdown(cx)).ok();
            }
        })
        .detach();

        entity
    }

    pub fn install_shutdown(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let entity = cx.entity();
        window.on_window_should_close(cx, move |_window, cx| {
            entity.update(cx, |app, cx| app.begin_shutdown(cx));
            false
        });
    }

    fn begin_shutdown(&mut self, cx: &mut Context<Self>) {
        if self.shutting_down {
            return;
        }
        self.shutting_down = true;
        let bluetooth = self.bluetooth.clone();
        let audio = self.audio.clone();
        cx.spawn(async move |this, cx| {
            let _ = futures_util::future::join(audio.dispose(), bluetooth.dispose()).await;
            this.update(cx, |_, cx| cx.quit()).ok();
        })
        .detach();
    }

    fn apply_bluetooth(&mut self, snapshot: BluetoothSnapshot, cx: &mut Context<Self>) {
        let mut new_activity = Vec::new();
        if let Some(previous) = &self.previous
            && !snapshot.loading
        {
            for device in &snapshot.devices {
                if let Some(before) = previous
                    .devices
                    .iter()
                    .find(|before| before.id == device.id)
                    && before.status != device.status
                {
                    let action = match device.status {
                        DeviceStatus::Connected => "connected",
                        DeviceStatus::Saved => "disconnected",
                        DeviceStatus::Nearby => "discovered",
                    };
                    new_activity.push(format!("{} {action}", device.name));
                }
            }
            if previous.available != snapshot.available {
                new_activity.push(if snapshot.available {
                    "Bluetooth service available".to_string()
                } else {
                    "Bluetooth service unavailable".to_string()
                });
            }
        }
        for message in new_activity {
            self.log(message);
        }

        let new_prompt = snapshot.prompt.as_ref().map(|prompt| prompt.id);
        if let Some(prompt) = &snapshot.prompt
            && self.last_prompt_id != new_prompt
        {
            self.last_prompt_id = new_prompt;
            let placeholder = if prompt.kind == PromptKind::Pin {
                "Enter PIN code"
            } else {
                "Enter 6-digit passkey"
            };
            let placeholder = SharedString::from(placeholder);
            self.code_input.update(cx, |input, cx| {
                input.set_content("", cx);
                input.set_placeholder(placeholder, cx);
            });
            if matches!(prompt.kind, PromptKind::Pin | PromptKind::Passkey) {
                self.focus_prompt = true;
            }
        } else if snapshot.prompt.is_none() {
            if self.last_prompt_id.is_some() {
                self.focus_root = true;
            }
            self.last_prompt_id = None;
        }

        self.selected = if snapshot
            .devices
            .iter()
            .any(|device| Some(&device.id) == self.selected.as_ref())
        {
            self.selected.clone()
        } else {
            snapshot
                .devices
                .iter()
                .find(|device| device.status == DeviceStatus::Connected)
                .or_else(|| snapshot.devices.first())
                .map(|device| device.id.clone())
        };
        self.previous = Some(snapshot.clone());
        self.bt = snapshot;
        if self
            .pair_id
            .as_ref()
            .is_some_and(|id| !self.bt.devices.iter().any(|device| &device.id == id))
        {
            self.pair_id = None;
        }
        cx.notify();
    }

    fn log(&mut self, message: String) {
        self.activity.insert(
            0,
            ActivityEntry {
                message,
                time: now_millis(),
            },
        );
        self.activity.truncate(100);
    }

    fn notify(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        let message = message.into();
        self.toast = message.clone();
        self.toast_generation += 1;
        let generation = self.toast_generation;
        self.log(message);
        let timer = cx.background_executor().timer(Duration::from_millis(4500));
        cx.spawn(async move |this, cx| {
            timer.await;
            this.update(cx, |this, cx| {
                if this.toast_generation == generation {
                    this.toast.clear();
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    fn begin_action(&mut self, label: impl Into<String>, cx: &mut Context<Self>) -> bool {
        if self.pending.is_some() {
            return false;
        }
        self.pending = Some(label.into());
        self.error = None;
        cx.notify();
        true
    }

    fn finish_action(
        &mut self,
        label: &str,
        result: Result<(), String>,
        success: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.pending = None;
        match result {
            Ok(()) => {
                if let Some(message) = success {
                    self.notify(message, cx);
                }
            }
            Err(error) => {
                self.error = Some(error.clone());
                self.log(format!("{label}: {error}"));
            }
        }
        cx.notify();
    }

    fn run_future<F>(
        &mut self,
        label: impl Into<String>,
        future: F,
        success: Option<String>,
        cx: &mut Context<Self>,
    ) where
        F: std::future::Future<Output = Result<(), String>> + 'static,
    {
        let label = label.into();
        if !self.begin_action(label.clone(), cx) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let result = future.await;
            this.update(cx, |this, cx| {
                this.finish_action(&label, result, success, cx)
            })
            .ok();
        })
        .detach();
    }

    fn bluetooth_action(
        &mut self,
        label: impl Into<String>,
        command: BluetoothCommand,
        success: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let bluetooth = self.bluetooth.clone();
        self.run_future(
            label,
            async move { bluetooth.execute(command).await },
            success,
            cx,
        );
    }

    fn quiet_action<F>(&mut self, label: &'static str, future: F, cx: &mut Context<Self>)
    where
        F: std::future::Future<Output = Result<(), String>> + 'static,
    {
        cx.spawn(async move |this, cx| {
            if let Err(error) = future.await {
                this.update(cx, |this, cx| {
                    this.error = Some(error.clone());
                    this.log(format!("{label}: {error}"));
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn can_control(&self) -> bool {
        self.bt.adapter_id.is_some() && self.bt.available && !self.bt.loading
    }

    fn power(&self) -> bool {
        self.adapter()
            .map(|adapter| adapter.powered)
            .unwrap_or(false)
    }

    fn visible(&self) -> bool {
        self.adapter()
            .map(|adapter| adapter.discoverable)
            .unwrap_or(false)
    }

    fn adapter(&self) -> Option<&crate::bluez::Adapter> {
        self.bt
            .adapters
            .iter()
            .find(|adapter| Some(&adapter.id) == self.bt.adapter_id.as_ref())
    }

    fn selected_device(&self) -> Option<&Device> {
        self.bt
            .devices
            .iter()
            .find(|device| Some(&device.id) == self.selected.as_ref())
    }

    fn busy(&self) -> bool {
        self.pending.is_some()
    }

    fn change_power(&mut self, cx: &mut Context<Self>) {
        let power = self.power();
        self.bluetooth_action(
            "Changing Bluetooth power",
            BluetoothCommand::SetPower(!power),
            None,
            cx,
        );
    }

    fn change_visible(&mut self, cx: &mut Context<Self>) {
        let visible = self.visible();
        self.bluetooth_action(
            "Changing discoverability",
            BluetoothCommand::SetDiscoverable(!visible),
            None,
            cx,
        );
    }

    fn open_pair(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        if !self.power() || !self.can_control() {
            return;
        }
        self.pair_id = id.or_else(|| {
            self.bt
                .devices
                .iter()
                .find(|device| device.status == DeviceStatus::Nearby)
                .map(|device| device.id.clone())
        });
        self.pair_query.clear();
        self.pair_search
            .update(cx, |input, cx| input.set_content("", cx));
        self.dialog = Some(Dialog::Pair);
        self.bluetooth_action(
            "Discovering nearby devices",
            BluetoothCommand::StartDiscovery,
            None,
            cx,
        );
    }

    fn close_dialog(&mut self, cx: &mut Context<Self>) {
        let dialog = self.dialog.take();
        if dialog == Some(Dialog::Pair) {
            if let Some(pair_id) = self.pair_id.clone()
                && self
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.starts_with("Pairing"))
            {
                self.bluetooth
                    .execute_background(BluetoothCommand::CancelPairing(pair_id));
            }
            let bluetooth = self.bluetooth.clone();
            self.quiet_action(
                "Stopping discovery",
                async move { bluetooth.execute(BluetoothCommand::StopDiscovery).await },
                cx,
            );
        }
        cx.notify();
    }

    fn pair_selected(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.pair_id.clone() else {
            return;
        };
        let Some(device) = self
            .bt
            .devices
            .iter()
            .find(|device| device.id == id)
            .cloned()
        else {
            return;
        };
        let label = format!("Pairing {}", device.name);
        if !self.begin_action(label.clone(), cx) {
            return;
        }
        let bluetooth = self.bluetooth.clone();
        let name = device.name.clone();
        let selected_id = device.id.clone();
        cx.spawn(async move |this, cx| {
            let result = async {
                bluetooth
                    .execute(BluetoothCommand::Pair(selected_id.clone()))
                    .await?;
                bluetooth.execute(BluetoothCommand::StopDiscovery).await
            }
            .await;
            this.update(cx, |this, cx| {
                this.pending = None;
                match result {
                    Ok(()) => {
                        this.selected = Some(selected_id.clone());
                        this.dialog = None;
                        this.notify(format!("{name} paired"), cx);
                    }
                    Err(error) => {
                        this.error = Some(error.clone());
                        this.log(format!("{label}: {error}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn forget_device(&mut self, cx: &mut Context<Self>) {
        let Some(device) = self.selected_device().cloned() else {
            return;
        };
        let label = format!("Forgetting {}", device.name);
        if !self.begin_action(label.clone(), cx) {
            return;
        }
        let bluetooth = self.bluetooth.clone();
        let name = device.name.clone();
        cx.spawn(async move |this, cx| {
            let result = bluetooth.execute(BluetoothCommand::Remove(device.id)).await;
            this.update(cx, |this, cx| {
                this.pending = None;
                match result {
                    Ok(()) => {
                        this.dialog = None;
                        this.notify(format!("{name} forgotten"), cx);
                    }
                    Err(error) => {
                        this.error = Some(error.clone());
                        this.log(format!("{label}: {error}"));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn reject_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.bt.prompt.clone() else {
            return;
        };
        self.bluetooth.respond_prompt(PromptReply::Reject);
        if prompt.kind == PromptKind::Display {
            let bluetooth = self.bluetooth.clone();
            let device_id = prompt.device_id;
            self.quiet_action(
                "Canceling pairing",
                async move {
                    bluetooth
                        .execute(BluetoothCommand::CancelPairing(device_id))
                        .await
                },
                cx,
            );
        }
        cx.notify();
    }

    fn accept_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(prompt) = self.bt.prompt.clone() else {
            return;
        };
        let reply = match prompt.kind {
            PromptKind::Pin | PromptKind::Passkey => {
                PromptReply::Text(self.code_input.read(cx).content().to_string())
            }
            _ => PromptReply::Accept,
        };
        self.bluetooth.respond_prompt(reply);
        cx.notify();
    }

    fn on_root_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.keystroke.key == "escape" {
            if self.bt.prompt.is_some() {
                self.reject_prompt(cx);
            } else {
                self.close_dialog(cx);
            }
            self.menu = false;
            self.dropdown = None;
            cx.notify();
        }
    }

    fn on_root_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let position = event.position;
        if self.menu {
            let menu = self.menu_bounds();
            if !rect_contains(menu, position) && !points_near(self.menu_trigger, position, 40.0) {
                self.menu = false;
                cx.notify();
            }
        }
        if let Some(dropdown) = self.dropdown.clone()
            && let Some(rect) = self.dropdown_bounds()
            && !rect_contains(rect, position)
            && !points_near(dropdown.trigger, position, 40.0)
        {
            self.dropdown = None;
            cx.notify();
        }
    }

    fn menu_bounds(&self) -> Bounds<Pixels> {
        let top = (f32::from(self.menu_anchor.y) + 20.0)
            .min(self.window_height - 104.0)
            .max(48.0);
        let left = (f32::from(self.menu_anchor.x) - 184.0)
            .max(8.0)
            .min(self.window_width - 208.0);
        Bounds::new(point(px(left), px(top)), size(px(200.0), px(104.0)))
    }

    fn open_menu(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        self.menu_anchor = position;
        self.menu_trigger = position;
        self.menu = !self.menu;
        self.dropdown = None;
        cx.notify();
    }

    fn open_dropdown(
        &mut self,
        kind: DropdownKind,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        if let Some(existing) = self.dropdown.as_ref()
            && same_dropdown(&existing.kind, &kind)
            && points_near(existing.trigger, position, 40.0)
        {
            self.dropdown = None;
            cx.notify();
            return;
        }
        let (left, width) = match kind {
            DropdownKind::AudioProfile { .. } => (self.window_width - 316.0, 292.0),
            DropdownKind::Adapter => (276.0, (self.window_width - 672.0).max(240.0)),
        };
        let top = (f32::from(position.y) + 25.0).max(48.0);
        self.dropdown = Some(Dropdown {
            kind,
            top,
            left,
            width,
            trigger: position,
        });
        self.menu = false;
        cx.notify();
    }

    fn dropdown_bounds(&self) -> Option<Bounds<Pixels>> {
        let dropdown = self.dropdown.as_ref()?;
        let rows = match &dropdown.kind {
            DropdownKind::AudioProfile { address } => self
                .sound
                .devices
                .get(&address.to_ascii_uppercase())
                .map(|device| device.profiles.len())
                .unwrap_or(0),
            DropdownKind::Adapter => self.bt.adapters.len(),
        };
        let height = (rows as f32 * 40.0 + 8.0).max(40.0);
        let top = dropdown
            .top
            .min(self.window_height - height - 8.0)
            .max(48.0);
        Some(Bounds::new(
            point(px(dropdown.left), px(top)),
            size(px(dropdown.width), px(height)),
        ))
    }

    fn change_volume_at(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(device) = self.selected_device().cloned() else {
            return;
        };
        let audio_device = self
            .sound
            .devices
            .get(&device.address.to_ascii_uppercase())
            .cloned();
        if audio_device.and_then(|device| device.sink_name).is_none() {
            return;
        }
        let left = self.window_width - 290.0;
        let width = 266.0;
        let value = (((f32::from(position.x) - left) / width) * 100.0)
            .round()
            .clamp(0.0, 100.0) as i32;
        self.draft_volume = Some((device.address, value));
        cx.notify();
    }

    fn commit_volume(
        &mut self,
        _event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dragging_volume = false;
        let Some((address, value)) = self.draft_volume.clone() else {
            return;
        };
        if self.busy() {
            self.draft_volume = None;
            cx.notify();
            return;
        }
        let label = "Changing volume";
        if !self.begin_action(label, cx) {
            self.draft_volume = None;
            return;
        }
        let audio = self.audio.clone();
        cx.spawn(async move |this, cx| {
            let result = audio.set_volume(&address, value as f64).await;
            this.update(cx, |this, cx| {
                this.draft_volume = None;
                this.finish_action(label, result, None, cx);
            })
            .ok();
        })
        .detach();
    }

    fn set_volume(&mut self, value: i32, cx: &mut Context<Self>) {
        let Some(device) = self.selected_device().cloned() else {
            return;
        };
        let audio = self.audio.clone();
        self.run_future(
            "Changing volume",
            async move {
                audio
                    .set_volume(&device.address, value.clamp(0, 100) as f64)
                    .await
            },
            None,
            cx,
        );
    }

    fn select_page(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        self.menu = false;
        self.dropdown = None;
        cx.notify();
    }
}

fn rect_contains(bounds: Bounds<Pixels>, point: Point<Pixels>) -> bool {
    bounds.contains(&point)
}

fn points_near(a: Point<Pixels>, b: Point<Pixels>, distance: f32) -> bool {
    let dx = f32::from(a.x) - f32::from(b.x);
    let dy = f32::from(a.y) - f32::from(b.y);
    dx * dx + dy * dy <= distance * distance
}

fn same_dropdown(left: &DropdownKind, right: &DropdownKind) -> bool {
    match (left, right) {
        (DropdownKind::Adapter, DropdownKind::Adapter) => true,
        (
            DropdownKind::AudioProfile { address: left },
            DropdownKind::AudioProfile { address: right },
        ) => left == right,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Rendering

impl BluetApp {
    fn render_titlebar(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(44.0))
            .px(px(16.0))
            .flex_shrink_0()
            .bg(C::sidebar())
            .border_b_1()
            .border_color(C::border())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(icon(Icon::Bluetooth, 15.0, C::accent()))
                    .child(t("Bluet", 12.0, C::muted(), 600.0)),
            )
            .child(
                div()
                    .id("titlebar-drag")
                    .flex_grow()
                    .h_full()
                    .window_control_area(WindowControlArea::Drag),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .child(icon_button(
                        "window-minimize",
                        Icon::Minus,
                        "Minimize window",
                        |_, window, _| window.minimize_window(),
                    ))
                    .child(icon_button(
                        "window-maximize",
                        Icon::Square,
                        "Maximize window",
                        |_, window, _| window.zoom_window(),
                    ))
                    .child(icon_button(
                        "window-close",
                        Icon::X,
                        "Close Bluet",
                        cx.listener(|this, _: &ClickEvent, _window, cx| {
                            this.begin_shutdown(cx);
                        }),
                    )),
            )
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> Div {
        let adapter = self.adapter().cloned();
        let power = adapter
            .as_ref()
            .map(|adapter| adapter.powered)
            .unwrap_or(false);
        let can_control = self.can_control();
        let busy = self.busy();
        let service_text = if self.bt.loading {
            "Connecting to service…"
        } else if !self.bt.available {
            "Service unavailable"
        } else if adapter.is_none() {
            "No adapter found"
        } else if power {
            "Connected to BlueZ"
        } else {
            "Bluetooth off"
        };

        let mut nav = div().flex().flex_col().gap(px(6.0));
        for page in [Page::Devices, Page::FileTransfers, Page::Activity] {
            let active = self.page == page;
            nav = nav.child(button(
                page.test_id(),
                ButtonOptions {
                    transparent: false,
                    border: Some(false),
                    height: Some(38.0),
                    padding_x: Some(12.0),
                    gap: Some(10.0),
                    justify_start: true,
                    background: Some(if active {
                        C::selection()
                    } else {
                        gpui::transparent_black()
                    }),
                    ..ButtonOptions::default()
                },
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.select_page(page, cx);
                }),
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(icon(
                        page.icon(),
                        18.0,
                        if active { C::accent() } else { C::muted() },
                    ))
                    .child(t(
                        page.id(),
                        13.0,
                        if active { C::fg() } else { C::muted() },
                        500.0,
                    )),
            ));
        }

        div()
            .relative()
            .flex()
            .flex_col()
            .w(px(220.0))
            .flex_shrink_0()
            .pt(px(28.0))
            .px(px(18.0))
            .pb(px(20.0))
            .gap(px(28.0))
            .bg(C::sidebar())
            .border_r_1()
            .border_color(C::border())
            .child(dither_layer(DitherVariant::Ambient, 0.7, 0.75, 0.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(36.0))
                            .h(px(40.0))
                            .rounded(px(9.0))
                            .bg(C::primary())
                            .child(icon(Icon::Bluetooth, 24.0, gpui::white())),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(t("Bluet", 21.0, C::fg(), 600.0))
                            .child(t("Bluetooth manager", 10.0, C::muted(), 400.0)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(14.0))
                    .gap(px(10.0))
                    .bg(C::panel())
                    .border_1()
                    .border_color(C::border())
                    .rounded(px(8.0))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(t("Bluetooth", 13.0, C::fg(), 500.0))
                            .child(toggle(
                                "bluetooth-power",
                                power,
                                !can_control || busy,
                                cx.listener(|this, _: &ClickEvent, _, cx| this.change_power(cx)),
                            )),
                    )
                    .child(
                        div()
                            .id("service-status")
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .child(div().w(px(5.0)).h(px(5.0)).rounded(px(3.0)).bg(if power {
                                C::green()
                            } else {
                                C::subtle()
                            }))
                            .child(t(service_text, 10.0, C::muted(), 400.0)),
                    ),
            )
            .child(nav)
            .child(div().flex_grow())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .border_t_1()
                    .border_color(C::border())
                    .pt(px(16.0))
                    .pb(px(16.0))
                    .px(px(4.0))
                    .gap(px(12.0))
                    .child(m("THIS COMPUTER", 9.0, C::subtle()))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .child(icon(Icon::Laptop, 20.0, C::muted()))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(4.0))
                                    .child(t(
                                        adapter
                                            .as_ref()
                                            .map(|adapter| adapter.name.clone())
                                            .unwrap_or_else(|| "No Bluetooth adapter".into()),
                                        12.0,
                                        C::fg(),
                                        500.0,
                                    ))
                                    .child(t(
                                        adapter
                                            .as_ref()
                                            .map(|adapter| {
                                                let id = adapter
                                                    .id
                                                    .rsplit('/')
                                                    .next()
                                                    .unwrap_or_default();
                                                format!("{id} · {}", adapter.address)
                                            })
                                            .unwrap_or_else(|| "BlueZ system service".into()),
                                        10.0,
                                        C::muted(),
                                        400.0,
                                    )),
                            ),
                    ),
            )
            .child(button(
                "nav-settings",
                ButtonOptions {
                    transparent: false,
                    border: Some(false),
                    height: Some(38.0),
                    padding_x: Some(12.0),
                    gap: Some(10.0),
                    justify_start: true,
                    background: Some(if self.page == Page::Settings {
                        C::selection()
                    } else {
                        gpui::transparent_black()
                    }),
                    ..ButtonOptions::default()
                },
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.select_page(Page::Settings, cx);
                }),
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.0))
                    .child(icon(Icon::Settings2, 18.0, C::muted()))
                    .child(t("Settings", 13.0, C::muted(), 500.0)),
            ))
    }

    fn render_main(&mut self, cx: &mut Context<Self>) -> Div {
        let mut main = div()
            .flex()
            .flex_col()
            .flex_grow()
            .min_w(px(0.0))
            .pt(px(30.0))
            .px(px(32.0))
            .pb(px(24.0))
            .gap(px(22.0))
            .bg(linear_gradient(
                120.0,
                linear_color_stop(C::bg(), 0.0),
                linear_color_stop(C::main_tint(), 1.0),
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.0))
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .flex_grow()
                            .child(t(self.page.title(), 28.0, C::fg(), 600.0))
                            .child(t(self.page.subtitle(), 12.0, C::muted(), 400.0)),
                    )
                    .when(self.page == Page::Devices, |element| {
                        let disabled = !self.can_control() || !self.power() || self.busy();
                        element.child(button(
                            "add-device",
                            ButtonOptions {
                                primary: true,
                                border: Some(false),
                                disabled,
                                ..ButtonOptions::default()
                            },
                            cx.listener(|this, _: &ClickEvent, _, cx| this.open_pair(None, cx)),
                            {
                                let mut content = div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(icon(Icon::Plus, 16.0, gpui::white()));
                                content = content.child(t("Add device", 13.0, C::fg(), 600.0));
                                content
                            },
                        ))
                    }),
            );

        let failure = self.error.clone().or_else(|| self.bt.error.clone());
        if let Some(failure) = failure {
            let mut banner = div()
                .id("service-error")
                .flex()
                .flex_col()
                .gap(px(8.0))
                .p(px(12.0))
                .border_1()
                .border_color(C::error_border())
                .rounded(px(6.0))
                .bg(C::error_bg())
                .flex_shrink_0()
                .child(
                    div()
                        .font_family("Inter")
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(C::red())
                        .child(failure),
                );
            if self.error.is_some() {
                banner = banner.child(div().flex().justify_start().child(button(
                    "dismiss-error",
                    ButtonOptions {
                        small: true,
                        ..ButtonOptions::default()
                    },
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.error = None;
                        cx.notify();
                    }),
                    t("Dismiss", 11.0, C::fg(), 500.0),
                )));
            }
            main = main.child(banner);
        }

        if let Some(pending) = &self.pending {
            main = main.child(
                div()
                    .id("pending-action")
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .flex_shrink_0()
                    .child(icon(Icon::Radar, 14.0, C::accent()))
                    .child(t(format!("{pending}…"), 11.0, C::accent(), 400.0)),
            );
        }

        match self.page {
            Page::Devices => main = main.child(self.render_devices(cx)),
            Page::Activity => main = main.child(self.render_activity()),
            Page::Settings => main = main.child(self.render_settings(cx)),
            Page::FileTransfers => {
                main = main.child(empty_state(
                    Icon::ArrowLeftRight,
                    "File transfers are unavailable",
                    "Bluetooth file transfer support has not been configured in Bluet. Device connections and pairing use the system service.",
                    None,
                ))
            }
        }
        main
    }

    fn render_devices(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let bt_loading = self.bt.loading;
        let available = self.bt.available;
        let has_adapter = self.adapter().is_some();
        let power = self.power();
        let busy = self.busy();

        let mut list = div()
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .gap(px(24.0))
            .id("device-list")
            .overflow_y_scroll()
            .scrollbar_width(px(3.0));

        if bt_loading {
            list = list.child(empty_state(
                Icon::Bluetooth,
                "Loading Bluetooth devices",
                "Connecting to the system Bluetooth service…",
                None,
            ));
        } else if !available {
            list = list.child(empty_state(
                Icon::BluetoothOff,
                "Bluetooth service unavailable",
                "Check that the system Bluetooth service is running.",
                Some(button(
                    "reconnect-bluetooth",
                    ButtonOptions {
                        disabled: busy,
                        ..ButtonOptions::default()
                    },
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.bluetooth_action(
                            "Reconnecting to Bluetooth",
                            BluetoothCommand::Reconnect,
                            None,
                            cx,
                        );
                    }),
                    t("Reconnect", 13.0, C::fg(), 500.0),
                )),
            ));
        } else if !has_adapter {
            list = list.child(empty_state(
                Icon::BluetoothOff,
                "No Bluetooth adapter",
                "Connect a Bluetooth adapter to get started.",
                None,
            ));
        } else if !power {
            list = list.child(empty_state(
                Icon::BluetoothOff,
                "Bluetooth is off",
                "Turn on Bluetooth to connect your devices.",
                Some(button(
                    "turn-on-bluetooth",
                    ButtonOptions {
                        primary: true,
                        border: Some(false),
                        disabled: busy,
                        ..ButtonOptions::default()
                    },
                    cx.listener(|this, _: &ClickEvent, _, cx| this.change_power(cx)),
                    t("Turn on Bluetooth", 13.0, C::fg(), 600.0),
                )),
            ));
        } else {
            let visible = filter_devices(&self.bt.devices, self.filter, &self.query);
            for status in [
                DeviceStatus::Connected,
                DeviceStatus::Saved,
                DeviceStatus::Nearby,
            ] {
                let group: Vec<_> = visible
                    .iter()
                    .filter(|device| device.status == status)
                    .cloned()
                    .collect();
                if group.is_empty() {
                    continue;
                }
                let mut group_element = div().flex().flex_col().flex_shrink_0().child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .mb(px(12.0))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(9.0))
                                .child(t(status.title(), 12.0, C::fg(), 500.0))
                                .child(m(group.len().to_string(), 10.0, C::muted())),
                        )
                        .when(
                            status == DeviceStatus::Nearby
                                && self
                                    .adapter()
                                    .map(|adapter| adapter.discovering)
                                    .unwrap_or(false),
                            |element| element.child(radar("Scanning nearby…")),
                        ),
                );
                for device in &group {
                    group_element = group_element.child(self.render_device_row(device, cx));
                }
                list = list.child(group_element);
            }
            if visible.is_empty() {
                let clear = if !self.query.is_empty() || self.filter != DeviceFilter::All {
                    Some(button(
                        "clear-filters",
                        ButtonOptions::default(),
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.query.clear();
                            this.filter = DeviceFilter::All;
                            this.search
                                .update(cx, |input, cx| input.set_content("", cx));
                            cx.notify();
                        }),
                        t("Clear filters", 13.0, C::fg(), 500.0),
                    ))
                } else {
                    None
                };
                list = list.child(empty_state(
                    Icon::Search,
                    if self.query.is_empty() {
                        "No devices to show"
                    } else {
                        "No devices found"
                    },
                    if self.query.is_empty() {
                        "Use Add device to discover nearby Bluetooth devices."
                    } else {
                        "Try a different search or filter."
                    },
                    clear,
                ));
            }
        }

        let visible = self.visible();
        let can_control = self.can_control();
        let busy = self.busy();
        let adapter_name = self
            .adapter()
            .map(|adapter| adapter.name.clone())
            .unwrap_or_default();
        let area = div()
            .flex()
            .flex_col()
            .flex_grow()
            .min_h(px(0.0))
            .gap(px(22.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(20.0))
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.0))
                            .p(px(3.0))
                            .rounded(px(7.0))
                            .bg(C::bg())
                            .children(DeviceFilter::ALL.into_iter().map(|filter| {
                                let active = self.filter == filter;
                                button(
                                    SharedString::from(format!("filter-{}", filter.key())),
                                    ButtonOptions {
                                        transparent: false,
                                        border: Some(false),
                                        height: Some(31.0),
                                        padding_x: Some(12.0),
                                        background: Some(if active {
                                            C::raised()
                                        } else {
                                            gpui::transparent_black()
                                        }),
                                        ..ButtonOptions::default()
                                    },
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.filter = filter;
                                        cx.notify();
                                    }),
                                    t(
                                        filter.title(),
                                        12.0,
                                        if active { C::fg() } else { C::muted() },
                                        400.0,
                                    ),
                                )
                            })),
                    )
                    .child(
                        div()
                            .id("device-search")
                            .child(search_field(self.search.clone(), Some(250.0))),
                    ),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .border_t_1()
                    .border_color(C::border())
                    .pt(px(16.0))
                    .justify_between()
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(7.0))
                            .child(icon(
                                if visible { Icon::Eye } else { Icon::EyeOff },
                                13.0,
                                C::muted(),
                            ))
                            .child(t(
                                if visible {
                                    format!("Visible as {adapter_name}")
                                } else {
                                    "Hidden from nearby devices".to_string()
                                },
                                11.0,
                                C::muted(),
                                400.0,
                            )),
                    )
                    .child(button(
                        "discoverable",
                        ButtonOptions {
                            transparent: true,
                            border: Some(false),
                            height: Some(16.0),
                            padding_x: Some(0.0),
                            disabled: !can_control || !power || busy,
                            ..ButtonOptions::default()
                        },
                        cx.listener(|this, _: &ClickEvent, _, cx| this.change_visible(cx)),
                        t(
                            if visible {
                                "Stop sharing"
                            } else {
                                "Make discoverable"
                            },
                            11.0,
                            C::accent(),
                            400.0,
                        ),
                    )),
            );
        area.into_any_element()
    }

    fn render_device_row(&self, device: &Device, cx: &mut Context<Self>) -> AnyElement {
        let selected = Some(&device.id) == self.selected.as_ref();
        let connected = device.status == DeviceStatus::Connected;
        let busy = self.busy();
        let hover_background = if selected {
            C::selection_hover()
        } else {
            C::row_hover()
        };
        let row_element_id = SharedString::from(format!("device-{}", device.id));
        let row_selected_id = device.id.clone();
        let mut row = div()
            .id(row_element_id)
            .relative()
            .flex()
            .items_center()
            .gap(px(14.0))
            .h(px(if connected { 72.0 } else { 68.0 }))
            .flex_shrink_0()
            .px(px(16.0))
            .rounded(px(if selected { 7.0 } else { 0.0 }))
            .border_b_1()
            .border_color(if selected { C::primary() } else { C::border() })
            .bg(if selected {
                C::selection()
            } else {
                gpui::transparent_black()
            })
            .cursor(CursorStyle::PointingHand)
            .hover(move |style| style.bg(hover_background))
            .when(selected, |element| element.border_1())
            .when(selected, |element| {
                element.child(dither_layer(DitherVariant::Selection, 0.9, 1.0, 7.0))
            });

        // The select area and the row's action buttons are siblings, so a click on
        // the options button does not also select the row and close its menu.
        row = row.child(
            div()
                .id(SharedString::from(format!("device-select-{}", device.id)))
                .flex()
                .items_center()
                .gap(px(14.0))
                .flex_grow()
                .h_full()
                .rounded(px(6.0))
                .cursor(CursorStyle::PointingHand)
                .tab_stop(true)
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.selected = Some(row_selected_id.clone());
                    this.menu = false;
                    this.dropdown = None;
                    cx.notify();
                }))
                .on_key_down({
                    let selected = device.id.clone();
                    cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                            this.selected = Some(selected.clone());
                            this.menu = false;
                            cx.notify();
                        }
                    })
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .w(px(42.0))
                        .h(px(42.0))
                        .rounded(px(8.0))
                        .flex_shrink_0()
                        .bg(C::raised())
                        .child(icon(
                            kind_icon(device.kind),
                            22.0,
                            if selected { C::accent() } else { C::muted() },
                        )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.0))
                        .flex_grow()
                        .min_w(px(0.0))
                        .child(t(device.name.clone(), 13.0, C::fg(), 500.0))
                        .child(t(
                            format!("{} · {}", device.kind.title(), status_label(device)),
                            11.0,
                            C::muted(),
                            400.0,
                        )),
                ),
        );

        if connected {
            if let Some(battery) = device.battery {
                row = row.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .child(icon(Icon::BatteryMedium, 17.0, C::green()))
                        .child(m(format!("{battery}%"), 11.0, C::muted())),
                );
            }
            let menu_id = SharedString::from(format!("device-options-{}", device.id));
            let select = device.id.clone();
            row = row.child(
                div()
                    .id(menu_id)
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(px(28.0))
                    .h(px(28.0))
                    .rounded(px(6.0))
                    .cursor(CursorStyle::PointingHand)
                    .tab_stop(true)
                    .hover(|style| style.bg(C::raised_hover()))
                    .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                        this.selected = Some(select.clone());
                        this.open_menu(event.position(), cx);
                    }))
                    .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                            this.open_menu(point(px(1000.0), px(200.0)), cx);
                        }
                    }))
                    .child(icon(Icon::Ellipsis, 16.0, C::muted())),
            );
        } else if device.status == DeviceStatus::Nearby {
            let pair_id = device.id.clone();
            row = row.child(button(
                SharedString::from(format!("pair-{}", device.id)),
                ButtonOptions {
                    small: true,
                    disabled: busy,
                    background: Some(C::selection()),
                    border: Some(false),
                    ..ButtonOptions::default()
                },
                cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.open_pair(Some(pair_id.clone()), cx);
                }),
                t("Pair", 11.0, C::accent(), 400.0),
            ));
        } else {
            let connect_id = device.id.clone();
            row = row.child(button(
                SharedString::from(format!("connect-{}", device.id)),
                ButtonOptions {
                    small: true,
                    disabled: busy,
                    ..ButtonOptions::default()
                },
                {
                    let id = connect_id.clone();
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let name = this
                            .bt
                            .devices
                            .iter()
                            .find(|device| device.id == id)
                            .map(|device| device.name.clone())
                            .unwrap_or_else(|| "device".into());
                        this.bluetooth_action(
                            format!("Connecting {name}"),
                            BluetoothCommand::Connect(id.clone()),
                            None,
                            cx,
                        );
                    })
                },
                t("Connect", 11.0, C::fg(), 400.0),
            ));
        }
        row.into_any_element()
    }

    fn render_activity(&self) -> AnyElement {
        let adapter_name = self
            .adapter()
            .map(|adapter| adapter.name.clone())
            .unwrap_or_else(|| "BlueZ".into());
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(12.0))
            .flex_grow()
            .min_h(px(0.0))
            .id("activity-list")
            .overflow_y_scroll()
            .scrollbar_width(px(3.0));
        if self.activity.is_empty() {
            list = list.child(empty_state(
                Icon::Clock3,
                "No activity yet",
                "Connection changes and completed actions will appear here.",
                None,
            ));
        } else {
            for (index, entry) in self.activity.iter().enumerate() {
                let time = DateTime::<Local>::from(UNIX_EPOCH + Duration::from_millis(entry.time))
                    .format("%I:%M:%S %p")
                    .to_string();
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .p(px(18.0))
                        .gap(px(14.0))
                        .border_1()
                        .border_color(C::border())
                        .rounded(px(7.0))
                        .bg(C::panel())
                        .child(icon(
                            Icon::Clock3,
                            18.0,
                            if index == 0 { C::accent() } else { C::muted() },
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(5.0))
                                .flex_grow()
                                .child(t(entry.message.clone(), 13.0, C::fg(), 400.0))
                                .child(t(
                                    format!("{time} · {adapter_name}"),
                                    11.0,
                                    C::muted(),
                                    400.0,
                                )),
                        ),
                );
            }
        }
        list.into_any_element()
    }

    fn render_settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let adapter = self.adapter().cloned();
        let power = adapter
            .as_ref()
            .map(|adapter| adapter.powered)
            .unwrap_or(false);
        let visible = adapter
            .as_ref()
            .map(|adapter| adapter.discoverable)
            .unwrap_or(false);
        let can_control = self.can_control();
        let busy = self.busy();
        let adapters = self.bt.adapters.clone();
        let adapter_id = self.bt.adapter_id.clone();

        let mut card = div()
            .flex()
            .flex_col()
            .gap(px(24.0))
            .p(px(24.0))
            .rounded(px(8.0))
            .border_1()
            .border_color(C::border())
            .bg(C::panel())
            .child(t("Bluetooth adapter", 17.0, C::fg(), 600.0));

        if adapters.len() > 1 {
            let current = adapters
                .iter()
                .find(|candidate| Some(&candidate.id) == adapter_id.as_ref())
                .map(|adapter| adapter.name.clone())
                .unwrap_or_else(|| "Select adapter".into());
            let trigger_id = SharedString::from("adapter-select");
            card = card.child(
                div()
                    .id(trigger_id)
                    .flex()
                    .items_center()
                    .justify_between()
                    .p(px(12.0))
                    .rounded(px(6.0))
                    .bg(C::raised())
                    .cursor(CursorStyle::PointingHand)
                    .tab_stop(true)
                    .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                        this.open_dropdown(DropdownKind::Adapter, event.position(), cx);
                    }))
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" || event.keystroke.key == "space" {
                            this.open_dropdown(
                                DropdownKind::Adapter,
                                point(px(276.0), px(400.0)),
                                cx,
                            );
                        }
                    }))
                    .child(
                        t(current, 13.0, C::fg(), 400.0)
                            .flex_grow()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis(),
                    )
                    .child(icon(Icon::ChevronDown, 14.0, C::muted())),
            );
        }

        card = card
            .child(info_row(
                "Computer name",
                adapter
                    .as_ref()
                    .map(|adapter| adapter.name.clone())
                    .unwrap_or_else(|| "Unavailable".into()),
                C::fg(),
                11.0,
            ))
            .child(info_row(
                "Adapter",
                adapter
                    .as_ref()
                    .and_then(|adapter| adapter.id.rsplit('/').next().map(str::to_string))
                    .unwrap_or_else(|| "Unavailable".into()),
                C::fg(),
                11.0,
            ))
            .child(info_row(
                "Address",
                adapter
                    .as_ref()
                    .map(|adapter| adapter.address.clone())
                    .filter(|address| !address.is_empty())
                    .unwrap_or_else(|| "Unavailable".into()),
                C::fg(),
                11.0,
            ))
            .child(info_row(
                "Service",
                if self.bt.available {
                    "BlueZ · system D-Bus"
                } else {
                    "Unavailable"
                },
                C::fg(),
                11.0,
            ))
            .child(divider())
            .child(preference(
                "Bluetooth",
                "Allow Bluetooth connections",
                power,
                !can_control || busy,
                "settings-power",
                cx.listener(|this, _: &ClickEvent, _, cx| this.change_power(cx)),
            ))
            .child(preference(
                "Discoverable",
                "Let nearby devices find this computer",
                visible,
                !can_control || !power || busy,
                "settings-discoverable",
                cx.listener(|this, _: &ClickEvent, _, cx| this.change_visible(cx)),
            ))
            .child(t(
                "Connected to the Bluetooth service installed on this computer.",
                11.0,
                C::muted(),
                400.0,
            ));
        card.into_any_element()
    }

    fn render_inspector(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let device = self.selected_device().cloned();
        let connected = device
            .as_ref()
            .map(|device| device.status == DeviceStatus::Connected)
            .unwrap_or(false);
        let audio_device = device
            .as_ref()
            .and_then(|device| self.sound.devices.get(&device.address.to_ascii_uppercase()))
            .cloned();
        let draft_volume = self.draft_volume.clone();
        let volume = draft_volume
            .as_ref()
            .filter(|(address, _)| {
                device
                    .as_ref()
                    .is_some_and(|device| device.address == *address)
            })
            .map(|(_, value)| *value)
            .or_else(|| audio_device.as_ref().map(|device| device.volume))
            .unwrap_or(0);
        let can_control = self.can_control();
        let power = self.power();
        let busy = self.busy();

        let mut inspector = div()
            .relative()
            .flex()
            .flex_col()
            .w(px(340.0))
            .flex_shrink_0()
            .p(px(24.0))
            .gap(px(22.0))
            .border_l_1()
            .border_color(C::border())
            .bg(linear_gradient(
                120.0,
                linear_color_stop(C::panel(), 0.0),
                linear_color_stop(C::inspector_tint(), 1.0),
            ))
            .id("inspector")
            .overflow_y_scroll()
            .scrollbar_width(px(3.0))
            .child(dither_layer(DitherVariant::Ambient, 1.0, 0.8, 0.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(28.0))
                    .flex_shrink_0()
                    .child(t("Device details", 12.0, C::muted(), 400.0))
                    .when(device.is_some(), |element| {
                        element.child(
                            div()
                                .id("device-options")
                                .flex()
                                .items_center()
                                .justify_center()
                                .w(px(28.0))
                                .h(px(28.0))
                                .rounded(px(6.0))
                                .cursor(CursorStyle::PointingHand)
                                .tab_stop(true)
                                .hover(|style| style.bg(C::raised_hover()))
                                .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                                    this.open_menu(event.position(), cx);
                                }))
                                .on_key_down({
                                    let fallback = point(px(self.window_width - 44.0), px(83.0));
                                    cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                        if event.keystroke.key == "enter"
                                            || event.keystroke.key == "space"
                                        {
                                            this.open_menu(fallback, cx);
                                        }
                                    })
                                })
                                .child(icon(Icon::Ellipsis, 16.0, C::muted())),
                        )
                    }),
            );

        let Some(device) = device else {
            inspector = inspector.child(empty_state(
                Icon::Bluetooth,
                "No device selected",
                "Select a device to see its details.",
                None,
            ));
            return inspector.into_any_element();
        };

        let mut content = div()
            .flex()
            .flex_col()
            .gap(px(22.0))
            .flex_shrink_0()
            .min_h(px((self.window_height - 142.0).max(0.0)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(10.0))
                    .pt(px(4.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .relative()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(88.0))
                            .h(px(88.0))
                            .rounded(px(44.0))
                            .bg(C::halo_bg())
                            .border_7()
                            .border_color(C::halo_border())
                            .child(dither_layer(DitherVariant::Halo, 1.0, 1.0, 44.0))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .w(px(68.0))
                                    .h(px(68.0))
                                    .rounded(px(34.0))
                                    .border_3()
                                    .border_color(C::halo_inner())
                                    .child(icon(kind_icon(device.kind), 38.0, C::accent())),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(6.0))
                            .pt(px(6.0))
                            .child(t(device.name.clone(), 22.0, C::fg(), 600.0))
                            .child(t(device.description.clone(), 11.0, C::muted(), 400.0)),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .py(px(4.0))
                            .px(px(8.0))
                            .rounded(px(4.0))
                            .bg(if connected {
                                C::green_bg()
                            } else {
                                C::raised()
                            })
                            .child(
                                div()
                                    .w(px(5.0))
                                    .h(px(5.0))
                                    .rounded(px(3.0))
                                    .bg(if connected { C::green() } else { C::muted() }),
                            )
                            .child(t(
                                if connected {
                                    "Connected"
                                } else if device.paired {
                                    "Paired · disconnected"
                                } else {
                                    "Not paired"
                                },
                                11.0,
                                if connected { C::green() } else { C::muted() },
                                400.0,
                            )),
                    ),
            );

        if let Some(battery) = device.battery {
            content = content.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.0))
                    .pb(px(2.0))
                    .flex_shrink_0()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(7.0))
                                    .child(icon(Icon::BatteryMedium, 15.0, C::green()))
                                    .child(t("Battery", 11.0, C::muted(), 400.0)),
                            )
                            .child(t(format!("{battery}%"), 12.0, C::fg(), 500.0)),
                    )
                    .child(
                        div()
                            .relative()
                            .h(px(5.0))
                            .bg(C::raised())
                            .rounded(px(3.0))
                            .child(
                                div()
                                    .h(px(5.0))
                                    .w(relative(battery as f32 / 100.0))
                                    .rounded(px(3.0))
                                    .bg(C::meter_fill()),
                            )
                            .child(dither_meter(battery as f32 / 100.0, 3.0)),
                    ),
            );
        } else {
            content = content.child(info_row("Battery", "Not reported", C::fg(), 11.0));
        }
        content = content.child(divider());

        if device.kind == DeviceKind::Headphones || device.kind == DeviceKind::Speaker {
            let codec = audio_device
                .as_ref()
                .and_then(|device| device.codec.as_ref())
                .map(|codec| codec.to_ascii_uppercase())
                .unwrap_or_else(|| {
                    if self.sound.available {
                        "SYSTEM AUDIO".into()
                    } else {
                        "UNAVAILABLE".into()
                    }
                });
            let mut audio_section = div().flex().flex_col().gap(px(14.0)).flex_shrink_0().child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(t("Audio", 13.0, C::fg(), 500.0))
                    .child(m(codec, 8.0, C::muted())),
            );
            if let Some(audio) = audio_device.clone() {
                let profile_label = short_profile_label(
                    audio
                        .profiles
                        .iter()
                        .find(|profile| Some(&profile.id) == audio.profile.as_ref())
                        .map(|profile| profile.label.as_str())
                        .or(audio.profile.as_deref())
                        .unwrap_or("No active profile"),
                );
                let address = device.address.clone();
                let address_for_key = address.clone();
                let fallback = point(px(self.window_width - 316.0), px(400.0));
                let disable_profile = busy || audio.profiles.is_empty();
                let profile_trigger = div()
                    .id("audio-profile")
                    .flex()
                    .items_center()
                    .justify_between()
                    .h(px(38.0))
                    .px(px(12.0))
                    .bg(C::raised())
                    .border_1()
                    .border_color(C::border())
                    .rounded(px(6.0))
                    .when(disable_profile, |element| element.opacity(0.4))
                    .when(!disable_profile, |element| {
                        element
                            .cursor(CursorStyle::PointingHand)
                            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                                this.open_dropdown(
                                    DropdownKind::AudioProfile {
                                        address: address.clone(),
                                    },
                                    event.position(),
                                    cx,
                                );
                            }))
                            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                                if event.keystroke.key == "enter" || event.keystroke.key == "space"
                                {
                                    this.open_dropdown(
                                        DropdownKind::AudioProfile {
                                            address: address_for_key.clone(),
                                        },
                                        fallback,
                                        cx,
                                    );
                                }
                            }))
                    })
                    .child(
                        t(profile_label, 12.0, C::fg(), 400.0)
                            .flex_grow()
                            .min_w(px(0.0))
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis(),
                    )
                    .child(icon(Icon::ChevronDown, 14.0, C::muted()));
                let sink_available = audio.sink_name.is_some();
                let muted = audio.muted;
                let displayed = if muted { 0 } else { volume };
                let percentage = displayed.clamp(0, 100) as f32;
                let thumb_left = (266.0 - 10.0) * percentage / 100.0;
                let mute_icon = if muted { Icon::VolumeX } else { Icon::Volume2 };
                let mute_address = device.address.clone();
                let slider_volume = volume;
                audio_section = audio_section
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(t("Output profile", 11.0, C::muted(), 400.0))
                            .child(profile_trigger),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.0))
                            .child(info_row(
                                "Volume",
                                if sink_available {
                                    format!("{displayed}%")
                                } else {
                                    "Unavailable".into()
                                },
                                C::fg(),
                                11.0,
                            ))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.0))
                                    .h(px(16.0))
                                    .child(button(
                                        "audio-mute",
                                        ButtonOptions {
                                            transparent: true,
                                            border: Some(false),
                                            height: Some(16.0),
                                            padding_x: Some(0.0),
                                            disabled: busy || !sink_available,
                                            ..ButtonOptions::default()
                                        },
                                        {
                                            let audio = self.audio.clone();
                                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.run_future(
                                                    "Changing mute",
                                                    {
                                                        let audio = audio.clone();
                                                        let address = mute_address.clone();
                                                        let muted = !muted;
                                                        async move {
                                                            audio.set_muted(&address, muted).await
                                                        }
                                                    },
                                                    None,
                                                    cx,
                                                );
                                            })
                                        },
                                        icon(mute_icon, 15.0, C::muted()),
                                    ))
                                    .child(
                                        div()
                                            .id("volume-slider")
                                            .flex()
                                            .items_center()
                                            .h(px(16.0))
                                            .flex_grow()
                                            .relative()
                                            .tab_stop(sink_available)
                                            .when(sink_available, |element| {
                                                element.cursor(CursorStyle::PointingHand)
                                            })
                                            .when(!sink_available, |element| element.opacity(0.4))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(
                                                    move |this, event: &MouseDownEvent, _, cx| {
                                                        if !sink_available {
                                                            return;
                                                        }
                                                        this.dragging_volume = true;
                                                        this.change_volume_at(event.position, cx);
                                                    },
                                                ),
                                            )
                                            .on_mouse_move(cx.listener(
                                                move |this, event: &MouseMoveEvent, _, cx| {
                                                    if this.dragging_volume {
                                                        this.change_volume_at(event.position, cx);
                                                    }
                                                },
                                            ))
                                            .on_key_down(cx.listener(
                                                move |this, event: &KeyDownEvent, _, cx| {
                                                    match event.keystroke.key.as_str() {
                                                        "left" | "down" => {
                                                            this.set_volume(slider_volume - 5, cx)
                                                        }
                                                        "right" | "up" => {
                                                            this.set_volume(slider_volume + 5, cx)
                                                        }
                                                        _ => {}
                                                    }
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .h(px(4.0))
                                                    .w_full()
                                                    .bg(C::border())
                                                    .rounded(px(3.0))
                                                    .child(
                                                        div()
                                                            .h(px(4.0))
                                                            .w(relative(percentage / 100.0))
                                                            .bg(C::primary())
                                                            .rounded(px(3.0)),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .absolute()
                                                    .left(px(thumb_left))
                                                    .w(px(10.0))
                                                    .h(px(10.0))
                                                    .rounded(px(5.0))
                                                    .bg(C::fg()),
                                            ),
                                    ),
                            ),
                    );
            } else {
                let message = self.sound.error.clone().unwrap_or_else(|| {
                    if connected {
                        "No system audio output is available for this device.".into()
                    } else {
                        "Connect this device to access system audio controls.".into()
                    }
                });
                audio_section = audio_section.child(
                    div()
                        .font_family("Inter")
                        .text_size(px(11.0))
                        .line_height(px(17.0))
                        .text_color(C::muted())
                        .child(message),
                );
            }
            content = content.child(audio_section).child(divider());
        }

        let trusted_id = device.id.clone();
        let trusted = device.trusted;
        content = content
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(20.0))
                    .flex_shrink_0()
                    .child(preference(
                        "Trusted device",
                        "Allow connections without authorization",
                        trusted,
                        busy || !can_control,
                        "trusted-device",
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let trusted_id = trusted_id.clone();
                            this.bluetooth_action(
                                "Changing device trust",
                                BluetoothCommand::SetTrusted(trusted_id, !trusted),
                                None,
                                cx,
                            );
                        }),
                    ))
                    .child(info_row(
                        "Paired",
                        if device.paired { "Yes" } else { "No" },
                        C::fg(),
                        11.0,
                    )),
            )
            .child(divider())
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.0))
                    .flex_shrink_0()
                    .child(info_row(
                        "Signal strength",
                        device
                            .rssi
                            .map(|rssi| format!("{rssi} dBm"))
                            .unwrap_or_else(|| "Not reported".into()),
                        if device.rssi.is_some() {
                            C::green()
                        } else {
                            C::muted()
                        },
                        10.0,
                    ))
                    .child(info_row(
                        "Connected for",
                        if connected {
                            match device.connected_since {
                                Some(since) => {
                                    let minutes = now_millis().saturating_sub(since) / 60_000;
                                    format!("{minutes} minutes")
                                }
                                None => "Not reported".into(),
                            }
                        } else {
                            "—".into()
                        },
                        C::fg(),
                        10.0,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(t("Device address", 10.0, C::muted(), 400.0))
                            .child(m(device.address.clone(), 10.0, C::fg())),
                    ),
            )
            .child(div().flex_grow().min_h(px(20.0)));

        let connection_label = if connected {
            "Disconnect"
        } else if !device.paired {
            "Pair device"
        } else {
            "Connect"
        };
        let connection_id = device.id.clone();
        let connection_name = device.name.clone();
        let paired = device.paired;
        let forget = device.paired;
        content = content.child(
            div()
                .flex()
                .flex_col()
                .gap(px(14.0))
                .items_center()
                .flex_shrink_0()
                .child(button(
                    "connection-action",
                    ButtonOptions {
                        full_width: true,
                        disabled: busy || !can_control || !power,
                        ..ButtonOptions::default()
                    },
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        if connected {
                            this.bluetooth_action(
                                format!("Disconnecting {connection_name}"),
                                BluetoothCommand::Disconnect(connection_id.clone()),
                                None,
                                cx,
                            );
                        } else if !paired {
                            this.open_pair(Some(connection_id.clone()), cx);
                        } else {
                            this.bluetooth_action(
                                format!("Connecting {connection_name}"),
                                BluetoothCommand::Connect(connection_id.clone()),
                                None,
                                cx,
                            );
                        }
                    }),
                    t(connection_label, 13.0, C::fg(), 500.0),
                ))
                .when(forget, |element| {
                    element.child(button(
                        "forget-device",
                        ButtonOptions {
                            transparent: true,
                            border: Some(false),
                            height: Some(14.0),
                            padding_x: Some(0.0),
                            disabled: busy,
                            ..ButtonOptions::default()
                        },
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.dialog = Some(Dialog::Forget);
                            cx.notify();
                        }),
                        t("Forget device", 11.0, C::red(), 400.0),
                    ))
                }),
        );
        inspector.child(content).into_any_element()
    }
}

impl Render for BluetApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.window_width = f32::from(window.bounds().size.width);
        self.window_height = f32::from(window.bounds().size.height);

        if self.focus_prompt {
            self.focus_prompt = false;
            cx.defer_in(window, |this, window, cx| {
                let handle = this.code_input.read(cx).focus_handle(cx);
                window.focus(&handle);
            });
        }
        if self.focus_root {
            self.focus_root = false;
            let handle = self.root_focus.clone();
            cx.defer_in(window, move |_, window, _| {
                window.focus(&handle);
            });
        }

        let titlebar = self.render_titlebar(cx);
        let sidebar = self.render_sidebar(cx);
        let main = self.render_main(cx);
        let inspector = self.render_inspector(cx);

        let mut root = div()
            .id("bluet-app")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(C::bg())
            .overflow_hidden()
            .text_color(C::fg())
            .track_focus(&self.root_focus)
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_root_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::commit_volume))
            .on_key_down(cx.listener(Self::on_root_key_down))
            .child(titlebar)
            .child(
                div()
                    .flex()
                    .flex_grow()
                    .min_h(px(0.0))
                    .child(sidebar)
                    .child(main)
                    .child(inspector),
            );

        if self.menu && self.selected_device().is_some() {
            root = root.child(self.render_menu(cx));
        }

        if !self.toast.is_empty() {
            root = root.child(self.render_toast());
        }

        if self.dropdown.is_some() {
            root = root.child(self.render_dropdown(cx));
        }

        if let Some(dialog) = self.dialog {
            root = root.child(self.render_dialog_backdrop(cx));
            root = root.child(match dialog {
                Dialog::Pair => self.render_pair_dialog(cx),
                Dialog::Forget => self.render_forget_dialog(cx),
            });
        }

        if self.bt.prompt.is_some() {
            root = root.child(self.render_prompt_backdrop(cx));
            root = root.child(self.render_prompt(cx));
        }

        root
    }
}

impl BluetApp {
    fn render_menu(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(device) = self.selected_device().cloned() else {
            return div().into_any_element();
        };
        let bounds = self.menu_bounds();
        let connected = device.status == DeviceStatus::Connected;
        let paired = device.paired;
        let id = device.id.clone();
        let name = device.name.clone();
        let disabled = self.busy() || !self.power();
        let busy = self.busy();
        let mut menu = div()
            .id("device-menu")
            .relative()
            .flex()
            .flex_col()
            .absolute()
            .top(bounds.origin.y)
            .left(bounds.origin.x)
            .w(px(200.0))
            .p(px(6.0))
            .rounded(px(7.0))
            .bg(C::raised())
            .border_1()
            .border_color(C::border())
            .shadow(shadow(8.0, 24.0, 0.38));
        let label = if connected {
            "Disconnect"
        } else if !paired {
            "Pair device"
        } else {
            "Connect"
        };
        menu = menu.child(text_button(
            "menu-connection",
            label,
            ButtonOptions {
                transparent: true,
                border: Some(false),
                disabled,
                justify_start: true,
                ..ButtonOptions::default()
            },
            cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.menu = false;
                if connected {
                    this.bluetooth_action(
                        format!("Disconnecting {name}"),
                        BluetoothCommand::Disconnect(id.clone()),
                        None,
                        cx,
                    );
                } else if !paired {
                    this.open_pair(Some(id.clone()), cx);
                } else {
                    this.bluetooth_action(
                        format!("Connecting {name}"),
                        BluetoothCommand::Connect(id.clone()),
                        None,
                        cx,
                    );
                }
            }),
        ));
        if paired {
            menu = menu.child(text_button(
                "menu-forget",
                "Forget device",
                ButtonOptions {
                    transparent: true,
                    border: Some(false),
                    disabled: busy,
                    justify_start: true,
                    ..ButtonOptions::default()
                },
                cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.menu = false;
                    this.dialog = Some(Dialog::Forget);
                    cx.notify();
                }),
            ));
        }
        menu.into_any_element()
    }

    fn render_toast(&self) -> AnyElement {
        div()
            .id("toast")
            .flex()
            .items_center()
            .absolute()
            .bottom(px(24.0))
            .left(px(252.0))
            .p(px(14.0))
            .gap(px(10.0))
            .border_1()
            .border_color(C::border())
            .rounded(px(8.0))
            .bg(C::raised())
            .child(icon(Icon::CircleCheck, 17.0, C::green()))
            .child(t(self.toast.clone(), 12.0, C::fg(), 400.0))
            .into_any_element()
    }

    fn render_dialog_backdrop(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("dialog-backdrop")
            .absolute()
            .top(px(44.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .bg(C::dialog_backdrop())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if !this.in_modal_card(event.position) {
                        cx.stop_propagation();
                    }
                }),
            )
            .into_any_element()
    }

    fn render_prompt_backdrop(&self, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("dialog-backdrop")
            .absolute()
            .top(px(44.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .bg(C::prompt_backdrop())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if !this.in_modal_card(event.position) {
                        cx.stop_propagation();
                    }
                }),
            )
            .into_any_element()
    }

    fn in_modal_card(&self, position: Point<Pixels>) -> bool {
        let center_y = 44.0 + (self.window_height - 44.0) / 2.0;
        (f32::from(position.x) - self.window_width / 2.0).abs() <= 240.0
            && (f32::from(position.y) - center_y).abs() <= 330.0
    }

    fn render_pair_dialog(&mut self, cx: &mut Context<Self>) -> Div {
        let nearby: Vec<Device> = self
            .bt
            .devices
            .iter()
            .filter(|device| {
                !device.paired
                    && filter_devices(
                        std::slice::from_ref(device),
                        DeviceFilter::All,
                        &self.pair_query,
                    )
                    .len()
                        == 1
            })
            .cloned()
            .collect();
        let failure = self.error.clone().or_else(|| self.bt.error.clone());
        let visible = self.visible();
        let adapter_name = self
            .adapter()
            .map(|adapter| adapter.name.clone())
            .unwrap_or_default();
        let discovering = self
            .adapter()
            .map(|adapter| adapter.discovering)
            .unwrap_or(false);
        let pending = self.pending.clone();
        let busy = self.busy();
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(10.0))
            .max_h(px(300.0))
            .id("pair-device-list")
            .overflow_y_scroll()
            .scrollbar_width(px(3.0))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pb(px(4.0))
                    .child(m(
                        format!("NEARBY DEVICES · {}", nearby.len()),
                        10.0,
                        C::muted(),
                    ))
                    .when(discovering, |element| element.child(radar("Scanning…")))
                    .when(!discovering, |element| {
                        element.child(t("Discovery stopped", 11.0, C::muted(), 400.0))
                    }),
            );
        for device in &nearby {
            let selected = self.pair_id.as_ref() == Some(&device.id);
            let id = device.id.clone();
            let row_id = SharedString::from(format!("pair-option-{}", device.id));
            list = list.child(
                div()
                    .id(row_id)
                    .flex()
                    .items_center()
                    .h(px(76.0))
                    .flex_shrink_0()
                    .px(px(16.0))
                    .gap(px(14.0))
                    .rounded(px(7.0))
                    .border_1()
                    .border_color(if selected { C::primary() } else { C::border() })
                    .bg(if selected { C::selection() } else { C::panel() })
                    .cursor(CursorStyle::PointingHand)
                    .tab_stop(true)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.pair_id = Some(id.clone());
                        cx.notify();
                    }))
                    .on_key_down({
                        let id = device.id.clone();
                        cx.listener(move |this, event: &KeyDownEvent, _, cx| {
                            if event.keystroke.key == "enter" {
                                this.pair_id = Some(id.clone());
                                cx.notify();
                            }
                        })
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(42.0))
                            .h(px(42.0))
                            .rounded(px(8.0))
                            .bg(C::raised())
                            .child(icon(
                                kind_icon(device.kind),
                                22.0,
                                if selected { C::accent() } else { C::muted() },
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.0))
                            .flex_grow()
                            .child(t(device.name.clone(), 13.0, C::fg(), 500.0))
                            .child(t(device.description.clone(), 11.0, C::muted(), 400.0)),
                    )
                    .child(icon(
                        if selected {
                            Icon::CircleCheck
                        } else {
                            Icon::Circle
                        },
                        18.0,
                        if selected { C::accent() } else { C::subtle() },
                    )),
            );
        }
        if nearby.is_empty() {
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .p(px(20.0))
                    .gap(px(8.0))
                    .items_center()
                    .child(t("No nearby devices found", 13.0, C::muted(), 400.0))
                    .child(t(
                        if discovering {
                            "Searching for devices in pairing mode…"
                        } else {
                            "Use Add device again to restart discovery."
                        },
                        11.0,
                        C::subtle(),
                        400.0,
                    )),
            );
        }

        let pair_valid = self.pair_id.as_ref().is_some_and(|id| {
            self.bt
                .devices
                .iter()
                .any(|device| &device.id == id && !device.paired)
        });
        div()
            .absolute()
            .top(px(44.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("pair-dialog")
                    .relative()
                    .flex()
                    .flex_col()
                    .w(px(480.0))
                    .p(px(28.0))
                    .gap(px(24.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(C::border())
                    .bg(linear_gradient(
                        120.0,
                        linear_color_stop(C::panel(), 0.0),
                        linear_color_stop(C::dialog_tint(), 1.0),
                    ))
                    .shadow(shadow(16.0, 56.0, 0.25))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(dither_layer(
                        DitherVariant::Ambient,
                        1.0,
                        0.9,
                        12.0,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(t("Add a device", 21.0, C::fg(), 600.0))
                            .child(icon_button(
                                "dialog-close",
                                Icon::X,
                                "Close dialog",
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.close_dialog(cx);
                                }),
                            )),
                    )
                    .child(t(
                        "Make sure your device is turned on and in pairing mode.",
                        13.0,
                        C::muted(),
                        400.0,
                    ))
                    .child(
                        div()
                            .id("pair-search")
                            .child(search_field(self.pair_search.clone(), None)),
                    )
                    .child(list)
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap(px(10.0))
                            .child(icon(Icon::Info, 15.0, C::muted()))
                            .child(
                                div()
                                    .flex_grow()
                                    .font_family("Inter")
                                    .text_size(px(11.0))
                                    .line_height(px(16.5))
                                    .text_color(C::muted())
                                    .child(format!(
                                        "Your computer is {}. Discovery does not change your visibility setting.",
                                        if visible {
                                            format!("visible as {adapter_name}")
                                        } else {
                                            "hidden from nearby devices".to_string()
                                        }
                                    )),
                            ),
                    )
                    .when(failure.is_some(), |element| {
                        element.child(
                            div()
                                .font_family("Inter")
                                .text_size(px(12.0))
                                .line_height(px(18.0))
                                .text_color(C::red())
                                .child(failure.unwrap_or_default()),
                        )
                    })
                    .when(pending.is_some(), |element| {
                        element.child(t(
                            format!("{}…", pending.unwrap_or_default()),
                            12.0,
                            C::accent(),
                            400.0,
                        ))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .border_t_1()
                            .border_color(C::border())
                            .pt(px(18.0))
                            .gap(px(10.0))
                            .justify_end()
                            .child(text_button(
                                "pair-cancel",
                                "Cancel",
                                ButtonOptions::default(),
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.close_dialog(cx);
                                }),
                            ))
                            .child(button(
                                "pair-confirm",
                                ButtonOptions {
                                    primary: true,
                                    border: Some(false),
                                    disabled: busy || !pair_valid,
                                    ..ButtonOptions::default()
                                },
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.pair_selected(cx);
                                }),
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.0))
                                    .child(icon(Icon::Link, 15.0, gpui::white()))
                                    .child(t("Pair device", 13.0, C::fg(), 600.0)),
                            )),
                    ),
            )
    }

    fn render_forget_dialog(&mut self, cx: &mut Context<Self>) -> Div {
        let failure = self.error.clone().or_else(|| self.bt.error.clone());
        let device = self.selected_device().cloned();
        let name = device
            .as_ref()
            .map(|device| device.name.clone())
            .unwrap_or_default();
        let busy = self.busy();
        let disabled = busy || device.is_none();
        div()
            .absolute()
            .top(px(44.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("forget-dialog")
                    .relative()
                    .flex()
                    .flex_col()
                    .w(px(480.0))
                    .p(px(28.0))
                    .gap(px(24.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(C::border())
                    .bg(linear_gradient(
                        120.0,
                        linear_color_stop(C::panel(), 0.0),
                        linear_color_stop(C::dialog_tint(), 1.0),
                    ))
                    .shadow(shadow(16.0, 56.0, 0.25))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(dither_layer(
                        DitherVariant::Ambient,
                        1.0,
                        0.9,
                        12.0,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(t("Forget device?", 21.0, C::fg(), 600.0))
                            .child(icon_button(
                                "forget-close",
                                Icon::X,
                                "Close dialog",
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.close_dialog(cx);
                                }),
                            )),
                    )
                    .child(t(
                        format!(
                            "Remove {name} from your system's paired devices? This removes its saved pairing keys."
                        ),
                        13.0,
                        C::muted(),
                        400.0,
                    ))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.0))
                            .justify_end()
                            .child(text_button(
                                "forget-cancel",
                                "Cancel",
                                ButtonOptions::default(),
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.close_dialog(cx);
                                }),
                            ))
                            .child(button(
                                "forget-confirm",
                                ButtonOptions {
                                    background: Some(C::forget_bg()),
                                    border: Some(false),
                                    disabled,
                                    ..ButtonOptions::default()
                                },
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.forget_device(cx);
                                }),
                                t("Forget device", 13.0, C::fg(), 500.0),
                            )),
                    )
                    .when(failure.is_some(), |element| {
                        element.child(
                            div()
                                .font_family("Inter")
                                .text_size(px(12.0))
                                .line_height(px(18.0))
                                .text_color(C::red())
                                .child(failure.unwrap_or_default()),
                        )
                    }),
            )
    }

    fn render_prompt(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(prompt) = self.bt.prompt.clone() else {
            return div();
        };
        let failure = self.error.clone().or_else(|| self.bt.error.clone());
        let code = self.code_input.read(cx).content().to_string();
        let accepts_text = matches!(prompt.kind, PromptKind::Pin | PromptKind::Passkey);
        let valid = match prompt.kind {
            PromptKind::Pin => !code.is_empty(),
            PromptKind::Passkey => {
                !code.is_empty()
                    && code.len() <= 6
                    && code.chars().all(|character| character.is_ascii_digit())
            }
            _ => true,
        };
        let button_label = match prompt.kind {
            PromptKind::Confirm => "Confirm",
            PromptKind::Authorize => "Authorize",
            _ => "Continue",
        };
        div()
            .absolute()
            .top(px(44.0))
            .left(px(0.0))
            .right(px(0.0))
            .bottom(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .id("pairing-prompt")
                    .relative()
                    .flex()
                    .flex_col()
                    .w(px(480.0))
                    .p(px(28.0))
                    .gap(px(22.0))
                    .rounded(px(12.0))
                    .border_1()
                    .border_color(C::primary())
                    .bg(C::panel())
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(dither_layer(DitherVariant::Ambient, 1.0, 0.9, 12.0))
                    .child(t("Bluetooth pairing request", 21.0, C::fg(), 600.0))
                    .child(t(prompt.message.clone(), 13.0, C::muted(), 400.0))
                    .when(prompt.passkey.is_some(), |element| {
                        element.child(m(prompt.passkey.clone().unwrap_or_default(), 30.0, C::fg()))
                    })
                    .when(accepts_text, |element| {
                        element.child(
                            div()
                                .flex()
                                .items_center()
                                .h(px(42.0))
                                .px(px(12.0))
                                .bg(C::raised())
                                .border_1()
                                .border_color(C::border())
                                .rounded(px(6.0))
                                .child(
                                    div().id("pairing-code").child(self.code_input.clone()),
                                ),
                        )
                    })
                    .when(failure.is_some(), |element| {
                        element.child(
                            div()
                                .font_family("Inter")
                                .text_size(px(12.0))
                                .line_height(px(18.0))
                                .text_color(C::red())
                                .child(failure.unwrap_or_default()),
                        )
                    })
                    .when(prompt.entered.is_some(), |element| {
                        element.child(t(
                            format!(
                                "{} digits entered on the device",
                                prompt.entered.unwrap_or(0)
                            ),
                            12.0,
                            C::muted(),
                            400.0,
                        ))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap(px(10.0))
                            .child(text_button(
                                "pairing-reject",
                                "Cancel",
                                ButtonOptions::default(),
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.reject_prompt(cx);
                                }),
                            ))
                            .when(prompt.kind != PromptKind::Display, |element| {
                                element.child(button(
                                    "pairing-accept",
                                    ButtonOptions {
                                        primary: true,
                                        border: Some(false),
                                        disabled: !valid,
                                        ..ButtonOptions::default()
                                    },
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.accept_prompt(cx);
                                    }),
                                    t(button_label, 13.0, C::fg(), 600.0),
                                ))
                            }),
                    ),
            )
    }

    fn render_dropdown(&mut self, cx: &mut Context<Self>) -> Div {
        let Some(dropdown) = self.dropdown.clone() else {
            return div();
        };
        let Some(bounds) = self.dropdown_bounds() else {
            return div();
        };
        let mut menu = div()
            .relative()
            .flex()
            .flex_col()
            .absolute()
            .top(bounds.origin.y)
            .left(bounds.origin.x)
            .w(px(dropdown.width))
            .p(px(4.0))
            .gap(px(4.0))
            .rounded(px(6.0))
            .bg(C::raised())
            .border_1()
            .border_color(C::border())
            .shadow(shadow(8.0, 24.0, 0.38));
        match &dropdown.kind {
            DropdownKind::AudioProfile { address } => {
                let profiles = self
                    .sound
                    .devices
                    .get(&address.to_ascii_uppercase())
                    .map(|device| device.profiles.clone())
                    .unwrap_or_default();
                for profile in profiles {
                    let address = address.clone();
                    let profile_id = profile.id.clone();
                    menu = menu.child(text_button(
                        SharedString::from(format!("profile-{}", profile.id)),
                        short_profile_label(&profile.label),
                        ButtonOptions {
                            transparent: true,
                            border: Some(false),
                            height: Some(36.0),
                            padding_x: Some(10.0),
                            justify_start: true,
                            hover_background: Some(C::selection()),
                            ..ButtonOptions::default()
                        },
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.dropdown = None;
                            let audio = this.audio.clone();
                            let address = address.clone();
                            let profile = profile_id.clone();
                            this.run_future(
                                "Changing audio profile",
                                async move { audio.set_profile(&address, &profile).await },
                                None,
                                cx,
                            );
                        }),
                    ));
                }
            }
            DropdownKind::Adapter => {
                let adapters = self.bt.adapters.clone();
                for adapter in adapters {
                    let id = adapter.id.clone();
                    menu = menu.child(text_button(
                        SharedString::from(format!("adapter-{}", adapter.id)),
                        adapter.name,
                        ButtonOptions {
                            transparent: true,
                            border: Some(false),
                            height: Some(36.0),
                            padding_x: Some(10.0),
                            justify_start: true,
                            hover_background: Some(C::selection()),
                            ..ButtonOptions::default()
                        },
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.dropdown = None;
                            this.bluetooth.select_adapter(id.clone());
                            cx.notify();
                        }),
                    ));
                }
            }
        }
        menu
    }
}
