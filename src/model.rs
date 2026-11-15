use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum DeviceKind {
    Headphones,
    Mouse,
    Keyboard,
    Phone,
    Speaker,
    Gamepad,
    #[default]
    Device,
}

impl DeviceKind {
    pub fn title(self) -> &'static str {
        match self {
            Self::Headphones => "Headphones",
            Self::Mouse => "Mouse",
            Self::Keyboard => "Keyboard",
            Self::Phone => "Phone",
            Self::Speaker => "Speaker",
            Self::Gamepad => "Game controller",
            Self::Device => "Bluetooth device",
        }
    }
}

impl fmt::Display for DeviceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.title())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum DeviceStatus {
    Connected,
    Saved,
    #[default]
    Nearby,
}

impl DeviceStatus {
    pub fn title(self) -> &'static str {
        match self {
            Self::Connected => "Connected",
            Self::Saved => "Saved",
            Self::Nearby => "Nearby",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub brand: String,
    pub kind: DeviceKind,
    pub description: String,
    pub status: DeviceStatus,
    pub battery: Option<u8>,
    pub address: String,
    pub paired: bool,
    pub trusted: bool,
    pub rssi: Option<i16>,
    pub adapter_id: String,
    pub connected_since: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum DeviceFilter {
    #[default]
    All,
    Connected,
    Saved,
}

impl DeviceFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Connected, Self::Saved];

    pub fn key(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Connected => "connected",
            Self::Saved => "saved",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::All => "All devices",
            Self::Connected => "Connected",
            Self::Saved => "Saved",
        }
    }
}

/// Combine a status filter with a case-insensitive search over the fields shown
/// in the device list.
pub fn filter_devices(devices: &[Device], filter: DeviceFilter, query: &str) -> Vec<Device> {
    let term = query.trim().to_lowercase();
    devices
        .iter()
        .filter(|device| {
            let status_matches = match filter {
                DeviceFilter::All => true,
                DeviceFilter::Connected => device.status == DeviceStatus::Connected,
                DeviceFilter::Saved => device.status == DeviceStatus::Saved,
            };
            let term_matches = term.is_empty()
                || [
                    device.name.as_str(),
                    device.brand.as_str(),
                    device.kind.title(),
                    device.description.as_str(),
                    device.address.as_str(),
                ]
                .iter()
                .any(|value| value.to_lowercase().contains(&term));
            status_matches && term_matches
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(id: &str, name: &str, status: DeviceStatus, address: &str) -> Device {
        Device {
            id: id.into(),
            name: name.into(),
            brand: String::new(),
            kind: DeviceKind::Headphones,
            description: "Headphones".into(),
            status,
            battery: None,
            address: address.into(),
            paired: true,
            trusted: true,
            rssi: None,
            adapter_id: "/adapter".into(),
            connected_since: None,
        }
    }

    #[test]
    fn combines_status_and_case_insensitive_search() {
        let devices = vec![
            device("/a", "Headset", DeviceStatus::Connected, "AA:BB"),
            device("/b", "Keyboard", DeviceStatus::Saved, "CC:DD"),
        ];
        let ids = |list: Vec<Device>| list.into_iter().map(|d| d.id).collect::<Vec<_>>();
        assert_eq!(
            ids(filter_devices(&devices, DeviceFilter::Saved, " KEY ")),
            ["/b"]
        );
        assert_eq!(
            ids(filter_devices(&devices, DeviceFilter::Connected, "cc:dd")),
            Vec::<String>::new()
        );
        assert_eq!(
            ids(filter_devices(&devices, DeviceFilter::All, "aa:bb")),
            ["/a"]
        );
    }
}
