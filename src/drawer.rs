use image::{imageops, ImageBuffer, Rgba, RgbaImage};
use include_dir::{include_dir, Dir};
use log;
use resvg;
use shakmaty::{self, File, Move, Pieces, Rank, Role, Square};
use thiserror::Error;
use tiny_skia::{self, Pixmap, PixmapPaint, Transform};
use usvg::{self, fontdb, FitTo, Options, Tree};

use crate::utils;

#[cfg(feature = "include-pieces")]
static PIECES_DIR: Dir = include_dir!("pieces/");

#[cfg(feature = "include-pieces")]
fn piece_svg_string(piece_path: &str) -> Result<String, DrawerError> {
    let piece_file = PIECES_DIR
        .get_file(&piece_path)
        .ok_or(DrawerError::PieceNotFound {
            piece: piece_path.to_owned(),
        })?;
    Ok(piece_file
        .contents_utf8()
        .expect("Failed to parse file contents")
        .to_owned())
}

#[cfg(not(feature = "include-pieces"))]
fn piece_svg_string(piece_path: &str) -> Result<String, DrawerError> {
    let mut f = fs::File::open(&piece_path).map_err(|_| DrawerError::PieceNotFound {
        piece: piece_path.to_owned(),
    })?;
    let mut piece_str = String::new();
    f.read_to_string(&mut piece_str)
        .map_err(|source| DrawerError::LoadFile { source: source })?;

    Ok(piece_str)
}

#[cfg(feature = "include-fonts")]
static FONTS_DIR: Dir = include_dir!("fonts/");

#[cfg(feature = "include-fonts")]
fn font_data(font: &str) -> Result<Vec<u8>, DrawerError> {
    let font_file = FONTS_DIR.get_file(font).ok_or(DrawerError::FontNotFound {
        font: font.to_owned(),
    })?;
    Ok(font_file.contents.to_vec())
}

#[cfg(not(feature = "include-fonts"))]
fn font_data(font: &str) -> Result<Vec<u8>, DrawerError> {
    let mut f = fs::File::open(font).map_err(|_| DrawerError::FontNotFound {
        font: font.to_owned(),
    })?;
    let mut buffer: Vec<u8> = Vec::new();
    f.read_to_end(&mut buffer)
        .map_err(|source| DrawerError::LoadFile { source: source })?;

    Ok(buffer)
}

pub struct SVGForest {
    pieces_dir: String,
    svg_options: Options,
}

impl SVGForest {
    pub fn new(pieces_dir: &str, font: &str) -> Result<Self, DrawerError> {
        let mut opt = Options::default();

        // Load font for coordinates
        let mut fonts = fontdb::Database::new();
        let font_data = font_data(font)?;
        fonts.load_font_data(font_data);
        opt.fontdb = fonts;
        opt.font_size = 16.0;
        // There should only be 1 font in DB
        opt.font_family = (*(opt.fontdb.faces())[0].family).to_owned();

        Ok(SVGForest {
            pieces_dir: pieces_dir.to_owned(),
            svg_options: opt,
        })
    }

    pub fn piece_tree(&self, role: &Role, color: &shakmaty::Color) -> Result<Tree, DrawerError> {
        let piece_path = format!("{}/{}_{}.svg", self.pieces_dir, color.char(), role.char());
        let svg_string = piece_svg_string(&piece_path)?;

        Tree::from_str(&svg_string, &self.svg_options)
            .map_err(|source| DrawerError::LoadPieceSVG { source: source })
    }

    pub fn coordinate_tree(
        &self,
        c: char,
        color: Rgba<u8>,
        background: Rgba<u8>,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
    ) -> Result<Tree, DrawerError> {
        let svg_string = format!(
            "<svg xmlns:svg=\"http://www.w3.org/2000/svg\" xmlns=\"http://www.w3.org/2000/svg\" version=\"1.0\" height=\"{}\" width=\"{}\" style=\"background-color:rgb({},{},{})\"> <text x=\"{}\" y=\"{}\" fill=\"rgb({}, {}, {})\" font-weight=\"600\">{}</text></svg>",
            height,
            width,
            background[0],
            background[1],
            background[2],
            x,
            y,
            color[0],
            color[1],
            color[2],
            c,
        );

        Tree::from_str(&svg_string, &self.svg_options).map_err(|source| {
            DrawerError::CoordinateSVG {
                source: source,
                coordinate: c,
            }
        })
    }
}

#[derive(Error, Debug)]
pub enum DrawerError {
    #[error("Piece {piece:?} not found in pieces directory")]
    PieceNotFound { piece: String },
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
    #[error("A correct SVG for {coordinate:?} could not be produced")]
    CoordinateSVG {
        source: usvg::Error,
        coordinate: char,
    },
}

