use std::fmt;
use std::path::{Path, PathBuf};

use image::Rgba;
use include_dir::{include_dir, Dir};
use shakmaty::{self, Role};
use usvg::{self, fontdb, Options, Tree};

use super::error::DrawerError;

#[cfg(feature = "include-svgs")]
static SVGS_DIR: Dir = include_dir!("svgs/");

#[cfg(feature = "include-svgs")]
fn load_svg_string(svg_path: &str) -> Result<String, DrawerError> {
    let svg_file = SVGS_DIR
        .get_file(&svg_path)
        .ok_or(DrawerError::SVGNotFound {
            svg: svg_path.to_owned(),
        })?;
    Ok(svg_file
        .contents_utf8()
        .expect("Failed to parse file contents")
        .to_owned())
}

#[cfg(not(feature = "include-svgs"))]
fn load_svg_string(svg_path: &str) -> Result<String, DrawerError> {
    let mut f = fs::File::open(&svg_path).map_err(|_| DrawerError::SVGNotFound {
        svg: svg_path.to_owned(),
    })?;
    let mut svg_str = String::new();
    f.read_to_string(&mut svg_str)
        .map_err(|source| DrawerError::LoadFile { source })?;

    Ok(svg_str)
}

#[cfg(feature = "include-fonts")]
static FONTS_DIR: Dir = include_dir!("fonts/");

#[cfg(feature = "include-fonts")]
fn load_fonts(fonts: &mut fontdb::Database, _fonts_dir: &str) {
    for font_file in FONTS_DIR.files() {
        fonts.load_font_data(font_file.contents().to_vec())
    }
}

#[cfg(not(feature = "include-fonts"))]
fn load_fonts(fonts: &mut fontdb::Database, fonts_dir: &str) {
    fonts.load_font_dir(fonts_dir);
}

/// SVG font-weight attribute options
pub enum FontWeight {
    Normal,
    Bold,
    Bolder,
    Lighter,
    Number(f32),
}

impl fmt::Display for FontWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            FontWeight::Normal => write!(f, "normal"),
            FontWeight::Bold => write!(f, "bold"),
            FontWeight::Bolder => write!(f, "bolder"),
            FontWeight::Lighter => write!(f, "lighter"),
            FontWeight::Number(n) => write!(f, "{}", n),
        }
    }
}

/// SVG font-size attribute options
pub enum FontSize {
    XXSmall,
    XSmall,
    Small,
    Medium,
    Large,
    XLarge,
    XXLarge,
    XXXLarge,
    Unit(f32, String),
}

impl fmt::Display for FontSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FontSize::XXSmall => write!(f, "xx-small"),
            FontSize::XSmall => write!(f, "x-small"),
            FontSize::Small => write!(f, "small"),
            FontSize::Medium => write!(f, "medium"),
            FontSize::Large => write!(f, "large"),
            FontSize::XLarge => write!(f, "x-large"),
            FontSize::XXLarge => write!(f, "xx-large"),
            FontSize::XXXLarge => write!(f, "xxx-large"),
            FontSize::Unit(n, s) => write!(f, "{}{}", n, s),
        }
    }
}

/// All SVG tree variants that can be loaded
pub enum SVGTree {
    Str {
        s: String,
        string_color: Rgba<u8>,
        background_color: Rgba<u8>,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
        font_weight: FontWeight,
        font_size: FontSize,
    },
    Piece {
        role: Role,
        color: shakmaty::Color,
        additional: Option<String>,
    },
    Termination {
        reason: String,
        color: Option<shakmaty::Color>,
    },
}

impl SVGTree {
    /// Produce an expected file name for a given SVGTree
    pub fn svg_file(&self) -> Option<String> {
        match self {
            SVGTree::Str { .. } => None,
            SVGTree::Piece {
                role: r,
                color: c,
                additional: add,
            } => {
                let s = match add {
                    Some(a) => format!("{}_{}_{}.svg", c.char(), r.char(), a),
                    None => format!("{}_{}.svg", c.char(), r.char()),
                };
                Some(s)
            }
            SVGTree::Termination { reason: r, color } => {
                let s = match color {
                    Some(c) => format!("{}_{}.svg", r, c.char()),
                    None => format!("{}.svg", r),
                };
                Some(s)
            }
        }
    }
}

/// A struct to hold SVG font configuration options and provide a default
/// configuration.
pub struct SVGFontConfig {
    pub font_path: String,
    pub font_family: Option<String>,
    pub font_size: Option<f64>,
}

