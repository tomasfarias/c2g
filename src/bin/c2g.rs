use std::collections::HashSet;
use std::ffi::OsString;
use std::io::{self, Read};
use std::str::FromStr;

use clap::{App, Arg};

use c2g::app::Chess2Gif;
use c2g::config::{Colors, Config, Output};
use c2g::delay::{Delay, Delays};
use c2g::error::C2GError;
use c2g::style::{StyleComponent, StyleComponents};

#[derive(Debug)]
pub struct Chess2GifCli {
    app: Chess2Gif,
}

impl Chess2GifCli {
    pub fn new() -> Self {
        Self::new_from(std::env::args_os().into_iter()).unwrap_or_else(|e| e.exit())
    }

    pub fn new_from<I, T>(args: I) -> Result<Self, C2GError>
    where
        I: Iterator<Item = T>,
        T: Into<OsString> + Clone,
    {
        let app = App::new("Chess to GIF")
            .version("0.7.4")
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
                    .long("style")
                    .takes_value(true)
                    .default_value("full")
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
                Arg::with_name("plain")
                    .long("plain")
                    .takes_value(false)
                    .overrides_with("style")
                    .help(
                        "Use plain style.",
                    )
            )
            .arg(
                Arg::with_name("dark")
                    .short("d")
                    .long("dark")
                    .takes_value(true)
                    .default_value("118,150,86")
                    .multiple(false)
                    .help("RGB or HEX color to use for the dark squares"),
            )
            .arg(
                Arg::with_name("light")
                    .short("l")
                    .long("light")
                    .takes_value(true)
                    .default_value("238,238,210")
                    .multiple(false)
                    .help("RGB or HEX color to use for the light squares"),
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

        let size = Self::get_valid_size(matches.value_of("size").expect("Size must be defined"))?;

        let pgn = Self::pgn_or_read_stdin(matches.value_of("PGN"), &mut io::stdin())?;

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

        let output = Output::Path(
            matches
                .value_of("output")
                .expect("Output must be defined")
                .to_string(),
        );

        let dark = matches
            .value_of("dark")
            .expect("Dark must be defined or default value is used");
        let light = matches
            .value_of("light")
            .expect("Light must be defined or default value is used");

        let colors = Colors::from_strs(dark, light)?;

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

        let delays = Delays::new(&delay, &first_frame_delay, &last_frame_delay);

        let config = Config {
            output: output,
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

        let app = Chess2Gif::new(pgn, config)?;

        Ok(Self { app })
    }

    fn pgn_or_read_stdin(pgn: Option<&str>, mut input: impl Read) -> Result<String, C2GError> {
        if let Some(s) = pgn {
            Ok(s.to_owned())
        } else {
            let mut buffer = String::new();
            input.read_to_string(&mut buffer)?;
            Ok(buffer)
        }
    }

    fn get_valid_size(s: &str) -> Result<u32, C2GError> {
        let size = u32::from_str_radix(s, 10).expect("Size must be a positive number");

        if size % 8 != 0 {
            return Err(C2GError::NotDivisibleBy8);
        }

        Ok(size)
    }

    fn run(self) -> Result<Option<Vec<u8>>, C2GError> {
        self.app.run()
    }
}

fn main() -> Result<(), C2GError> {
    env_logger::init();

    let c2g = Chess2GifCli::new();
    match c2g.run() {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_valid_size() -> Result<(), String> {
        let result = Chess2GifCli::get_valid_size("16");

        match result {
            Ok(v) => {
                if v == 16 {
                    Ok(())
                } else {
                    Err(String::from(
                        "Value returned by get_valid_size did not match 16",
                    ))
                }
            }
            Err(_) => Err(String::from("Returned error despite being divisible by 8")),
        }
    }

    #[test]
    fn test_get_valid_size_with_invalid_input() -> Result<(), String> {
        let result = Chess2GifCli::get_valid_size("20");

        match result {
            Ok(v) => {
                if v == 20 {
                    Err(String::from(
                        "Return value despite not being divisible by 8",
                    ))
                } else {
                    Err(String::from("Returned unexpected value"))
                }
            }
            // We expect an error this time
            Err(_) => Ok(()),
        }
    }

    #[test]
    #[should_panic(expected = "Size must be a positive number")]
    fn test_get_valid_size_should_panic() {
        let _ = Chess2GifCli::get_valid_size("-20");
    }

    #[test]
    fn test_pgn_or_read_stdin_with_none_pgn() -> Result<(), String> {
        use std::io::Cursor;

        let buff = Cursor::new("test string");
        let pgn = None;
        let result = Chess2GifCli::pgn_or_read_stdin(pgn, buff);

        match result {
            Ok(s) => {
                if s == "test string".to_string() {
                    Ok(())
                } else {
                    Err(String::from(
                        "String read from buffer does not equal test string",
                    ))
                }
            }
            Err(_) => Err(String::from("Error reading buffer")),
        }
    }

    #[test]
    fn test_pgn_or_read_stdin() -> Result<(), String> {
        use std::io::Cursor;

        let buff = Cursor::new("invalid");
        let pgn = Some("test string");
        let result = Chess2GifCli::pgn_or_read_stdin(pgn, buff);

        match result {
            Ok(s) => {
                if s == "test string".to_string() {
                    Ok(())
                } else if s == "invalid".to_string() {
                    Err(String::from("String read from buffer when pgn not none"))
                } else {
                    Err(String::from(
                        "String read from buffer does not equal test string",
                    ))
                }
            }
            Err(_) => Err(String::from("Error reading buffer")),
        }
    }
}
