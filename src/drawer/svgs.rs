use std::fmt;

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
fn font_data(font: &str) -> Result<Vec<u8>, DrawerError> {
    let font_file = FONTS_DIR.get_file(font).ok_or(DrawerError::FontNotFound {
        font: font.to_owned(),
    })?;
    Ok(font_file.contents.to_vec())
}

#[cfg(not(feature = "include-fonts"))]
fn font_data(font: &str) -> Result<Vec<u8>, DrawerError> {
    let mut f = fs::File::open(font).map_err(|_| DrawerError::FontNotFound {
        font: font.to_owned(),
    })?;
    let mut buffer: Vec<u8> = Vec::new();
    f.read_to_end(&mut buffer)
        .map_err(|source| DrawerError::LoadFile { source: source })?;

    Ok(buffer)
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

/// A collection of SVG trees
pub struct SVGForest {
    pieces_path: Option<String>,
    terminations_path: Option<String>,
    svg_options: Options,
}

impl SVGForest {
    pub fn new(
        font_path: &str,
        pieces_path: Option<&str>,
        terminations_path: Option<&str>,
    ) -> Result<Self, DrawerError> {
        let mut opt = Options::default();

        // Load font for coordinates
        let mut fonts = fontdb::Database::new();
        let font_data = font_data(font_path)?;
        fonts.load_font_data(font_data);
        opt.keep_named_groups = true;
        opt.fontdb = fonts;
        opt.font_size = 16.0;
        // There should only be 1 font in DB
        opt.font_family = (*(opt.fontdb.faces())[0].family).to_owned();

        Ok(SVGForest {
            pieces_path: pieces_path.map_or(None, |s| Some(s.to_owned())),
            terminations_path: terminations_path.map_or(None, |s| Some(s.to_owned())),
            svg_options: opt,
        })
    }

    pub fn piece_tree(
        &self,
        role: &Role,
        color: &shakmaty::Color,
        additional: Option<&str>,
    ) -> Result<Tree, DrawerError> {
        let pieces_path = self.pieces_path.as_ref().expect("pieces_path not defined");
        let full_piece_path = match additional {
            Some(s) => format!("{}/{}_{}_{}.svg", pieces_path, color.char(), role.char(), s),
            None => format!("{}/{}_{}.svg", pieces_path, color.char(), role.char()),
        };
        let svg_string = load_svg_string(&full_piece_path).unwrap_or_else(
            // Fallback to regular piece if additional not found
            |_| {
                load_svg_string(&format!(
                    "{}/{}_{}.svg",
                    pieces_path,
                    color.char(),
                    role.char()
                ))
                .unwrap()
            },
        );
        Tree::from_str(&svg_string, &self.svg_options)
            .map_err(|source| DrawerError::LoadPieceSVG { source })
    }

    pub fn termination_tree<F>(
        &self,
        termination: F,
        color: Option<shakmaty::Color>,
    ) -> Result<Tree, DrawerError>
    where
        F: fmt::Display,
    {
        let terminations_path = self
            .terminations_path
            .as_ref()
            .expect("terminations_path not defined");
        let full_termination_path = match color {
            Some(c) => format!("{}/{}_{}.svg", terminations_path, termination, c.char()),
            None => format!("{}/{}.svg", terminations_path, termination),
        };

        let svg_string = load_svg_string(&full_termination_path).unwrap_or_else(
            // Fallback to regular termination if color not found
            |_| load_svg_string(&format!("{}/{}.svg", terminations_path, termination)).unwrap(),
        );
        Tree::from_str(&svg_string, &self.svg_options)
            .map_err(|source| DrawerError::LoadPieceSVG { source })
    }

    pub fn str_svg_tree(
        &self,
        s: &str,
        color: Rgba<u8>,
        background: Rgba<u8>,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
        font_weight: FontWeight,
        font_size: FontSize,
    ) -> Result<Tree, DrawerError> {
        let svg_string = format!(
            "<svg xmlns:svg=\"http://www.w3.org/2000/svg\" xmlns=\"http://www.w3.org/2000/svg\" version=\"1.0\" height=\"{}\" width=\"{}\" style=\"background-color:rgb({},{},{})\"> <text x=\"{}%\" y=\"{}%\" fill=\"rgb({}, {}, {})\" font-weight=\"{}\" font-size=\"{}\" dominant-baseline=\"text-bottom\" text-anchor=\"start\">{}</text></svg>",
            height,
            width,
            background[0],
            background[1],
            background[2],
            x,
            y,
            color[0],
            color[1],
            color[2],
            font_weight.to_string(),
            font_size.to_string(),
            s,
        );

        Tree::from_str(&svg_string, &self.svg_options).map_err(|source| {
            DrawerError::SVGTreeFromStrError {
                source,
                s: s.to_owned(),
            }
        })
    }
}