pub struct BoardDrawer {
    svgs: SVGForest,
    size: u32,
    dark: Rgba<u8>,
    light: Rgba<u8>,
}

impl BoardDrawer {
    pub fn new(
        piece_path: &str,
        font: &str,
        size: u32,
        dark: [u8; 4],
        light: [u8; 4],
    ) -> Result<Self, DrawerError> {
        let svgs = SVGForest::new(piece_path, font)?;
        Ok(BoardDrawer {
            svgs: svgs,
            size: size,
            dark: image::Rgba(dark),
            light: image::Rgba(light),
        })
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

    pub fn square_size(&self) -> u32 {
        self.size / 8
    }

    pub fn square_image(&mut self, square: &Square) -> RgbaImage {
        match square.is_dark() {
            true => self.dark_square(),
            false => self.light_square(),
        }
    }

    pub fn dark_square(&self) -> RgbaImage {
        ImageBuffer::from_pixel(self.square_size(), self.square_size(), self.dark)
    }

    pub fn light_square(&self) -> RgbaImage {
        ImageBuffer::from_pixel(self.square_size(), self.square_size(), self.light)
    }

    pub fn draw_position_from_empty(&mut self, pieces: Pieces) -> Result<RgbaImage, DrawerError> {
        log::debug!("Drawing initial board");
        let mut counter = 1;
        let mut column = ImageBuffer::from_fn(self.square_size(), self.size, |_, y| {
            if y >= self.square_size() * counter {
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
            imageops::replace(&mut board, &column, n * self.square_size(), 0);
            imageops::flip_vertical_in_place(&mut column)
        }

        for (square, piece) in pieces {
            log::debug!("Initializing {:?} in {:?}", piece, square);
            self.draw_piece(&square, &piece.role, piece.color, false, &mut board)?;
        }
        self.draw_ranks(2, 6, &mut board)?;

        Ok(board)
    }

    pub fn draw_ranks(
        &mut self,
        from: u32,
        to: u32,
        img: &mut RgbaImage,
    ) -> Result<(), DrawerError> {
        for n in from..to {
            let square = Square::new(n * 8);
            self.draw_square(&square, img)?;
        }

        Ok(())
    }

    pub fn draw_move(
        &mut self,
        _move: &Move,
        color: shakmaty::Color,
        img: &mut RgbaImage,
    ) -> Result<(), DrawerError> {
        log::debug!("Drawing move: {:?}", _move);
        match _move {
            Move::Normal {
                role,
                from,
                capture,
                to,
                promotion,
            } => {
                self.draw_square(from, img)?;
                let blank_to_square = if capture.is_some() { true } else { false };

                if let Some(promoted) = promotion {
                    self.draw_piece(to, promoted, color, blank_to_square, img)?;
                } else {
                    self.draw_piece(to, role, color, blank_to_square, img)?;
                }
            }
            Move::EnPassant { from, to } => {
                self.draw_square(from, img)?;
                self.draw_piece(to, &Role::Pawn, color, true, img)?;
            }
            Move::Castle { king, rook } => {
                // King and Rook initial squares, e.g. E1 and H1 respectively.
                // Need to calculate where the pieces end up before drawing.
                let offset = if rook.file() > king.file() { 1 } else { -1 };

                self.draw_square(king, img)?;
                self.draw_square(rook, img)?;

                let rook_square = king.offset(offset * 1).unwrap();
                let king_square = king.offset(offset * 2).unwrap();
                self.draw_piece(&king_square, &Role::King, color, true, img)?;
                self.draw_piece(&rook_square, &Role::Rook, color, true, img)?;
            }
            Move::Put { role, to } => {
                self.draw_piece(to, role, color, true, img)?;
            }
        };

        Ok(())
    }

    pub fn draw_square(&mut self, square: &Square, img: &mut RgbaImage) -> Result<(), DrawerError> {
        log::debug!("Drawing square: {}", square);
        let pixmap = self.square_pixmap(self.square_size(), self.square_size(), square)?;
        let square_img = ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
            .ok_or(DrawerError::ImageTooBig {
                image: format!("{}x{} square", self.square_size(), self.square_size()),
            })?;

        let x = self.square_size() * u32::from(square.file());
        let y = self.size - self.square_size() * (u32::from(square.rank()) + 1);

        imageops::overlay(img, &square_img, x, y);

        Ok(())
    }

    pub fn draw_piece(
        &mut self,
        square: &Square,
        role: &Role,
        color: shakmaty::Color,
        blank_target: bool,
        img: &mut RgbaImage,
    ) -> Result<(), DrawerError> {
        log::debug!("Drawing {:?} {:?} on {:?}", color, role, square);
        if blank_target {
            self.draw_square(square, img)?;
        }

        let x = self.square_size() * u32::from(square.file());
        let y = self.size - self.square_size() * (u32::from(square.rank()) + 1);
        log::debug!("Piece coordinates: ({}, {})", x, y);

        let height = self.square_size();
        let resized_piece = self.piece_image(color, square, role, height, height)?;
        imageops::replace(img, &resized_piece, x, y);

        Ok(())
    }

    pub fn piece_image<'a>(
        &'a mut self,
        piece_color: shakmaty::Color,
        square: &'a Square,
        role: &'a Role,
        height: u32,
        width: u32,
    ) -> Result<RgbaImage, DrawerError> {
        let fit_to = FitTo::Height(height);
        let rtree = self.svgs.piece_tree(role, &piece_color)?;
        let mut pixmap = self.square_pixmap(height, width, square)?;
        resvg::render(&rtree, fit_to, pixmap.as_mut()).ok_or(DrawerError::SVGRenderError {
            svg: format!("{}_{}.svg", piece_color.char(), role.char()),
        })?;

        ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).ok_or(
            DrawerError::ImageTooBig {
                image: format!("{}_{}.svg", piece_color.char(), role.char()),
            },
        )
    }

    pub fn coordinate_pixmap(
        &mut self,
        coordinate: char,
        square: &Square,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
    ) -> Result<Pixmap, DrawerError> {
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
        let rtree = self.svgs.coordinate_tree(
            coordinate,
            coord_color,
            square_color,
            height,
            width,
            x,
            y,
        )?;

        let fit_to = FitTo::Height(height);
        resvg::render(&rtree, fit_to, pixmap.as_mut()).ok_or(DrawerError::SVGRenderError {
            svg: coordinate.to_string(),
        })?;

        Ok(pixmap)
    }

    pub fn square_pixmap(
        &mut self,
        height: u32,
        width: u32,
        square: &Square,
    ) -> Result<Pixmap, DrawerError> {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        match square.is_dark() {
            true => pixmap.fill(self.dark_color()),
            false => pixmap.fill(self.light_color()),
        };
        if utils::has_coordinate(square) {
            if square.rank() == Rank::First {
                let file_pixmap = self.coordinate_pixmap(
                    square.file().char(),
                    square,
                    self.size / 32,
                    self.size / 32,
                    self.size / 128,
                    self.size / 32 - self.size / 128,
                )?;
                let paint = PixmapPaint::default();
                let transform = Transform::default();
                pixmap.draw_pixmap(
                    (self.square_size() - self.size / 32) as i32,
                    (self.square_size() - self.size / 32) as i32,
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
                )?;
                let paint = PixmapPaint::default();
                let transform = Transform::default();
                pixmap.draw_pixmap(0, 0, rank_pixmap.as_ref(), &paint, transform, None);
            }
        }

        Ok(pixmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_square_image() {
        let dark: [u8; 4] = [249, 100, 100, 1];
        let light: [u8; 4] = [255, 253, 253, 1];
        let mut drawer = BoardDrawer::new("some/path/".to_string(), None, 80, dark, light).unwrap();

        let square = Square::new(0); // A1 is dark
        let expected = ImageBuffer::from_pixel(10, 10, image::Rgba(dark));
        assert_eq!(expected, drawer.square_image(&square));

        let square = Square::new(7); // H1 is light
        let expected = ImageBuffer::from_pixel(10, 10, image::Rgba(light));
        assert_eq!(expected, drawer.square_image(&square));
    }

    #[test]
    fn test_sizes() {
        let dark: [u8; 4] = [249, 100, 100, 1];
        let light: [u8; 4] = [255, 253, 253, 1];
        let drawer = BoardDrawer::new("some/path/".to_string(), None, 80, dark, light).unwrap();

        assert_eq!(drawer.size(), 80);
        assert_eq!(drawer.square_size(), 10);
    }

    #[test]
    fn test_square_pixmap() {
        let dark: [u8; 4] = [249, 100, 100, 1];
        let light: [u8; 4] = [255, 253, 253, 1];
        let mut drawer = BoardDrawer::new("some/path/".to_string(), None, 80, dark, light).unwrap();

        let mut pixmap = Pixmap::new(10, 10).unwrap();
        let square = Square::new(9); // B2 is dark
        pixmap.fill(tiny_skia::Color::from_rgba8(249, 100, 100, 255));
        let result = drawer.square_pixmap(10, 10, &square).unwrap();
        assert_eq!(pixmap, result);

        let square = Square::new(10); // C2 is dark
        pixmap.fill(tiny_skia::Color::from_rgba8(255, 253, 253, 255));
        let result = drawer.square_pixmap(10, 10, &square).unwrap();
        assert_eq!(pixmap, result);
    }
}