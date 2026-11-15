use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use gpui::{
    Animation, AnimationExt, AnyElement, App, Bounds, BoxShadow, ClickEvent, Corners, CursorStyle,
    Div, Element, ElementId, Entity, FontWeight, GlobalElementId, Hsla, InspectorElementId,
    IntoElement, LayoutId, Pixels, RenderImage, SharedString, Style, Styled, Window, div, point,
    prelude::*, px, relative, svg,
};
use image::{Frame, RgbaImage};

use crate::dither::{DitherOptions, DitherVariant, create_dither_texture};
use crate::icons::Icon;
use crate::input::TextInput;
use crate::theme::C;

type ClickCallback = Arc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

pub fn t(content: impl Into<SharedString>, size: f32, color: Hsla, weight: f32) -> Div {
    div()
        .font_family("Inter")
        .text_size(px(size))
        .line_height(px(size * 1.3))
        .font_weight(FontWeight(weight))
        .text_color(color)
        .child(content.into())
}

pub fn m(content: impl Into<SharedString>, size: f32, color: Hsla) -> Div {
    div()
        .font_family("Geist Mono")
        .text_size(px(size))
        .line_height(px(size * 1.3))
        .text_color(color)
        .child(content.into())
}

pub fn icon(name: Icon, size: f32, color: Hsla) -> impl IntoElement {
    svg()
        .path(name.path())
        .text_color(color)
        .w(px(size))
        .h(px(size))
        .flex_shrink_0()
}

pub fn divider() -> Div {
    div().h(px(1.0)).w_full().flex_shrink_0().bg(C::border())
}

#[derive(Clone, Default)]
pub struct ButtonOptions {
    pub primary: bool,
    pub small: bool,
    pub disabled: bool,
    pub justify_start: bool,
    pub full_width: bool,
    pub height: Option<f32>,
    pub padding_x: Option<f32>,
    pub gap: Option<f32>,
    pub background: Option<Hsla>,
    pub transparent: bool,
    pub border: Option<bool>,
    pub hover_background: Option<Hsla>,
    pub font_size: Option<f32>,
}

pub fn button(
    id: impl Into<ElementId>,
    options: ButtonOptions,
    on_activate: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    content: impl IntoElement,
) -> AnyElement {
    let callback: ClickCallback = Arc::new(on_activate);
    let disabled = options.disabled;
    let primary = options.primary;
    let small = options.small;
    let height = options.height.unwrap_or(if small { 28.0 } else { 37.0 });
    let padding_x = options.padding_x.unwrap_or(if small { 12.0 } else { 16.0 });
    let base_background =
        options
            .background
            .unwrap_or(if primary { C::primary() } else { C::raised() });
    let hover_background = options.hover_background.unwrap_or(if primary {
        C::primary_hover()
    } else {
        C::raised_hover()
    });
    let border = options.border.unwrap_or(!primary);
    let gap = options.gap.unwrap_or(8.0);
    let content = if disabled {
        div().opacity(0.4).child(content).into_any_element()
    } else {
        content.into_any_element()
    };

    let mut element = div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(gap))
        .h(px(height))
        .px(px(padding_x))
        .flex_shrink_0()
        .rounded(px(6.0))
        .tab_stop(!disabled)
        .cursor(if disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .when(options.full_width, |element| element.w_full())
        .when(options.justify_start, |element| element.justify_start())
        .when(!options.justify_start, |element| element.justify_center())
        .when(!options.transparent, |element| element.bg(base_background))
        .when(border, |element| {
            element.border_1().border_color(C::border())
        });
    if !disabled {
        element = element
            .hover(move |style| style.bg(hover_background))
            .on_click({
                let callback = callback.clone();
                move |event, window, cx| callback(event, window, cx)
            })
            .on_key_down({
                let callback = callback.clone();
                move |event, window, cx| {
                    let key = event.keystroke.key.as_str();
                    if key == "enter" || key == "space" {
                        callback(&ClickEvent::default(), window, cx);
                    }
                }
            });
    }
    if primary {
        element = element.child(dither_layer(
            DitherVariant::Button,
            0.7,
            if disabled { 0.4 } else { 1.0 },
            6.0,
        ));
    }
    element.child(content).into_any_element()
}

