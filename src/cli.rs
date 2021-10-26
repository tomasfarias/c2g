use std::collections::HashSet;
use std::ffi::OsString;
use std::io;
use std::str::FromStr;

use clap::{App, Arg};
use pgn_reader::BufferedReader;

use crate::config::{Colors, Config};
use crate::delay::{Delay, Delays};
use crate::error::C2GError;
use crate::giffer::PGNGiffer;
use crate::style::{StyleComponent, StyleComponents};

pub struct Chess2Gif {
    pgn: Option<String>,
    giffer: PGNGiffer,
}

impl Chess2Gif {
    pub fn new() -> Self {
        Self::new_from(std::env::args_os().into_iter()).unwrap_or_else(|e| e.exit())
    }

    pub fn new_from<I, T>(args: I) -> Result<Self, C2GError>
    where
        I: Iterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let app = App::new("Chess to GIF")
            .version("0.6.1")
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
                Arg::with_name("flip")
                    .long("flip")
                    .takes_value(false)
                    .help("By default, white appears at the bottom, use this flag to flip the board"),
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
                Arg::with_name("delay")
                    .long("delay")
                    .takes_value(true)
                    .default_value("1000")
                    .help("Delay between GIF frames in ms. Use 'real' to use the time given by %clk comments if available in the PGN"),
            )
            .arg(
                Arg::with_name("first-frame-delay")
                    .long("first-frame-delay")
                    .takes_value(true)
                    .default_value("1000")
                    .help("Delay for the first frame in ms, since clocks start with first move"),
            )
            .arg(
                Arg::with_name("last-frame-delay")
                    .long("last-frame-delay")
                    .takes_value(true)
                    .default_value("5000")
                    .help("Delay for the last frame in ms, before the GIF loops back around"),
            )
            .arg(
                Arg::with_name("style")
                    .long("no-player-bars")
                    .takes_value(false)
                    .overrides_with("plain")
                    .validator(|val| {
                        let mut invalid_vals = val.split(',').filter(|style| {
                            !&[
                                "full", "plain", "player-bars", "ranks", "files", "coordinates", "terminations",
                            ]
                                .contains(style)
                        });
                        if let Some(invalid) = invalid_vals.next() {
                            Err(C2GError::UnknownStyle(invalid.to_string()).to_string())
                        } else {
                            Ok(())
                        }
                    })
                    .help(
                        "Comma-separated list of style elements to display \
                         (*full*, plain, player-bars, ranks, files, terminations).",
                    )
                    .long_help(
                        "Configure which elements (ranks, files, player-bars, ...)
                         to display with the game GIF. The argument is a comma-separated \
                         list of components to display (e.g. 'ranks,files') or a \
                         pre-defined style (e.g. 'full').
                         Possible values:\n\n  \
                         * full: enables all available elements (default).\n  \
                         * plain: disables all available elements.\n  \
                         * ranks: show rank numbers.\n  \
                         * files: show file lettrs.\n  \
                         * coordintes: show both ranks and files. Same as 'ranks,files'.\n  \
                         * player-bars: draw bars with player information like names and ELO.",
                    ),
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
                Arg::with_name("svgs-path")
                    .long("svgs-path")
                    .takes_value(true)
                    .required(false)
                    .help("Path to directory containing SVGs of chess pieces and other effects. If compiled with include-svgs (default), this argument is ignored."),
            )
            .arg(
                Arg::with_name("pieces")
                    .long("pieces")
                    .takes_value(true)
                    .default_value("cburnett")
                    .required(false)
                    .help("Family of SVG pieces to use. Should be a directory inside svgs-path."),
            )
            .arg(
                Arg::with_name("fonts-path")
                    .long("fonts-path")
                    .takes_value(true)
                    .required(false)
                    .help("Path to directory containing desired coordinates font. If compiled with include-fonts (default), this argument is ignored."),
            )
            .arg(
                Arg::with_name("font-family")
                    .long("font-family")
                    .takes_value(true)
                    .default_value("Roboto")
                    .required(false)
                    .help("Font family to use for coordinates. Should be a file inside fonts-path."),
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

        let svgs_path = if cfg!(feature = "include-svgs") {
            "svgs/"
        } else {
            match matches.value_of("svgs-path") {
                Some(p) => p,
                None => "svgs/",
            }
        };

        let font_path = if cfg!(feature = "include-fonts") {
            "fonts/"
        } else {
            match matches.value_of("font-path") {
                Some(p) => p,
                None => "fonts/",
            }
        };

        let font_family = matches
            .value_of("font-family")
            .expect("Font-family must be defined or default value of roboto is used");
        let pieces = matches
            .value_of("pieces")
            .expect("Pieces must be defined or default value of cburnett is used");

        let output = matches.value_of("output").expect("Output must be defined");

        let dark = matches
            .value_of("dark")
            .expect("Dark must be defined or default value is used");
        let light = matches
            .value_of("light")
            .expect("Light must be defined or default value is used");

        let delay = match matches.value_of("delay") {
            Some(s) => Delay::from_str(s).expect("Invalid delay value"),
            None => panic!("Delay must be defined as it has a default value"),
        };

        let last_frame_delay = match matches.value_of("last-frame-delay") {
            Some(s) => Delay::from_str(s).expect("Invalid last frame delay value"),
            None => panic!("Last frame delay must be defined as it has a default value"),
        };

        let first_frame_delay = match matches.value_of("first-frame-delay") {
            Some(s) => Delay::from_str(s).expect("Invalid first frame delay value"),
            None => panic!("First frame delay must be defined as it has a default value"),
        };

        let flip = matches.is_present("flip");
        let styles = if matches.is_present("plain") {
            [StyleComponent::Plain].iter().cloned().collect()
        } else {
            matches
                .value_of("style")
                .map(|styles| {
                    styles
                        .split(',')
                        .map(|style| style.parse::<StyleComponent>())
                        .filter_map(|style| style.ok())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_else(|| vec![StyleComponent::Full])
                .into_iter()
                .map(|style| style.components())
                .fold(HashSet::new(), |mut acc, components| {
                    acc.extend(components.iter().cloned());
                    acc
                })
        };
        let style_components = StyleComponents(styles);
        let colors = Colors::from_strs(dark, light)?;
        let delays = Delays::new(&delay, &first_frame_delay, &last_frame_delay);

        let config = Config {
            output_path: output.to_string(),
            svgs_path: svgs_path.to_string(),
            font_path: font_path.to_string(),
            font_family: font_family.to_string(),
            pieces_family: pieces.to_string(),
            size,
            colors,
            flip,
            delays,
            style_components,
        };

        Ok(Chess2Gif {
            pgn,
            giffer: PGNGiffer::new(config)?,
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
