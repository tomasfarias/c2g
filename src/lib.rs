use std::convert::TryInto;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufWriter};

#[macro_use]
extern crate clap;

use clap::{App, Arg};
use gif::{Encoder, Frame, Repeat};
use image::{imageops, ImageBuffer, Rgba, RgbaImage};
use log;
use pgn_reader::{BufferedReader, SanPlus, Skip, Visitor};
use resvg;
use shakmaty::{self, Chess, Color, File, Move, Pieces, Position, Rank, Role, Setup, Square};
use tiny_skia::{self, Pixmap};
use usvg::{self, fontdb};

pub fn has_coordinate(s: &Square) -> bool {
    if s.rank() == Rank::First || s.file() == File::A {
        true
    } else {
        false
    }
}

pub struct BoardDrawer {
    image_path: String,
    svg_options: usvg::Options,
    size: u32,
    dark: Rgba<u8>,
    light: Rgba<u8>,
}

impl BoardDrawer {
    pub fn new(
        image_path: String,
        font_path: Option<String>,
        size: u32,
        dark: [u8; 4],
        light: [u8; 4],
    ) -> Self {
        let mut opt = usvg::Options::default();

        if let Some(p) = font_path {
            let mut fonts = fontdb::Database::new();
            fonts.load_font_file(p);
            opt.fontdb = fonts;
            opt.font_size = 16.0;
            // There should only be 1 font in DB
            opt.font_family = (*(opt.fontdb.faces())[0].family).to_owned();
        }

        BoardDrawer {
            image_path: image_path,
            svg_options: opt,
            size: size,
            dark: image::Rgba(dark),
            light: image::Rgba(light),
        }
    }

