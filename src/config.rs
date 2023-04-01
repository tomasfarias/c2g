use std::convert::TryInto;
use std::str::FromStr;

use crate::delay::Delays;
use crate::error::C2GError;
use crate::style::StyleComponents;

#[derive(Debug, Clone)]
pub struct Color(pub [u8; 4]);

impl FromStr for Color {
    type Err = C2GError;

    fn from_str(s: &str) -> Result<Self, C2GError> {
        let parse_result = if s.starts_with("#") {
            from_hex_str(s)
        } else {
            from_rgba_str(s)
        };

        let mut vec_color = match parse_result {
            Ok(v) => v,
            Err(e) => {
                return Err(C2GError::CannotParseColor {
                    color: s.to_string(),
                    reason: format!("{}", e),
                })
            }
        };

        if vec_color.len() == 3 {
            vec_color.push(1)
        } else if vec_color.len() != 4 {
            return Err(C2GError::CannotParseColor {
                color: s.to_string(),
                reason: format!("Parsed vec is not of length 4: {:?}", vec_color),
            });
        }

        let try_array = vec_color.try_into();
        match try_array {
            Ok(arr) => Ok(Color(arr)),
            Err(e) => Err(C2GError::CannotParseColor {
                color: s.to_string(),
                reason: format!("Vec: {:?}", e),
            }),
        }
    }
}

/// Parse an RGBA color string
fn from_rgba_str(s: &str) -> Result<Vec<u8>, C2GError> {
    let mut tmp = Vec::with_capacity(3);

    for val in s.split(",") {
        match val.parse::<u8>() {
            Ok(n) => tmp.push(n),
            Err(e) => {
                return Err(C2GError::CannotParseColor {
                    color: s.to_string(),
                    reason: format!("{}", e),
                })
            }
        }
    }

    Ok(tmp)
}

/// Parse a HEX color string
fn from_hex_str(s: &str) -> Result<Vec<u8>, C2GError> {
    let mut tmp = Vec::with_capacity(4);

    let s = match s.strip_prefix("#") {
        Some(stripped) => stripped,
        None => s,
    };

    let bytes = s.as_bytes();

    for (n, b) in bytes.iter().step_by(2).enumerate() {
        // We are stepping by 2.
        let hex_bytes = &[*b, bytes[n * 2 + 1]];

        let hex_number =
            std::str::from_utf8(hex_bytes).map_err(|e| C2GError::CannotParseColor {
                color: s.to_string(),
                reason: format!("{}", e),
            })?;

        let parsed =
            u8::from_str_radix(&hex_number, 16).map_err(|e| C2GError::CannotParseColor {
                color: s.to_string(),
                reason: format!("{}", e),
            })?;
        tmp.push(parsed)
    }

    Ok(tmp)
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
pub enum Output {
    Path(String),
    Buffer,
}

#[derive(Debug, Clone)]
pub struct Config {
    /// GIF output: either a path or a buffer.
    pub output: Output,

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
            output: Output::Path("c2g.gif".to_string()),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_str() {
        let color = Color::from_str("#B83B26").unwrap();
        assert_eq!(color.to_arr(), [184, 59, 38, 1]);

        let color = Color::from_str("B83B26").unwrap();
        assert_eq!(color.to_arr(), [184, 59, 38, 1]);

        let color = Color::from_str("184,59,38").unwrap();
        assert_eq!(color.to_arr(), [184, 59, 38, 1]);
    }
}