impl Default for SVGFontConfig {
    fn default() -> Self {
        SVGFontConfig {
            font_path: "fonts/".to_owned(),
            font_family: Some("roboto".to_owned()),
            // 16 works well with the default size of 640px but there should be a way
            // to calculate a proper default size given a board size.
            font_size: Some(16.0),
        }
    }
}

/// An SVG forest is where you would find SVG trees. SVGForest contains all
/// methods to produce SVG trees for pieces, circles, and coordinates.
pub struct SVGForest {
    pieces_path: PathBuf,
    terminations_path: PathBuf,
    svg_options: Options,
}

impl SVGForest {
    pub fn new(
        font_config: SVGFontConfig,
        svgs_path: &str,
        pieces_dir: &str,
        terminations_dir: &str,
    ) -> Result<Self, DrawerError> {
        let mut opt = Options::default();

        // Load font for coordinates
        let mut fonts = fontdb::Database::new();
        load_fonts(&mut fonts, &font_config.font_path);

        opt.keep_named_groups = true;
        opt.fontdb = fonts;

        if let Some(s) = font_config.font_size {
            opt.font_size = s;
        } else {
            // 16 works well with the default size of 640px
            opt.font_size = 16.0;
        }

        if let Some(f) = font_config.font_family {
            opt.font_family = f.to_string();
        } else {
            // If font_family is None, assume we will use the first font in DB
            opt.font_family = (*(opt.fontdb.faces())[0].family).to_owned();
        }

        let (pieces_path, terminations_path) = if cfg!(feature = "include-svgs") {
            (
                Path::new(pieces_dir).to_path_buf(),
                Path::new(terminations_dir).to_path_buf(),
            )
        } else {
            (
                Path::new(svgs_path).join(pieces_dir),
                Path::new(svgs_path).join(terminations_dir),
            )
        };

        Ok(SVGForest {
            pieces_path: pieces_path,
            terminations_path: terminations_path,
            svg_options: opt,
        })
    }

    pub fn load_svg_tree(&self, svg_tree: &SVGTree) -> Result<Tree, DrawerError> {
        let svg_string = match svg_tree {
            SVGTree::Str {
                s,
                height: h,
                width: w,
                x,
                y,
                background_color: b,
                string_color: c,
                font_weight: font_w,
                font_size: font_s,
            } => self.build_svg_string(s, *h, *w, *x, *y, *b, *c, font_w, font_s),
            s => self.load_svg_string_from_tree(s),
        }?;
        Tree::from_str(&svg_string, &self.svg_options)
            .map_err(|source| DrawerError::LoadPieceSVG { source })
    }

    pub fn build_svg_string(
        &self,
        s: &str,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
        background_color: Rgba<u8>,
        string_color: Rgba<u8>,
        font_weight: &FontWeight,
        font_size: &FontSize,
    ) -> Result<String, DrawerError> {
        Ok(format!(
            "<svg xmlns:svg=\"http://www.w3.org/2000/svg\" xmlns=\"http://www.w3.org/2000/svg\" version=\"1.0\" height=\"{}\" width=\"{}\" style=\"background-color:rgb({},{},{})\"> <text x=\"{}%\" y=\"{}%\" fill=\"rgb({}, {}, {})\" font-weight=\"{}\" font-size=\"{}\" dominant-baseline=\"text-bottom\" text-anchor=\"start\">{}</text></svg>",
            height,
            width,
            background_color[0],
            background_color[1],
            background_color[2],
            x,
            y,
            string_color[0],
            string_color[1],
            string_color[2],
            font_weight.to_string(),
            font_size.to_string(),
            s,
        ))
    }

    pub fn load_svg_string_from_tree(&self, svg_tree: &SVGTree) -> Result<String, DrawerError> {
        let svg_file = svg_tree.svg_file().expect("SVGTree variant not supported");

        let full_path = match svg_tree {
            SVGTree::Piece {
                role: _,
                color: _,
                additional: _,
            } => self.pieces_path.join(svg_file),
            SVGTree::Termination {
                reason: _,
                color: _,
            } => self.terminations_path.join(svg_file),
            _ => {
                return Err(DrawerError::LoadSVGTree {
                    s: "Str".to_string(),
                })
            }
        };

        let full_path_str = full_path.to_str().expect("Invalid SVG path");
        load_svg_string(full_path_str)
    }
}
