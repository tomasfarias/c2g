use std::collections::HashMap;
use std::convert::TryInto;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead, BufWriter, Read};
use std::mem::replace;

#[macro_use]
extern crate clap;

use clap::{App, Arg};
use gif::{Encoder, Frame, Repeat};
use image::{imageops, ImageBuffer, Rgba, RgbaImage};
use log;
use pgn_reader::{BufferedReader, SanPlus, Skip, Visitor};
use resvg;
use shakmaty::{self, Chess, Color, Move, Pieces, Position, Role, Setup, Square};
use tiny_skia::Pixmap;
use usvg;
use usvg::fontdb;

pub struct BoardDrawer {
    image_path: String,
    svg_options: usvg::Options,
    size: u32,
    piece_map: HashMap<String, RgbaImage>,
    dark: Rgba<u8>,
    light: Rgba<u8>,
    coordinates_margin: u32,
}

impl BoardDrawer {
    pub fn new(
        image_path: String,
        font_path: Option<String>,
        size: u32,
        dark: [u8; 4],
        light: [u8; 4],
        coordinates: bool,
    ) -> Self {
        let mut piece_map = HashMap::new();
        piece_map.reserve(12);

        let coordinates_margin = if coordinates { size / 8 / 6 } else { 0 };

        let mut opt = usvg::Options::default();

        if let Some(p) = font_path {
            let mut fonts = fontdb::Database::new();
            fonts.load_font_file(p);
            opt.fontdb = fonts;
            // There should only be 1 font in DB
            opt.font_family = (*(opt.fontdb.faces())[0].family).to_owned();
        }

        BoardDrawer {
            image_path: image_path,
            svg_options: opt,
            size: size,
            piece_map: piece_map,
            coordinates_margin: coordinates_margin,
            dark: image::Rgba(dark),
            light: image::Rgba(light),
        }
    }

    pub fn total_size(&self) -> u32 {
        self.size + self.coordinates_margin
    }

