use std::process;

use thiserror::Error;

use crate::giffer::GifferError;

#[derive(Error, Debug)]
pub enum C2GError {
    #[error("Failed to read PGN chess game")]
    ReadGame {
        #[from]
        source: std::io::Error,
    },
    #[error(transparent)]
    GIFRenderingError {
        #[from]
        source: GifferError,
    },
    #[error("Size is not divisible by 8")]
    NotDivisibleBy8,
    #[error("Unknown style {0}")]
    UnknownStyle(String),
    #[error("Unable to parse duration {0}")]
    CannotParseDuration(String),
    #[error("Unable to parse color string {color}")]
    CannotParseColor { color: String, reason: String },
    #[error("Clap failed")]
    ClapError {
        #[from]
        source: clap::Error,
    },
}

impl C2GError {
    pub fn exit(&self) -> ! {
        match self {
            C2GError::ClapError { source: s } => s.exit(),
            C2GError::UnknownStyle(_)
            | C2GError::GIFRenderingError { source: _ }
            | C2GError::ReadGame { source: _ }
            | C2GError::NotDivisibleBy8
            | C2GError::CannotParseDuration(_)
            | C2GError::CannotParseColor {
                color: _,
                reason: _,
            } => {
                eprintln!("Error: {}", self);
                process::exit(1);
            }
        }
    }
}
