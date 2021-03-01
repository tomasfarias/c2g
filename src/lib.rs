use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::OsString;
use std::fs::File;
use std::io::BufWriter;

#[macro_use]
extern crate clap;

use clap::{App, Arg};
use gif::{Encoder, Frame, Repeat};
use image::{imageops, ImageBuffer, Rgba, RgbaImage};
use log;
use pgn_reader::{BufferedReader, SanPlus, Skip, Visitor};
use resvg;
use shakmaty::{Chess, Color, Move, Pieces, Position, Role, Setup, Square};
use usvg;

pub struct BoardDrawer {
    image_path: String,
    size: u32,
    piece_map: HashMap<String, RgbaImage>,
    dark: Rgba<u8>,
    light: Rgba<u8>,
}

impl BoardDrawer {
    pub fn new(image_path: String, size: u32, dark: [u8; 4], light: [u8; 4]) -> Self {
        let mut piece_map = HashMap::new();
        piece_map.reserve(12);

        BoardDrawer {
            image_path: image_path,
            size: size,
            piece_map: piece_map,
            dark: image::Rgba(dark),
            light: image::Rgba(light),
        }
    }

    pub fn get_buffer(&self) -> RgbaImage {
        ImageBuffer::new(self.size, self.size)
    }

    pub fn get_dark_square(&self) -> RgbaImage {
        ImageBuffer::from_pixel(self.size / 8, self.size / 8, self.dark)
    }

    pub fn get_light_square(&self) -> RgbaImage {
        ImageBuffer::from_pixel(self.size / 8, self.size / 8, self.light)
    }

    pub fn draw_position_from_empty(&mut self, pieces: Pieces) -> RgbaImage {
        log::debug!("Drawing initial board");
        let mut counter = 1;
        let mut column = ImageBuffer::from_fn(self.size / 8, self.size, |_, y| {
            if y >= self.size / 8 * counter {
                counter += 1;
            }
            if counter % 2 != 0 {
                self.dark
            } else {
                self.light
            }
        });

        let mut board = self.get_buffer();
        for n in 0..9 {
            imageops::overlay(&mut board, &column, n * self.size / 8, 0);
            imageops::flip_vertical_in_place(&mut column)
        }

        for (square, piece) in pieces {
            log::debug!("Initializing {:?} in {:?}", piece, square);
            self.draw_piece_on_board(&square, &piece.role, piece.color, false, &mut board);
        }

        board
    }

    pub fn draw_move_on_board(&mut self, _move: &Move, color: Color, board: &mut RgbaImage) {
        log::debug!("Drawing move: {:?}", _move);
        match _move {
            Move::Normal {
                role,
                from,
                capture,
                to,
                promotion,
            } => {
                self.blank_square_on_board(from, board);
                let blank_to_square = if capture.is_some() { true } else { false };

                if let Some(promoted) = promotion {
                    self.draw_piece_on_board(to, promoted, color, blank_to_square, board);
                } else {
                    self.draw_piece_on_board(to, role, color, blank_to_square, board);
                }
            }
            Move::EnPassant { from, to } => {
                self.blank_square_on_board(from, board);
                self.draw_piece_on_board(to, &Role::Pawn, color, true, board);
            }
            Move::Castle { king, rook } => {
                // King and Rook initial squares, e.g. E1 and H1 respectively.
                // Need to calculate where the pieces end up before drawing.
                let offset = if rook.file() > king.file() { 1 } else { -1 };

                self.blank_square_on_board(king, board);
                self.blank_square_on_board(rook, board);

                let rook_square = king.offset(offset * 1).unwrap();
                let king_square = king.offset(offset * 2).unwrap();
                self.draw_piece_on_board(&king_square, &Role::King, color, true, board);
                self.draw_piece_on_board(&rook_square, &Role::Rook, color, true, board);
            }
            Move::Put { role, to } => {
                self.draw_piece_on_board(to, role, color, true, board);
            }
        };
    }

    pub fn get_piece_rgba_image(
        &mut self,
        color: Color,
        role: &Role,
        width: u32,
        height: u32,
    ) -> &RgbaImage {
        let p = self
            .piece_map
            .entry(format!(
                "{}/{}_{}.svg",
                self.image_path,
                color.char(),
                role.char()
            ))
            .or_insert_with_key(|key| {
                log::debug!("Cache miss");
                let mut opt = usvg::Options::default();
                opt.path = Some(key.into());
                opt.dpi = 300.0;
                let svg = usvg::Tree::from_file(key, &opt).unwrap();
                let fit_to = usvg::FitTo::Height(height);
                let fitted = resvg::render(&svg, fit_to, None).unwrap();
                let piece = ImageBuffer::from_vec(fitted.width(), fitted.height(), fitted.take());

                piece.unwrap()
            });
        p
    }

    pub fn draw_piece_on_board(
        &mut self,
        square: &Square,
        role: &Role,
        color: Color,
        blank_target: bool,
        board: &mut RgbaImage,
    ) {
        log::debug!("Drawing {:?} {:?} on {:?}", color, role, square);
        if blank_target {
            self.blank_square_on_board(square, board);
        }

        let start_x = self.size / 8 * u32::from(square.file());
        let start_y = self.size - self.size / 8 * (u32::from(square.rank()) + 1);
        log::debug!("Piece coordinates: ({}, {})", start_x, start_y);

        let resized_piece = self.get_piece_rgba_image(color, role, self.size / 8, self.size / 8);
        imageops::overlay(board, resized_piece, start_x, start_y);
    }