pub fn text_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    options: ButtonOptions,
    on_activate: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let size = options
        .font_size
        .unwrap_or(if options.small { 11.0 } else { 13.0 });
    let weight = if options.primary { 600.0 } else { 500.0 };
    let label = label.into();
    button(
        id,
        options.clone(),
        on_activate,
        t(label, size, C::fg(), weight),
    )
}

pub fn icon_button(
    id: impl Into<ElementId>,
    name: Icon,
    label: &'static str,
    on_activate: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let element = button(
        id,
        ButtonOptions {
            height: Some(28.0),
            padding_x: Some(0.0),
            transparent: true,
            border: Some(false),
            ..ButtonOptions::default()
        },
        on_activate,
        icon(name, 16.0, C::muted()),
    );
    let _ = label;
    element
}

pub fn toggle(
    id: impl Into<ElementId>,
    value: bool,
    disabled: bool,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let callback: ClickCallback = Arc::new(on_toggle);
    let mut element = div()
        .id(id)
        .flex()
        .items_center()
        .w(px(34.0))
        .h(px(20.0))
        .p(px(3.0))
        .rounded(px(12.0))
        .flex_shrink_0()
        .tab_stop(!disabled)
        .cursor(if disabled {
            CursorStyle::Arrow
        } else {
            CursorStyle::PointingHand
        })
        .bg(if value { C::primary() } else { C::toggle_off() })
        .when(value, |element| element.justify_end())
        .when(!value, |element| element.justify_start())
        .child(
            div()
                .w(px(14.0))
                .h(px(14.0))
                .rounded(px(7.0))
                .bg(gpui::white()),
        )
        .when(disabled, |element| element.opacity(0.4));
    if !disabled {
        element = element
            .on_click({
                let callback = callback.clone();
                move |event, window, cx| callback(event, window, cx)
            })
            .on_key_down(move |event, window, cx| {
                let key = event.keystroke.key.as_str();
                if key == "enter" || key == "space" {
                    callback(&ClickEvent::default(), window, cx);
                }
            });
    }
    element.into_any_element()
}

pub fn search_field(input: Entity<TextInput>, width: Option<f32>) -> Div {
    let mut element = div()
        .flex()
        .items_center()
        .h(px(37.0))
        .px(px(12.0))
        .gap(px(10.0))
        .bg(C::panel())
        .border_1()
        .border_color(C::border())
        .rounded(px(6.0));
    element = match width {
        Some(width) => element.w(px(width)),
        None => element.w_full(),
    };
    element
        .child(icon(Icon::Search, 16.0, C::muted()))
        .child(div().flex_grow().h(px(30.0)).child(input))
}

pub fn empty_state(
    name: Icon,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    action: Option<AnyElement>,
) -> Div {
    let mut element = div()
        .flex()
        .flex_col()
        .flex_grow()
        .items_center()
        .justify_center()
        .gap(px(16.0))
        .p(px(24.0))
        .child(icon(name, 40.0, C::subtle()))
        .child(t(title, 20.0, C::fg(), 600.0))
        .child(
            div().text_center().max_w(px(420.0)).child(
                div()
                    .font_family("Inter")
                    .text_size(px(12.0))
                    .line_height(px(18.0))
                    .text_color(C::muted())
                    .child(description.into()),
            ),
        );
    if let Some(action) = action {
        element = element.child(action);
    }
    element
}

pub fn info_row(
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    color: Hsla,
    size: f32,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(t(label, size, C::muted(), 400.0))
        .child(t(value, size, color, 400.0))
}

pub fn preference(
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    value: bool,
    disabled: bool,
    id: impl Into<ElementId>,
    on_toggle: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let title = title.into();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.0))
                .flex_grow()
                .child(t(title.clone(), 12.0, C::fg(), 500.0))
                .child(t(subtitle, 10.0, C::muted(), 400.0)),
        )
        .child(toggle(id, value, disabled, on_toggle))
}

