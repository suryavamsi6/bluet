use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    Bluetooth,
    BluetoothOff,
    ArrowLeftRight,
    Clock3,
    Settings2,
    Laptop,
    Plus,
    Search,
    Headphones,
    Mouse,
    Keyboard,
    Smartphone,
    Speaker,
    Gamepad2,
    BatteryMedium,
    Ellipsis,
    Radar,
    EyeOff,
    Eye,
    ChevronDown,
    ChevronRight,
    Volume2,
    VolumeX,
    Minus,
    Square,
    X,
    Link,
    CircleCheck,
    Circle,
    Info,
    Check,
    Folder,
    File,
    ArrowUpRight,
    ShieldCheck,
    RefreshCw,
}

impl Icon {
    pub fn path(self) -> &'static str {
        match self {
            Self::Bluetooth => "icons/bluetooth.svg",
            Self::BluetoothOff => "icons/bluetooth-off.svg",
            Self::ArrowLeftRight => "icons/arrow-left-right.svg",
            Self::Clock3 => "icons/clock-3.svg",
            Self::Settings2 => "icons/settings-2.svg",
            Self::Laptop => "icons/laptop.svg",
            Self::Plus => "icons/plus.svg",
            Self::Search => "icons/search.svg",
            Self::Headphones => "icons/headphones.svg",
            Self::Mouse => "icons/mouse.svg",
            Self::Keyboard => "icons/keyboard.svg",
            Self::Smartphone => "icons/smartphone.svg",
            Self::Speaker => "icons/speaker.svg",
            Self::Gamepad2 => "icons/gamepad-2.svg",
            Self::BatteryMedium => "icons/battery-medium.svg",
            Self::Ellipsis => "icons/ellipsis.svg",
            Self::Radar => "icons/radar.svg",
            Self::EyeOff => "icons/eye-off.svg",
            Self::Eye => "icons/eye.svg",
            Self::ChevronDown => "icons/chevron-down.svg",
            Self::ChevronRight => "icons/chevron-right.svg",
            Self::Volume2 => "icons/volume-2.svg",
            Self::VolumeX => "icons/volume-x.svg",
            Self::Minus => "icons/minus.svg",
            Self::Square => "icons/square.svg",
            Self::X => "icons/x.svg",
            Self::Link => "icons/link.svg",
            Self::CircleCheck => "icons/circle-check.svg",
            Self::Circle => "icons/circle.svg",
            Self::Info => "icons/info.svg",
            Self::Check => "icons/check.svg",
            Self::Folder => "icons/folder.svg",
            Self::File => "icons/file.svg",
            Self::ArrowUpRight => "icons/arrow-up-right.svg",
            Self::ShieldCheck => "icons/shield-check.svg",
            Self::RefreshCw => "icons/refresh-cw.svg",
        }
    }
}

const ICONS: &[(&str, &[u8])] = &[
    (
        "icons/bluetooth.svg",
        include_bytes!("../assets/icons/bluetooth.svg"),
    ),
    (
        "icons/bluetooth-off.svg",
        include_bytes!("../assets/icons/bluetooth-off.svg"),
    ),
    (
        "icons/arrow-left-right.svg",
        include_bytes!("../assets/icons/arrow-left-right.svg"),
    ),
    (
        "icons/clock-3.svg",
        include_bytes!("../assets/icons/clock-3.svg"),
    ),
    (
        "icons/settings-2.svg",
        include_bytes!("../assets/icons/settings-2.svg"),
    ),
    (
        "icons/laptop.svg",
        include_bytes!("../assets/icons/laptop.svg"),
    ),
    ("icons/plus.svg", include_bytes!("../assets/icons/plus.svg")),
    (
        "icons/search.svg",
        include_bytes!("../assets/icons/search.svg"),
    ),
    (
        "icons/headphones.svg",
        include_bytes!("../assets/icons/headphones.svg"),
    ),
    (
        "icons/mouse.svg",
        include_bytes!("../assets/icons/mouse.svg"),
    ),
    (
        "icons/keyboard.svg",
        include_bytes!("../assets/icons/keyboard.svg"),
    ),
    (
        "icons/smartphone.svg",
        include_bytes!("../assets/icons/smartphone.svg"),
    ),
    (
        "icons/speaker.svg",
        include_bytes!("../assets/icons/speaker.svg"),
    ),
    (
        "icons/gamepad-2.svg",
        include_bytes!("../assets/icons/gamepad-2.svg"),
    ),
    (
        "icons/battery-medium.svg",
        include_bytes!("../assets/icons/battery-medium.svg"),
    ),
    (
        "icons/ellipsis.svg",
        include_bytes!("../assets/icons/ellipsis.svg"),
    ),
    (
        "icons/radar.svg",
        include_bytes!("../assets/icons/radar.svg"),
    ),
    (
        "icons/eye-off.svg",
        include_bytes!("../assets/icons/eye-off.svg"),
    ),
    ("icons/eye.svg", include_bytes!("../assets/icons/eye.svg")),
    (
        "icons/chevron-down.svg",
        include_bytes!("../assets/icons/chevron-down.svg"),
    ),
    (
        "icons/chevron-right.svg",
        include_bytes!("../assets/icons/chevron-right.svg"),
    ),
    (
        "icons/volume-2.svg",
        include_bytes!("../assets/icons/volume-2.svg"),
    ),
    (
        "icons/volume-x.svg",
        include_bytes!("../assets/icons/volume-x.svg"),
    ),
    (
        "icons/minus.svg",
        include_bytes!("../assets/icons/minus.svg"),
    ),
    (
        "icons/square.svg",
        include_bytes!("../assets/icons/square.svg"),
    ),
    ("icons/x.svg", include_bytes!("../assets/icons/x.svg")),
    ("icons/link.svg", include_bytes!("../assets/icons/link.svg")),
    (
        "icons/circle-check.svg",
        include_bytes!("../assets/icons/circle-check.svg"),
    ),
    (
        "icons/circle.svg",
        include_bytes!("../assets/icons/circle.svg"),
    ),
    ("icons/info.svg", include_bytes!("../assets/icons/info.svg")),
    (
        "icons/check.svg",
        include_bytes!("../assets/icons/check.svg"),
    ),
    (
        "icons/folder.svg",
        include_bytes!("../assets/icons/folder.svg"),
    ),
    ("icons/file.svg", include_bytes!("../assets/icons/file.svg")),
    (
        "icons/arrow-up-right.svg",
        include_bytes!("../assets/icons/arrow-up-right.svg"),
    ),
    (
        "icons/shield-check.svg",
        include_bytes!("../assets/icons/shield-check.svg"),
    ),
    (
        "icons/refresh-cw.svg",
        include_bytes!("../assets/icons/refresh-cw.svg"),
    ),
];

pub struct BluetAssets;

impl AssetSource for BluetAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(ICONS
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, bytes)| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(ICONS
            .iter()
            .filter(|(name, _)| name.starts_with(path))
            .map(|(name, _)| SharedString::from(*name))
            .collect())
    }
}
