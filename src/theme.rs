use gpui::{Hsla, rgb};

pub struct C;

impl C {
    pub fn bg() -> Hsla {
        rgb(0x111317).into()
    }
    pub fn main_tint() -> Hsla {
        rgb(0x121820).into()
    }
    pub fn sidebar() -> Hsla {
        rgb(0x16181D).into()
    }
    pub fn panel() -> Hsla {
        rgb(0x181B21).into()
    }
    pub fn inspector_tint() -> Hsla {
        rgb(0x1B222D).into()
    }
    pub fn dialog_tint() -> Hsla {
        rgb(0x1B2533).into()
    }
    pub fn raised() -> Hsla {
        rgb(0x22262E).into()
    }
    pub fn raised_hover() -> Hsla {
        rgb(0x2B303A).into()
    }
    pub fn border() -> Hsla {
        rgb(0x2B3039).into()
    }
    pub fn fg() -> Hsla {
        rgb(0xF0F2F6).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x969EAE).into()
    }
    pub fn subtle() -> Hsla {
        rgb(0x7D8798).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x5B92F5).into()
    }
    pub fn primary() -> Hsla {
        rgb(0x3370DA).into()
    }
    pub fn primary_hover() -> Hsla {
        rgb(0x3C7CE9).into()
    }
    pub fn selection() -> Hsla {
        rgb(0x1C2C45).into()
    }
    pub fn selection_hover() -> Hsla {
        rgb(0x233755).into()
    }
    pub fn green() -> Hsla {
        rgb(0x75D9A3).into()
    }
    pub fn green_bg() -> Hsla {
        rgb(0x193328).into()
    }
    pub fn red() -> Hsla {
        rgb(0xF29399).into()
    }
    pub fn error_bg() -> Hsla {
        rgb(0x321E26).into()
    }
    pub fn error_border() -> Hsla {
        rgb(0x6D3B43).into()
    }
    pub fn forget_bg() -> Hsla {
        rgb(0x823F4C).into()
    }
    pub fn row_hover() -> Hsla {
        rgb(0x191E26).into()
    }
    pub fn halo_bg() -> Hsla {
        rgb(0x1C2D46).into()
    }
    pub fn halo_border() -> Hsla {
        rgb(0x20314B).into()
    }
    pub fn halo_inner() -> Hsla {
        rgb(0x2A4672).into()
    }
    pub fn meter_fill() -> Hsla {
        rgb(0x315D49).into()
    }
    pub fn toggle_off() -> Hsla {
        rgb(0x3C424D).into()
    }
    pub fn dialog_backdrop() -> Hsla {
        let color: Hsla = rgb(0x06080D).into();
        color.opacity(0.72)
    }
    pub fn prompt_backdrop() -> Hsla {
        let color: Hsla = rgb(0x06080D).into();
        color.opacity(0.91)
    }
    pub fn shadow(alpha: f32) -> Hsla {
        gpui::black().opacity(alpha)
    }
}
