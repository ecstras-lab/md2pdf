//! Colors shared by the Typst preamble and the generated syntax theme.

/// An 8-bit RGBA color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl Color {
    /// An opaque color from a `0xRRGGBB` literal.
    pub const fn hex(value: u32) -> Self {
        Self {
            red: (value >> 16) as u8,
            green: (value >> 8) as u8,
            blue: value as u8,
            alpha: 0xff,
        }
    }

    /// CSS `hsl()`: `hue` in degrees, `saturation` and `lightness` as fractions of one.
    pub fn hsl(
        hue: f64,
        saturation: f64,
        lightness: f64,
    ) -> Self {
        let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let sector = hue / 60.0;
        let midpoint = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());

        let (red, green, blue) = match sector as u32 {
            0 => (chroma, midpoint, 0.0),
            1 => (midpoint, chroma, 0.0),
            2 => (0.0, chroma, midpoint),
            3 => (0.0, midpoint, chroma),
            4 => (midpoint, 0.0, chroma),
            _ => (chroma, 0.0, midpoint),
        };

        let lift = lightness - chroma / 2.0;
        let to_byte = |channel: f64| ((channel + lift) * 255.0).round() as u8;

        Self {
            red: to_byte(red),
            green: to_byte(green),
            blue: to_byte(blue),
            alpha: 0xff,
        }
    }

    /// The alpha of CSS `rgba(.., opacity)`, where `opacity` is a fraction of one.
    pub fn with_opacity(
        self,
        opacity: f64,
    ) -> Self {
        Self {
            alpha: (opacity * 255.0).round() as u8,
            ..self
        }
    }

    /// `#rrggbb`. TextMate themes accept no alpha channel.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.red, self.green, self.blue,)
    }

    /// A Typst `rgb(..)` call, carrying alpha only when the color is translucent.
    pub fn to_typst(self) -> String {
        if self.alpha == 0xff {
            format!("rgb(\"{}\")", self.to_hex())
        } else {
            format!("rgb(\"{}{:02x}\")", self.to_hex(), self.alpha,)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected values produced independently by Python's `colorsys.hls_to_rgb`.
    #[test]
    fn hsl_matches_css_reference_values() {
        let cases = [
            (142.0, 0.76, 0.70, "#78eda3"),
            (215.0, 0.16, 0.65, "#97a3b4"),
            (6.0, 1.00, 0.77, "#ff958a"),
            (35.0, 1.00, 0.70, "#ffbf66"),
        ];

        for (hue, saturation, lightness, expected) in cases {
            let color = Color::hsl(hue, saturation, lightness);

            assert_eq!(color.to_hex(), expected);
        }
    }

    #[test]
    fn hsl_covers_every_hue_sector() {
        let cases = [
            (0.0, "#ff0000"),
            (60.0, "#ffff00"),
            (120.0, "#00ff00"),
            (180.0, "#00ffff"),
            (240.0, "#0000ff"),
            (300.0, "#ff00ff"),
            (360.0, "#ff0000"),
        ];

        for (hue, expected) in cases {
            let color = Color::hsl(hue, 1.0, 0.5);

            assert_eq!(color.to_hex(), expected);
        }
    }

    #[test]
    fn typst_literal_carries_alpha_only_when_translucent() {
        let opaque = Color::hex(0xe11d48);
        let translucent = Color::hex(0x086ddd).with_opacity(0.1);

        assert_eq!(opaque.to_typst(), "rgb(\"#e11d48\")");
        assert_eq!(translucent.to_typst(), "rgb(\"#086ddd1a\")");
    }
}
