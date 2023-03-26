use pgn_reader::BufferedReader;

use crate::config::Config;
use crate::error::C2GError;
use crate::giffer::PGNGiffer;

/// The main c2g app.
#[derive(Debug)]
pub struct Chess2Gif {
    pgn: String,
    giffer: PGNGiffer,
}

impl Chess2Gif {
    pub fn new(pgn: String, config: Config) -> Result<Self, C2GError> {
        Ok(Chess2Gif {
            pgn,
            giffer: PGNGiffer::new(config)?,
        })
    }

    /// Runs the main c2g app by reading the PGN game provided.
    pub fn run(mut self) -> Result<Option<Vec<u8>>, C2GError> {
        log::info!("Reading PGN");
        let mut reader = BufferedReader::new_cursor(&self.pgn[..]);

        match reader.read_game(&mut self.giffer) {
            Ok(result) => match result {
                // result contains Option<Result<(), C2GError>>
                Some(r) => match r {
                    Ok(Some(v)) => Ok(Some(v)),
                    Ok(None) | Err(_) => Ok(None),
                },
                None => Ok(None),
            },
            Err(e) => Err(C2GError::ReadGame { source: e }),
        }
    }
}
