use thiserror::Error;
use usvg;

/// An error with the drawer module. Could either come from SVG or font
/// loading errors, or rendering errors
#[derive(Error, Debug)]
pub enum DrawerError {
    #[error("SVG {svg:?} not found")]
    SVGNotFound { svg: String },
    #[error("Font {font:?} not found in fonts directory")]
    FontNotFound { font: String },
    #[error("Could not load file")]
    LoadFile {
        #[from]
        source: std::io::Error,
    },
    #[error("Could not load piece svg file")]
    LoadPieceSVG {
        #[from]
        source: usvg::Error,
    },
    #[error("An image {image:?} is too big to fit in an ImageBuffer")]
    ImageTooBig { image: String },
    #[error("SVG {svg:?} failed to be rendered")]
    SVGRenderError { svg: String },
    #[error("A correct SVG for {s:?} could not be produced")]
    SVGTreeFromStrError { source: usvg::Error, s: String },
    #[error("An SVGTree::{s:?} could not be loaded")]
    LoadSVGTree { s: String },
}