    pub fn get_buffer(&self) -> RgbaImage {
        ImageBuffer::new(
            self.size + self.coordinates_margin,
            self.size + self.coordinates_margin,
        )
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
                self.light
            } else {
                self.dark
            }
        });

        let mut board = ImageBuffer::new(self.size, self.size);
        for n in 0..9 {
            imageops::replace(&mut board, &column, n * self.size / 8, 0);
            imageops::flip_vertical_in_place(&mut column)
        }

        self.draw_coordinates_on_board(&mut board);

        for (square, piece) in pieces {
            log::debug!("Initializing {:?} in {:?}", piece, square);
            self.draw_piece_on_board(&square, &piece.role, piece.color, false, &mut board);
        }

        board
    }

    pub fn draw_coordinates_on_board(&mut self, board: &mut RgbaImage) {
        if self.coordinates_margin == 0 {
            return;
        }

        let mut margin_board = ImageBuffer::from_pixel(
            self.size + self.coordinates_margin,
            self.size + self.coordinates_margin,
            self.light,
        );
        imageops::replace(&mut margin_board, board, self.coordinates_margin, 0);

        replace(board, margin_board);

        for n in 0..8 {
            self.draw_rank_on_board(n, board);
            self.draw_file_on_board(n, board);
        }
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
                log::debug!("Cache missed for {}", key);
                let svg = usvg::Tree::from_file(key, &usvg::Options::default()).unwrap();
                let fit_to = usvg::FitTo::Height(height);
                let mut pixmap = Pixmap::new(height, height).unwrap();
                resvg::render(&svg, fit_to, pixmap.as_mut()).unwrap();

                let piece = ImageBuffer::from_vec(pixmap.width(), pixmap.height(), pixmap.take());

                piece.unwrap()
            });
        p
    }

    pub fn get_coordinate_rgba_image(
        &mut self,
        coordinate: char,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
    ) -> RgbaImage {
        log::debug!("Generating svg text: {}", coordinate);
        let svg_string = format!(
            "<svg xmlns:svg=\"http://www.w3.org/2000/svg\" xmlns=\"http://www.w3.org/2000/svg\" version=\"1.0\" height=\"{}\" width=\"{}\"> \
             <text font-size=\"100%\" x=\"{}\" y=\"{}\" fill=\"rgb({}, {}, {})\" stroke=\"rgb({}, {}, {})\" stroke-width=\"1%\">{}</text> \
             </svg>", height, width, x, y, self.dark[0], self.dark[1], self.dark[2], self.dark[0], self.dark[1], self.dark[2], coordinate
        );
        let svg = usvg::Tree::from_str(&svg_string, &self.svg_options).unwrap();

        let fit_to = usvg::FitTo::Original;
        let mut pixmap = Pixmap::new(width, height).unwrap();
        resvg::render(&svg, fit_to, pixmap.as_mut()).unwrap();

        let coordinate_img = ImageBuffer::from_vec(pixmap.width(), pixmap.height(), pixmap.take());

        coordinate_img.unwrap()
    }

    pub fn draw_rank_on_board(&mut self, index: u32, board: &mut RgbaImage) {
        let rank = shakmaty::Rank::new(index);
        log::debug!("Drawing rank: {}", rank.char());
        let coordinate = self.get_coordinate_rgba_image(
            rank.char(),
            self.size / 8,
            self.coordinates_margin,
            self.coordinates_margin / 4,
            self.size / 16,
        );

        let x = 0;
        let y = self.size / 8 * (7 - index);
        imageops::overlay(board, &coordinate, x, y);
    }

    pub fn draw_file_on_board(&mut self, index: u32, board: &mut RgbaImage) {
        let file = shakmaty::File::new(index);
        log::debug!("Drawing file: {}", file.char());

        let coordinate = self.get_coordinate_rgba_image(
            file.char(),
            self.coordinates_margin,
            self.size / 8,
            self.size / 16,
            self.coordinates_margin * 3 / 4,
        );

        let x = self.coordinates_margin + (self.size / 8 * index);
        let y = self.size;
        imageops::overlay(board, &coordinate, x, y);
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

        let x = self.coordinates_margin + self.size / 8 * u32::from(square.file());
        let y = self.size - self.size / 8 * (u32::from(square.rank()) + 1);
        log::debug!("Piece coordinates: ({}, {})", x, y);

        let resized_piece = self.get_piece_rgba_image(color, role, self.size / 8, self.size / 8);
        imageops::overlay(board, resized_piece, x, y);
    }

    pub fn blank_square_on_board(&self, square: &Square, board: &mut RgbaImage) {
        let blank_square = match square.is_dark() {
            true => self.get_dark_square(),
            _ => self.get_light_square(),
        };

        let x = self.coordinates_margin + self.size / 8 * u32::from(square.file());
        let y = self.size - self.size / 8 * (u32::from(square.rank()) + 1);
        log::debug!("Blank square coordinates: ({}, {})", x, y);
        imageops::replace(board, &blank_square, x, y);
    }
}

pub struct PGNGiffer<'a> {
    drawer: BoardDrawer,
    position: Chess,
    encoder: Encoder<BufWriter<fs::File>>,
    delay: u16,
    frames: Vec<Frame<'a>>,
}

impl<'a> PGNGiffer<'a> {
    pub fn new(
        image_path: &str,
        font_path: &str,
        board_size: u32,
        output_path: &str,
        ms_delay: u16,
        dark: [u8; 4],
        light: [u8; 4],
        coordinates: bool,
    ) -> Self {
        let file = fs::File::create(output_path).unwrap();
        let buffer = BufWriter::with_capacity(1000, file);

        let drawer = BoardDrawer::new(
            image_path.to_owned(),
            Some(font_path.to_owned()),
            board_size as u32,
            dark,
            light,
            coordinates,
        );

        let mut encoder = Encoder::new(
            buffer,
            drawer.total_size() as u16,
            drawer.total_size() as u16,
            &[],
        )
        .unwrap();
        encoder.set_repeat(Repeat::Infinite).unwrap();

        PGNGiffer {
            drawer: drawer,
            position: Chess::default(),
            encoder: encoder,
            delay: ms_delay,
            frames: Vec::new(),
        }
    }
}

