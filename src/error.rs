use std::io::{stderr, Write};
use std::process;

use clap;
use thiserror::Error;

use crate::giffer::GifferError;

#[derive(Error, Debug)]
pub enum C2GError {
    #[error("Failed to read PGN chess game")]
    ReadGame {
        #[from]
        source: std::io::Error,
    },
    #[error("Failed to produce a GIF")]
    GIFRenderingError {
        #[from]
        source: GifferError,
    },
    #[error("Size is not divisible by 8")]
    NotDivisibleBy8,
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
            C2GError::GIFRenderingError { source: _ } => {
                writeln!(&mut stderr(), "{}", self).ok();
                process::exit(1);
            }
            C2GError::ReadGame { source: _ } => {
                writeln!(&mut stderr(), "{}", self).ok();
                process::exit(1);
            }
            C2GError::NotDivisibleBy8 => {
                writeln!(&mut stderr(), "{}", self).ok();
                process::exit(1);
            }
        }
    }
}