    pub fn blank_square_on_board(&self, square: &Square, board: &mut RgbaImage) {
        let blank_square = match square.is_dark() {
            true => self.get_dark_square(),
            _ => self.get_light_square(),
        };

        let start_x = self.size / 8 * u32::from(square.file());
        let start_y = self.size - self.size / 8 * (u32::from(square.rank()) + 1);

        log::debug!("Blank square coordinates: ({}, {})", start_x, start_y);
        imageops::overlay(board, &blank_square, start_x, start_y);
    }
}

pub struct PGNGiffer {
    drawer: BoardDrawer,
    position: Chess,
    encoder: Encoder<BufWriter<File>>,
    delay: u16,
    counter: usize,
}

impl PGNGiffer {
    pub fn new(
        image_path: &str,
        board_size: u32,
        output_path: &str,
        ms_delay: u16,
        dark: [u8; 4],
        light: [u8; 4],
    ) -> Self {
        let file = File::create(output_path).unwrap();
        let buffer = BufWriter::with_capacity(1000, file);
        let mut encoder = Encoder::new(buffer, board_size as u16, board_size as u16, &[]).unwrap();
        encoder.set_repeat(Repeat::Infinite).unwrap();

        PGNGiffer {
            drawer: BoardDrawer::new(image_path.to_owned(), board_size as u32, dark, light),
            position: Chess::default(),
            encoder: encoder,
            delay: ms_delay,
            counter: 1,
        }
    }
}

impl Visitor for PGNGiffer {
    type Result = ();

    fn begin_game(&mut self) {
        log::info!("Rendering initial board");
        let pieces = self.position.board().pieces();
        let board = self.drawer.draw_position_from_empty(pieces);

        let mut frame = Frame::from_rgba_speed(
            self.drawer.size as u16,
            self.drawer.size as u16,
            &mut board.into_raw(),
            30,
        );
        frame.delay = self.delay;
        log::debug!("Encoding initial board frame");
        self.encoder.write_frame(&frame);
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true) // stay in the mainline
    }

    fn san(&mut self, san_plus: SanPlus) {
        if let Ok(m) = san_plus.san.to_move(&self.position) {
            log::info!("Rendering move {}", self.counter);
            let mut board = self.drawer.get_buffer();
            self.drawer
                .draw_move_on_board(&m, self.position.turn(), &mut board);

            let mut frame = Frame::from_rgba_speed(
                self.drawer.size as u16,
                self.drawer.size as u16,
                &mut board.into_raw(),
                30,
            );
            frame.delay = self.delay;
            log::debug!("Encoding frame for move {:?}", m);
            self.encoder.write_frame(&frame);

            self.position.play_unchecked(&m);
            self.counter += 1;
        }
    }

    fn end_game(&mut self) -> Self::Result {}
}

pub struct Chess2Gif {
    pgn: String,
    giffer: PGNGiffer,
}

impl Chess2Gif {
    pub fn new() -> Self {
        Self::new_from(std::env::args_os().into_iter()).unwrap_or_else(|e| e.exit())
    }

    pub fn new_from<I, T>(args: I) -> Result<Self, clap::Error>
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
                    .required(true)
                    .help("A PGN string for a chess game"),
            )
            .arg(
                Arg::with_name("output")
                    .short("o")
                    .takes_value(true)
                    .default_value("chess.gif")
                    .help("Write GIF to file"),
            )
            .arg(
                Arg::with_name("size")
                    .short("s")
                    .long("size")
                    .takes_value(true)
                    .default_value("512")
                    .help("The size of one side of the board in pixels"),
            )
            .arg(
                Arg::with_name("dark")
                    .short("d")
                    .long("dark")
                    .takes_value(true)
                    .number_of_values(4)
                    .default_value("238,238,210,1")
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
                    .default_value("118,150,86,1")
                    .require_delimiter(true)
                    .multiple(false)
                    .help("RGBA color to use for the light squares"),
            )
            .arg(
                Arg::with_name("pieces-path")
                    .long("pieces-path")
                    .takes_value(true)
                    .help("Path to directory containing images of chess pieces")
                    .default_value("pieces/"),
            );

        let matches = app.get_matches_from_safe(args)?;

        let size = u32::from_str_radix(matches.value_of("size").expect("Size must be defined"), 10)
            .unwrap();
        let pgn = matches.value_of("PGN").expect("PGN is required");
        let pieces_path = matches
            .value_of("pieces-path")
            .expect("Path to pieces must be defined");
        let output = matches.value_of("output").expect("Output must be defined");

        let dark: [u8; 4] = clap::values_t_or_exit!(matches, "dark", u8)
            .try_into()
            .unwrap();
        let light: [u8; 4] = clap::values_t_or_exit!(matches, "light", u8)
            .try_into()
            .unwrap();

        Ok(Chess2Gif {
            pgn: pgn.to_owned(),
            giffer: PGNGiffer::new(pieces_path, size, output, 100, dark, light),
        })
    }

    pub fn run(mut self) {
        let mut reader = BufferedReader::new_cursor(&self.pgn[..]);
        log::info!("Reading PGN");
        reader.read_game(&mut self.giffer);
        log::info!("Done!");
    }
}
