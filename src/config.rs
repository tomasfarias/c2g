use std::convert::TryInto;
use std::str::FromStr;

use crate::delay::Delays;
use crate::error::C2GError;
use crate::style::StyleComponents;

#[derive(Debug, Clone)]
pub struct Color([u8; 4]);

impl FromStr for Color {
    type Err = C2GError;

    fn from_str(s: &str) -> Result<Self, C2GError> {
        let mut tmp = Vec::with_capacity(4);
        for val in s.split(",") {
            match val.parse::<u8>() {
                Ok(n) => tmp.push(n),
                Err(_) => return Err(C2GError::CannotParseColor(s.to_string())),
            }
        }
        let rgba = tmp.try_into();
        match rgba {
            Ok(n) => Ok(Color(n)),
            Err(_) => Err(C2GError::CannotParseColor(s.to_string())),
        }
    }
}

impl Color {
    pub fn to_arr(&self) -> [u8; 4] {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct Colors {
    /// The board's dark square color.
    pub dark: Color,

    /// The board's light square color.
    pub light: Color,
}

impl Colors {
    pub fn new(dark: Color, light: Color) -> Colors {
        Colors { dark, light }
    }

    pub fn from_strs(dark: &str, light: &str) -> Result<Self, C2GError> {
        let dark = Color::from_str(dark)?;
        let light = Color::from_str(light)?;
        Ok(Colors::new(dark, light))
    }
}

impl Default for Colors {
    fn default() -> Self {
        Colors {
            dark: Color([118, 150, 86, 1]),
            light: Color([238, 238, 210, 1]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    /// Path to GIF output.
    pub output_path: String,

    /// Path to SVG files used to render pieces and others.
    pub svgs_path: String,

    /// Path to font files used to render coordinates.
    pub font_path: String,

    /// Font family name to render coordinates.
    pub font_family: String,

    /// Family of SVG pieces to use.
    pub pieces_family: String,

    /// Size of one side of the board in pixels. Must be multiple of 8.
    pub size: u32,

    /// Board colors.
    pub colors: Colors,

    /// Indicate whether to flip the board or not.
    pub flip: bool,

    /// Settings for delays between GIF frames.
    pub delays: Delays,

    /// Style elements like rank and file coordinates, player bars, etc ...
    pub style_components: StyleComponents,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            output_path: "c2g.gif".to_string(),
            svgs_path: "".to_string(),
            font_path: "".to_string(),
            font_family: "roboto".to_string(),
            pieces_family: "cburnett".to_string(),
            size: 640,
            colors: Colors::default(),
            flip: false,
            delays: Delays::default(),
            style_components: StyleComponents::default(),
        }
    }
}