pub fn radar(label: impl Into<SharedString>) -> Div {
    let label = label.into();
    div()
        .flex()
        .items_center()
        .gap(px(6.0))
        .child(
            div()
                .child(icon(Icon::Radar, 13.0, C::accent()))
                .with_animation(
                    "radar-pulse",
                    Animation::new(Duration::from_millis(1800))
                        .repeat()
                        .with_easing(gpui::pulsating_between(0.4, 1.0)),
                    |element, delta| element.opacity(delta),
                ),
        )
        .child(t(label, 10.0, C::accent(), 400.0))
}

pub fn shadow(offset_y: f32, blur: f32, alpha: f32) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: C::shadow(alpha),
        offset: point(px(0.0), px(offset_y)),
        blur_radius: px(blur),
        spread_radius: px(0.0),
    }]
}

// ---------------------------------------------------------------------------
// Dither layers

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct DitherKey {
    width: u32,
    height: u32,
    variant: u8,
    strength: u16,
    value: u16,
    opacity: u16,
}

static DITHER_CACHE: OnceLock<Mutex<HashMap<DitherKey, Arc<RenderImage>>>> = OnceLock::new();

fn variant_index(variant: DitherVariant) -> u8 {
    match variant {
        DitherVariant::Ambient => 0,
        DitherVariant::Selection => 1,
        DitherVariant::Halo => 2,
        DitherVariant::Button => 3,
        DitherVariant::Meter => 4,
    }
}

fn texture(options: DitherOptions) -> Arc<RenderImage> {
    let (width, height) = options.dimensions();
    let key = DitherKey {
        width,
        height,
        variant: variant_index(options.variant),
        strength: (options.strength_unit() * 100.0).round() as u16,
        value: (options.value_unit() * 100.0).round() as u16,
        opacity: (options.opacity_unit() * 100.0).round() as u16,
    };
    let cache = DITHER_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(image) = cache.lock().unwrap().get(&key) {
        return image.clone();
    }
    let generated = create_dither_texture(options);
    let buffer = RgbaImage::from_raw(generated.width, generated.height, generated.pixels)
        .expect("dither buffer matches dimensions");
    let image = Arc::new(RenderImage::new(vec![Frame::new(buffer)]));
    let mut cache = cache.lock().unwrap();
    if cache.len() > 32 {
        cache.clear();
    }
    cache.insert(key, image.clone());
    image
}

pub struct DitherLayer {
    variant: DitherVariant,
    strength: f64,
    opacity: f64,
    value: f64,
    radius: f32,
}

impl DitherLayer {
    pub fn new(variant: DitherVariant) -> Self {
        Self {
            variant,
            strength: 1.0,
            opacity: 1.0,
            value: 1.0,
            radius: 0.0,
        }
    }

    pub fn strength(mut self, strength: f64) -> Self {
        self.strength = strength;
        self
    }

    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }
}

impl IntoElement for DitherLayer {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for DitherLayer {
    type RequestLayoutState = ();
    type PrepaintState = Option<Arc<RenderImage>>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let width = f32::from(bounds.size.width).ceil() as f64;
        let height = f32::from(bounds.size.height).ceil() as f64;
        if width <= 0.0 || height <= 0.0 {
            return None;
        }
        Some(texture(
            DitherOptions::new(width, height, self.variant)
                .strength(self.strength)
                .value(self.value)
                .opacity(self.opacity),
        ))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        if let Some(image) = prepaint.take() {
            window
                .paint_image(bounds, Corners::all(px(self.radius)), image, 0, false)
                .ok();
        }
    }
}

pub fn dither_layer(variant: DitherVariant, strength: f64, opacity: f64, radius: f32) -> Div {
    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .child(
            DitherLayer::new(variant)
                .strength(strength)
                .opacity(opacity)
                .radius(radius),
        )
}

pub fn dither_meter(value: f32, radius: f32) -> Div {
    div()
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .right(px(0.0))
        .bottom(px(0.0))
        .child(
            DitherLayer::new(DitherVariant::Meter)
                .value(value as f64)
                .radius(radius),
        )
}
