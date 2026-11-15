//! Deterministic ordered-dither textures.
//!
//! The Bayer matrix recursion and field vocabulary are adapted from
//! dither-kit-rs by Deepanshu Mishra (MIT); see assets/licenses/DitherKit-LICENSE.

use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DitherVariant {
    Ambient,
    Selection,
    Halo,
    Button,
    Meter,
}

impl DitherVariant {
    fn cell(self) -> u32 {
        match self {
            Self::Halo | Self::Meter => 1,
            _ => 2,
        }
    }

    fn alpha(self) -> u8 {
        match self {
            Self::Ambient => 22,
            Self::Selection => 45,
            Self::Halo => 100,
            Self::Button => 70,
            Self::Meter => 220,
        }
    }

    fn color(self) -> [u8; 3] {
        match self {
            Self::Meter => [117, 217, 163],
            _ => [91, 146, 245],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DitherOptions {
    pub width: f64,
    pub height: f64,
    pub variant: DitherVariant,
    pub strength: f64,
    pub value: f64,
    pub opacity: f64,
}

impl DitherOptions {
    pub fn new(width: f64, height: f64, variant: DitherVariant) -> Self {
        Self {
            width,
            height,
            variant,
            strength: 1.0,
            value: 1.0,
            opacity: 1.0,
        }
    }

    pub fn strength(mut self, strength: f64) -> Self {
        self.strength = strength;
        self
    }

    pub fn value(mut self, value: f64) -> Self {
        self.value = value;
        self
    }

    pub fn opacity(mut self, opacity: f64) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn dimensions(&self) -> (u32, u32) {
        (dim(self.width), dim(self.height))
    }

    pub fn strength_unit(&self) -> f64 {
        unit(self.strength, 1.0)
    }

    pub fn value_unit(&self) -> f64 {
        unit(self.value, 1.0)
    }

    pub fn opacity_unit(&self) -> f64 {
        unit(self.opacity, 1.0)
    }
}

pub struct DitherTexture {
    pub width: u32,
    pub height: u32,
    /// BGRA, as required by GPUI's render image cache.
    pub pixels: Vec<u8>,
}

const B2: [u32; 4] = [0, 2, 3, 1];

fn b4(x: u32, y: u32) -> u32 {
    4 * B2[((y % 2) * 2 + (x % 2)) as usize] + B2[((y / 2) * 2 + (x / 2)) as usize]
}

fn b8(x: u32, y: u32) -> u32 {
    4 * b4(x % 4, y % 4) + B2[((y / 4) * 2 + (x / 4)) as usize]
}

pub fn bayer8_threshold(x: i64, y: i64) -> f64 {
    fn modulo8(n: i64) -> u32 {
        n.rem_euclid(8) as u32
    }
    (b8(modulo8(x), modulo8(y)) as f64 + 0.5) / 64.0
}

fn clamp(value: f64, low: f64, high: f64) -> f64 {
    value.min(high).max(low)
}

fn dim(value: f64) -> u32 {
    clamp(
        if value.is_finite() {
            value.round()
        } else {
            1.0
        },
        1.0,
        2048.0,
    ) as u32
}

fn unit(value: f64, fallback: f64) -> f64 {
    let value = if value.is_finite() { value } else { fallback };
    (clamp(value, 0.0, 1.0) * 100.0).round() / 100.0
}

fn density(variant: DitherVariant, x: f64, y: f64, value: f64) -> f64 {
    match variant {
        DitherVariant::Ambient => {
            let wave = 0.5 + 0.5 * (x * 9.0 + (y * 7.0).sin() * 1.5 + y * 5.0).sin();
            let curtain = (-((y - (0.35 + 0.12 * (x * 6.0).sin())) / 0.36).powi(2)).exp();
            (0.09 + 0.28 * wave * curtain)
                * (PI * clamp(y, 0.0, 1.0)).sin()
                * ((PI * clamp(x, 0.0, 1.0)) / 2.0).sin()
        }
        DitherVariant::Selection => 0.45 * x.powf(1.5) * (0.35 + 0.65 * y.powf(1.8)),
        DitherVariant::Halo => {
            let r = (((x - 0.5) * 2.0).powi(2) + ((y - 0.5) * 2.0).powi(2)).sqrt();
            0.55 * (-((r - 0.53) / 0.25).powi(2)).exp() * clamp((1.0 - r) * 5.0, 0.0, 1.0)
        }
        DitherVariant::Button => 0.24 + 0.18 * (1.0 - y) + 0.07 * (x * PI * 2.0 + y * 5.0).sin(),
        DitherVariant::Meter => {
            if value <= 0.0 || x >= value {
                0.0
            } else {
                0.78 * clamp((value - x) * 32.0, 0.0, 1.0)
            }
        }
    }
}

pub fn create_dither_texture(options: DitherOptions) -> DitherTexture {
    let (width, height) = options.dimensions();
    let strength = options.strength_unit();
    let opacity = options.opacity_unit();
    let value = options.value_unit();
    let cell = options.variant.cell();
    let alpha = options.variant.alpha() as f64;
    let color = options.variant.color();
    let mut pixels = vec![0u8; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let gx = x / cell;
            let gy = y / cell;
            let fx = (gx * cell) as f64 + cell as f64 / 2.0;
            let fy = (gy * cell) as f64 + cell as f64 / 2.0;
            let fx = fx / width as f64;
            let fy = fy / height as f64;
            if density(options.variant, fx, fy, value) * strength
                <= bayer8_threshold(gx as i64, gy as i64)
            {
                continue;
            }
            let index = ((y * width + x) * 4) as usize;
            pixels[index] = color[2];
            pixels[index + 1] = color[1];
            pixels[index + 2] = color[0];
            pixels[index + 3] = (alpha * opacity).round() as u8;
        }
    }

    DitherTexture {
        width,
        height,
        pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bayer_thresholds_are_periodic_and_cover_each_level_once() {
        let mut thresholds = Vec::new();
        for i in 0..64 {
            thresholds.push(bayer8_threshold((i % 8) as i64, (i / 8) as i64));
        }
        let unique: std::collections::HashSet<_> =
            thresholds.iter().map(|value| value.to_bits()).collect();
        assert_eq!(unique.len(), 64);
        assert_eq!(thresholds.iter().filter(|value| **value < 0.5).count(), 32);
        assert_eq!(bayer8_threshold(-1, -1), bayer8_threshold(7, 7));
    }

    #[test]
    fn produces_deterministic_distinct_transparent_textures() {
        let variants = [
            DitherVariant::Ambient,
            DitherVariant::Selection,
            DitherVariant::Halo,
            DitherVariant::Button,
            DitherVariant::Meter,
        ];
        let images: Vec<_> = variants
            .iter()
            .map(|variant| {
                create_dither_texture(DitherOptions::new(64.0, 32.0, *variant).value(0.7))
            })
            .collect();
        let again =
            create_dither_texture(DitherOptions::new(64.0, 32.0, DitherVariant::Button).value(0.7));
        assert_eq!(images[3].pixels, again.pixels);
        assert_eq!((images[0].width, images[0].height), (64, 32));
        for (index, image) in images.iter().enumerate() {
            assert!(
                image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| pixel[3] == 0)
            );
            assert_eq!(
                image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|pixel| pixel[3])
                    .max()
                    .unwrap(),
                [22, 45, 100, 70, 220][index]
            );
            assert!(
                image
                    .pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|pixel| pixel[3] == 0 || pixel[2] == 91 || pixel[2] == 117)
            );
        }
        let unique: std::collections::HashSet<_> =
            images.iter().map(|image| image.pixels.clone()).collect();
        assert_eq!(unique.len(), 5);
    }

    #[test]
    fn clamps_dimensions_strength_and_meter_value() {
        let image =
            create_dither_texture(DitherOptions::new(-4.0, 2.3, DitherVariant::Meter).value(-1.0));
        assert_eq!((image.width, image.height), (1, 2));
        assert!(image.pixels.iter().all(|byte| *byte == 0));
        let strong = create_dither_texture(
            DitherOptions::new(10.0, 10.0, DitherVariant::Button).strength(5.0),
        );
        let normal = create_dither_texture(DitherOptions::new(10.0, 10.0, DitherVariant::Button));
        assert_eq!(strong.pixels, normal.pixels);
        assert_eq!(
            create_dither_texture(DitherOptions::new(3000.0, 1.0, DitherVariant::Meter)).width,
            2048
        );
    }

    #[test]
    fn meter_never_draws_beyond_its_value() {
        let texture = create_dither_texture(
            DitherOptions::new(100.0, 12.0, DitherVariant::Meter).value(0.35),
        );
        for y in 0..texture.height {
            for x in 35..texture.width {
                let index = ((y * texture.width + x) * 4) as usize;
                assert_eq!(texture.pixels[index + 3], 0);
            }
        }
    }
}