    pub fn dark_color(&mut self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.dark[0], self.dark[1], self.dark[2], self.dark[3] * 255)
    }

    pub fn light_color(&mut self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(
            self.light[0],
            self.light[1],
            self.light[2],
            self.light[3] * 255,
        )
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn image_buffer(&self) -> RgbaImage {
        ImageBuffer::new(self.size, self.size)
    }

    pub fn square_image(&mut self, square: &Square) -> RgbaImage {
        match square.is_dark() {
            true => self.dark_square(),
            false => self.light_square(),
        }
    }

    pub fn dark_square(&self) -> RgbaImage {
        ImageBuffer::from_pixel(self.size / 8, self.size / 8, self.dark)
    }

    pub fn light_square(&self) -> RgbaImage {
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

        for (square, piece) in pieces {
            log::debug!("Initializing {:?} in {:?}", piece, square);
            self.draw_piece(&square, &piece.role, piece.color, false, &mut board);
        }
        self.draw_ranks(2, 6, &mut board);

        board
    }

    pub fn draw_ranks(&mut self, from: u32, to: u32, img: &mut RgbaImage) {
        for n in from..to {
            let square = shakmaty::Square::new(n * 8);
            self.draw_square(&square, img);
        }
    }

    pub fn draw_move(&mut self, _move: &Move, color: Color, img: &mut RgbaImage) {
        log::debug!("Drawing move: {:?}", _move);
        match _move {
            Move::Normal {
                role,
                from,
                capture,
                to,
                promotion,
            } => {
                self.draw_square(from, img);
                let blank_to_square = if capture.is_some() { true } else { false };

                if let Some(promoted) = promotion {
                    self.draw_piece(to, promoted, color, blank_to_square, img);
                } else {
                    self.draw_piece(to, role, color, blank_to_square, img);
                }
            }
            Move::EnPassant { from, to } => {
                self.draw_square(from, img);
                self.draw_piece(to, &Role::Pawn, color, true, img);
            }
            Move::Castle { king, rook } => {
                // King and Rook initial squares, e.g. E1 and H1 respectively.
                // Need to calculate where the pieces end up before drawing.
                let offset = if rook.file() > king.file() { 1 } else { -1 };

                self.draw_square(king, img);
                self.draw_square(rook, img);

                let rook_square = king.offset(offset * 1).unwrap();
                let king_square = king.offset(offset * 2).unwrap();
                self.draw_piece(&king_square, &Role::King, color, true, img);
                self.draw_piece(&rook_square, &Role::Rook, color, true, img);
            }
            Move::Put { role, to } => {
                self.draw_piece(to, role, color, true, img);
            }
        };
    }

    pub fn square_pixmap(&mut self, height: u32, width: u32, square: &Square) -> Pixmap {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        match square.is_dark() {
            true => pixmap.fill(self.dark_color()),
            false => pixmap.fill(self.light_color()),
        };
        if has_coordinate(square) {
            if square.rank() == Rank::First {
                let file_pixmap = self.coordinate_pixmap(
                    square.file().char(),
                    square,
                    self.size / 32,
                    self.size / 32,
                    self.size / 128,
                    self.size / 32 - self.size / 128,
                );
                let paint = tiny_skia::PixmapPaint::default();
                let transform = tiny_skia::Transform::default();
                pixmap.draw_pixmap(
                    (self.size / 8 - self.size / 32) as i32,
                    (self.size / 8 - self.size / 32) as i32,
                    file_pixmap.as_ref(),
                    &paint,
                    transform,
                    None,
                );
            }
            if square.file() == File::A {
                let rank_pixmap = self.coordinate_pixmap(
                    square.rank().char(),
                    square,
                    self.size / 32,
                    self.size / 32,
                    self.size / 128,
                    self.size / 32,
                );
                let paint = tiny_skia::PixmapPaint::default();
                let transform = tiny_skia::Transform::default();
                pixmap.draw_pixmap(0, 0, rank_pixmap.as_ref(), &paint, transform, None);
            }
        }

        pixmap
    }

    pub fn piece_image<'a>(
        &'a mut self,
        piece_color: Color,
        square: &'a Square,
        role: &'a Role,
        height: u32,
        width: u32,
    ) -> RgbaImage {
        let fit_to = usvg::FitTo::Height(height);
        let file_path = format!(
            "{}/{}_{}.svg",
            self.image_path,
            piece_color.char(),
            role.char()
        );
        let rtree = usvg::Tree::from_file(file_path, &self.svg_options).unwrap();
        let mut pixmap = self.square_pixmap(height, width, square);
        resvg::render(&rtree, fit_to, pixmap.as_mut()).unwrap();

        ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).unwrap()
    }

    pub fn coordinate_pixmap(
        &mut self,
        coordinate: char,
        square: &Square,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
    ) -> Pixmap {
        log::debug!("Generating svg text: {}", coordinate);
        let mut pixmap = Pixmap::new(width, height).unwrap();
        let (square_color, coord_color) = match square.is_dark() {
            true => {
                pixmap.fill(self.dark_color());
                (self.dark, self.light)
            }
            false => {
                pixmap.fill(self.light_color());
                (self.light, self.dark)
            }
        };

        let svg_string = format!(
            "<svg xmlns:svg=\"http://www.w3.org/2000/svg\" xmlns=\"http://www.w3.org/2000/svg\" version=\"1.0\" height=\"{}\" width=\"{}\" style=\"background-color:rgb({},{},{})\"> <text x=\"{}\" y=\"{}\" fill=\"rgb({}, {}, {})\" font-weight=\"600\">{}</text></svg>",
            height,
            width,
            square_color[0],
            square_color[1],
            square_color[2],
            x,
            y,
            coord_color[0],
            coord_color[1],
            coord_color[2],
            coordinate,
        );

        let rtree = usvg::Tree::from_str(&svg_string, &self.svg_options).unwrap();
        let fit_to = usvg::FitTo::Height(height);

        resvg::render(&rtree, fit_to, pixmap.as_mut()).unwrap();

        pixmap
    }

    pub fn draw_square(&mut self, square: &Square, img: &mut RgbaImage) {
        log::debug!("Drawing square: {}", square);
        let pixmap = self.square_pixmap(self.size / 8, self.size / 8, square);
        let square_img =
            ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).unwrap();

        let x = self.size / 8 * u32::from(square.file());
        let y = self.size - self.size / 8 * (u32::from(square.rank()) + 1);

        imageops::overlay(img, &square_img, x, y);
    }

    pub fn draw_piece(
        &mut self,
        square: &Square,
        role: &Role,
        color: Color,
        blank_target: bool,
        img: &mut RgbaImage,
    ) {
        log::debug!("Drawing {:?} {:?} on {:?}", color, role, square);
        if blank_target {
            self.draw_square(square, img);
        }

        let x = self.size / 8 * u32::from(square.file());
        let y = self.size - self.size / 8 * (u32::from(square.rank()) + 1);
        log::debug!("Piece coordinates: ({}, {})", x, y);

        let height = self.size / 8;
        let resized_piece = self.piece_image(color, square, role, height, height);
        imageops::replace(img, &resized_piece, x, y);
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
    ) -> Self {
        let file = fs::File::create(output_path).unwrap();
        let buffer = BufWriter::with_capacity(1000, file);

        let drawer = BoardDrawer::new(
            image_path.to_owned(),
            Some(font_path.to_owned()),
            board_size as u32,
            dark,
            light,
        );

        let mut encoder =
            Encoder::new(buffer, drawer.size() as u16, drawer.size() as u16, &[]).unwrap();
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
            self.drawer.size() as u16,
            self.drawer.size() as u16,
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
            let mut board = self.drawer.image_buffer();
            self.drawer.draw_move(&m, self.position.turn(), &mut board);

            let mut frame = Frame::from_rgba_speed(
                self.drawer.size() as u16,
                self.drawer.size() as u16,
                &mut board.into_raw(),
                10,
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

        let dark: [u8; 4] = clap::values_t_or_exit!(matches, "dark", u8)
            .try_into()
            .unwrap();
        let light: [u8; 4] = clap::values_t_or_exit!(matches, "light", u8)
            .try_into()
            .unwrap();

        Ok(Chess2Gif {
            pgn: pgn,
            giffer: PGNGiffer::new(pieces_path, font_path, size, output, 100, dark, light),
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