impl<'a> Visitor for PGNGiffer<'a> {
    type Result = ();

    fn begin_game(&mut self) {
        log::info!("Rendering initial board");
        let pieces = self.position.board().pieces();
        let board = self.drawer.draw_position_from_empty(pieces);

        let mut frame = Frame::from_rgba_speed(
            self.drawer.total_size() as u16,
            self.drawer.total_size() as u16,
            &mut board.into_raw(),
            30,
        );
        frame.delay = self.delay;
        self.frames.push(frame);
        // log::debug!("Encoding initial board frame");
        // self.encoder.write_frame(&frame);
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true) // stay in the mainline
    }

    fn san(&mut self, san_plus: SanPlus) {
        if let Ok(m) = san_plus.san.to_move(&self.position) {
            let mut board = self.drawer.get_buffer();
            self.drawer
                .draw_move_on_board(&m, self.position.turn(), &mut board);

            let mut frame = Frame::from_rgba_speed(
                self.drawer.total_size() as u16,
                self.drawer.total_size() as u16,
                &mut board.into_raw(),
                30,
            );
            frame.delay = self.delay;
            self.frames.push(frame);
            log::debug!("Encoding frame for move {:?}", m);
            // self.encoder.write_frame(&frame);

            self.position.play_unchecked(&m);
        }
    }

    fn end_game(&mut self) -> Self::Result {
        if let Some(last) = self.frames.last_mut() {
            (*last).delay = self.delay * 5;
        }
        for f in self.frames.iter() {
            self.encoder.write_frame(f);
        }
    }
}

pub struct Chess2Gif<'a> {
    pgn: Option<String>,
    giffer: PGNGiffer<'a>,
}

impl<'a> Chess2Gif<'a> {
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
                    .default_value("512")
                    .help("The size of one side of the board in pixels"),
            )
            .arg(
                Arg::with_name("no-coordinates")
                    .long("no-coordinates")
                    .takes_value(false)
                    .help("Do not draw coordinates on board"),
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
                    .help("Path to directory containing images of chess pieces")
                    .default_value("pieces/"),
            )
            .arg(
                Arg::with_name("font-path")
                    .long("font-path")
                    .takes_value(true)
                    .help("Path to directory containing images of chess coordinates")
                    .default_value("font/roboto.ttf"),
            );

        let matches = app.get_matches_from_safe(args)?;

        let size = u32::from_str_radix(matches.value_of("size").expect("Size must be defined"), 10)
            .unwrap();

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

        let coordinates = if matches.is_present("no-coordinates") {
            false
        } else {
            true
        };

        let dark: [u8; 4] = clap::values_t_or_exit!(matches, "dark", u8)
            .try_into()
            .unwrap();
        let light: [u8; 4] = clap::values_t_or_exit!(matches, "light", u8)
            .try_into()
            .unwrap();

        Ok(Chess2Gif {
            pgn: pgn,
            giffer: PGNGiffer::new(
                pieces_path,
                font_path,
                size,
                output,
                100,
                dark,
                light,
                coordinates,
            ),
        })
    }

    pub fn run(mut self) {
        log::info!("Reading PGN");
        if let Some(pgn) = self.pgn {
            let mut reader = BufferedReader::new_cursor(&pgn[..]);
            reader.read_game(&mut self.giffer);
        } else {
            let stdin = io::stdin();
            let mut reader = BufferedReader::new(stdin);
            reader.read_game(&mut self.giffer);
        }
        log::info!("Done!");
    }
}
