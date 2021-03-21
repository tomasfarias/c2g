use std::convert::TryInto;
use std::ffi::OsString;
use std::io;

use clap::{App, Arg};
use pgn_reader::BufferedReader;

use crate::error::C2GError;
use crate::giffer::PGNGiffer;

pub struct Chess2Gif<'a> {
    pgn: Option<String>,
    giffer: PGNGiffer<'a>,
}

impl<'a> Chess2Gif<'a> {
    pub fn new() -> Self {
        Self::new_from(std::env::args_os().into_iter()).unwrap_or_else(|e| e.exit())
    }

    pub fn new_from<I, T>(args: I) -> Result<Self, C2GError>
    where
        I: Iterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let app = App::new("Chess to GIF")
            .version("0.1.0")
            .author("Tomas Farias <tomas@tomasfarias.dev>")
            .about("Turns a PGN chess game into a GIF")
            .arg(
                Arg::with_name("PGN")
                    .takes_value(true)
                    .required(false)
                    .help("A PGN string for a chess game"),
            )
            .arg(
                Arg::with_name("output")
                    .short("o")
                    .long("output")
                    .takes_value(true)
                    .default_value("chess.gif")
                    .help("Write GIF to file"),
            )
            .arg(
                Arg::with_name("size")
                    .short("s")
                    .long("size")
                    .takes_value(true)
                    .default_value("640")
                    .help("The size of one side of the board in pixels"),
            )
            .arg(
                Arg::with_name("dark")
                    .short("d")
                    .long("dark")
                    .takes_value(true)
                    .number_of_values(4)
                    .default_value("118,150,86,1")
                    .require_delimiter(true)
                    .multiple(false)
                    .help("RGBA color to use for the dark squares"),
            )
            .arg(
                Arg::with_name("light")
                    .short("l")
                    .long("light")
                    .takes_value(true)
                    .number_of_values(4)
                    .default_value("238,238,210,1")
                    .require_delimiter(true)
                    .multiple(false)
                    .help("RGBA color to use for the light squares"),
            )
            .arg(
                Arg::with_name("pieces-path")
                    .long("pieces-path")
                    .takes_value(true)
                    .help("Path to directory containing SVGs of chess pieces. If compiled with include-pieces (default), this argument can be used to set a different family of pieces, defaults to cburnett")
                    .default_value("cburnett"),
            )
            .arg(
                Arg::with_name("font-path")
                    .long("font-path")
                    .takes_value(true)
                    .help("Path to desired coordinates font. If compiled with include-fonts, this argument can be used to set a different coordinate font, defaults to roboto")
                    .default_value("roboto.ttf"),
            );

        let matches = app.get_matches_from_safe(args)?;

        let size = u32::from_str_radix(matches.value_of("size").expect("Size must be defined"), 10)
            .expect("Size must be a positive number");

        if size % 8 != 0 {
            return Err(C2GError::NotDivisibleBy8);
        }

        let pgn = if matches.value_of("PGN").is_some() {
            Some(matches.value_of("PGN").unwrap().to_owned())
        } else {
            None
        };

        let pieces_path = matches
            .value_of("pieces-path")
            .expect("Path to pieces must be defined");
        let font_path = matches
            .value_of("font-path")
            .expect("Path to coordinates must be defined");

        let output = matches.value_of("output").expect("Output must be defined");

        let dark: [u8; 4] = clap::values_t_or_exit!(matches, "dark", u8)
            .try_into()
            .expect("Invalid dark color");
        let light: [u8; 4] = clap::values_t_or_exit!(matches, "light", u8)
            .try_into()
            .expect("Invalid light color");

        Ok(Chess2Gif {
            pgn: pgn,
            giffer: PGNGiffer::new(pieces_path, font_path, size, output, 100, dark, light)?,
        })
    }

    pub fn run(mut self) -> Result<(), C2GError> {
        log::info!("Reading PGN");
        let result = if let Some(pgn) = self.pgn {
            let mut reader = BufferedReader::new_cursor(&pgn[..]);
            match reader.read_game(&mut self.giffer) {
                Ok(result) => match result {
                    // result contains Option<Result<(), C2GError>>
                    Some(r) => Ok(r.unwrap()),
                    None => Ok(()),
                },
                Err(e) => Err(C2GError::ReadGame { source: e }),
            }
        } else {
            let stdin = io::stdin();
            let mut reader = BufferedReader::new(stdin);
            match reader.read_game(&mut self.giffer) {
                Ok(result) => match result {
                    // result contains Option<Result<(), C2GError>>
                    Some(r) => Ok(r.unwrap()),
                    None => Ok(()),
                },
                Err(e) => Err(C2GError::ReadGame { source: e }),
            }
        };
        log::info!("Done!");
        result
    }
}
