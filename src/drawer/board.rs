use image::{imageops, ImageBuffer, Rgba, RgbaImage};
use shakmaty::{self, Chess, File, Move, Position, Rank, Role, Square};
use tiny_skia::{self, Pixmap, PixmapPaint, Transform};
use usvg::FitTo;

use super::error::DrawerError;
use super::svgs::{FontSize, FontWeight, SVGForest, SVGTree};
use super::utils;

use crate::config::Color;

#[derive(Debug)]
pub struct BoardDrawer {
    size: u32,
    flip: bool,
    dark: Rgba<u8>,
    light: Rgba<u8>,
}

impl BoardDrawer {
    pub fn new(flip: bool, size: u32, dark: Color, light: Color) -> Result<Self, DrawerError> {
        Ok(BoardDrawer {
            size,
            flip,
            dark: image::Rgba(dark.to_arr()),
            light: image::Rgba(light.to_arr()),
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

    pub fn flip(&self) -> bool {
        self.flip
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
    pub fn draw_position(
        &mut self,
        position: &Chess,
        svgs: &SVGForest,
    ) -> Result<RgbaImage, DrawerError> {
        log::debug!("Drawing position");
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

        let mut board_img = ImageBuffer::new(self.size, self.size);
        for n in 0..9 {
            imageops::replace(&mut board_img, &column, (n * self.square_size()).into(), 0);
            imageops::flip_vertical_in_place(&mut column)
        }

        for rank in Rank::ALL.into_iter().rev() {
            for file in File::ALL {
                let square = Square::from_coords(file, rank);
                if let Some(piece) = position.board().piece_at(square) {
                    log::debug!("Drawing {:?} in {:?}", piece, square);
                    self.draw_piece(
                        &square,
                        &piece.role,
                        piece.color,
                        false,
                        &mut board_img,
                        None,
                        svgs,
                        false,
                    )?;
                } else {
                    self.draw_square(&square, &mut board_img, svgs)?;
                }
            }
        }

        self.draw_ranks(2, 6, &mut board_img, svgs)?;

        if self.flip == true {
            imageops::flip_horizontal_in_place(&mut board_img);
            imageops::flip_vertical_in_place(&mut board_img);
        }

        Ok(board_img)
    }

    pub fn draw_initial_position(&mut self, svgs: &SVGForest) -> Result<RgbaImage, DrawerError> {
        log::debug!("Drawing initial position");
        let position = Chess::default();
        let board_img = self.draw_position(&position, svgs)?;

        Ok(board_img)
    }

    pub fn draw_ranks(
        &mut self,
        from: u32,
        to: u32,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        for n in from..to {
            let square = Square::new((n * 8) + (self.flip as u32 * 7));
            self.draw_square(&square, img, svgs)?;
        }

        Ok(())
    }

    pub fn draw_move(
        &mut self,
        _move: &Move,
        color: shakmaty::Color,
        img: &mut RgbaImage,
        svgs: &SVGForest,
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
                self.draw_square(from, img, svgs)?;
                let blank_to_square = if capture.is_some() { true } else { false };

                if let Some(promoted) = promotion {
                    self.draw_piece(to, promoted, color, blank_to_square, img, None, svgs, false)?;
                } else {
                    self.draw_piece(to, role, color, blank_to_square, img, None, svgs, false)?;
                }
            }
            Move::EnPassant { from, to } => {
                self.draw_square(from, img, svgs)?;
                // Need to delete the pawn that was taken
                // This pawn is in the same Rank as from
                // And the same File as to
                let taken_pawn = Square::from_coords(to.file(), from.rank());
                self.draw_square(&taken_pawn, img, svgs)?;

                self.draw_piece(to, &Role::Pawn, color, true, img, None, svgs, false)?;
            }
            Move::Castle { king, rook } => {
                // King and Rook initial squares, e.g. E1 and H1 respectively.
                // Need to calculate where the pieces end up before drawing.
                let offset = if rook.file() > king.file() { 1 } else { -1 };

                self.draw_square(king, img, svgs)?;
                self.draw_square(rook, img, svgs)?;

                let rook_square = king.offset(offset * 1).unwrap();
                let king_square = king.offset(offset * 2).unwrap();
                self.draw_piece(
                    &king_square,
                    &Role::King,
                    color,
                    true,
                    img,
                    None,
                    svgs,
                    false,
                )?;
                self.draw_piece(
                    &rook_square,
                    &Role::Rook,
                    color,
                    true,
                    img,
                    None,
                    svgs,
                    false,
                )?;
            }
            Move::Put { role, to } => {
                self.draw_piece(to, role, color, true, img, None, svgs, false)?;
            }
        };

        if self.flip == true {
            imageops::flip_horizontal_in_place(img);
            imageops::flip_vertical_in_place(img);
        }

        Ok(())
    }

    pub fn draw_checked_king(
        &mut self,
        mut piece: utils::PieceInBoard,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        if self.flip == true {
            piece.flip_both()
        };

        self.draw_piece(
            &piece.square,
            &Role::King,
            piece.color,
            true,
            img,
            Some("check".to_string()),
            svgs,
            true,
        )
    }

    pub fn draw_win_king(
        &mut self,
        square: &Square,
        color: shakmaty::Color,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        self.draw_piece(
            square,
            &Role::King,
            color,
            true,
            img,
            Some("win".to_string()),
            svgs,
            false,
        )
    }

    pub fn draw_square(
        &mut self,
        square: &Square,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        log::debug!("Drawing square: {}", square);
        let pixmap =
            self.square_pixmap(self.square_size(), self.square_size(), square, svgs, false)?;
        let mut square_img = ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
            .ok_or(DrawerError::ImageTooBig {
                image: format!("{}x{} square", self.square_size(), self.square_size()),
            })?;

        let x = self.square_size() * u32::from(square.file());
        let y = self.size - self.square_size() * (u32::from(square.rank()) + 1);

        if self.flip == true {
            imageops::flip_vertical_in_place(&mut square_img);
            imageops::flip_horizontal_in_place(&mut square_img);
        }

        imageops::overlay(img, &square_img, x.into(), y.into());

        Ok(())
    }

    pub fn draw_piece(
        &mut self,
        square: &Square,
        role: &Role,
        color: shakmaty::Color,
        blank_target: bool,
        img: &mut RgbaImage,
        additional: Option<String>,
        svgs: &SVGForest,
        skip_flip: bool,
    ) -> Result<(), DrawerError> {
        log::debug!("Drawing {:?} {:?} on {:?}", color, role, square);
        if blank_target {
            self.draw_square(square, img, svgs)?;
        }

        let x = self.square_size() * u32::from(square.file());
        let y = self.size - self.square_size() * (u32::from(square.rank()) + 1);
        log::debug!("Piece coordinates: ({}, {})", x, y);

        let height = self.square_size();
        let mut resized_piece = self.piece_image(
            color, square, role, height, height, additional, svgs, skip_flip,
        )?;

        if self.flip == true && skip_flip == false {
            imageops::flip_vertical_in_place(&mut resized_piece);
            imageops::flip_horizontal_in_place(&mut resized_piece);
        }

        imageops::replace(img, &resized_piece, x.into(), y.into());

        Ok(())
    }

    pub fn piece_image<'a>(
        &'a mut self,
        piece_color: shakmaty::Color,
        square: &'a Square,
        role: &'a Role,
        height: u32,
        width: u32,
        additional: Option<String>,
        svgs: &SVGForest,
        skip_flip: bool,
    ) -> Result<RgbaImage, DrawerError> {
        let fit_to = FitTo::Height(height);
        let piece_tree = SVGTree::Piece {
            role: *role,
            color: piece_color,
            additional: additional,
        };
        let rtree = svgs.load_svg_tree(&piece_tree)?;
        let mut pixmap = self.square_pixmap(height, width, square, svgs, skip_flip)?;
        resvg::render(&rtree, fit_to, Transform::identity(), pixmap.as_mut()).ok_or(
            DrawerError::SVGRenderError {
                svg: format!("{}_{}.svg", piece_color.char(), role.char()),
            },
        )?;

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
        svgs: &SVGForest,
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
        let coordinate_tree = SVGTree::Str {
            s: coordinate.to_string(),
            string_color: coord_color,
            background_color: square_color,
            height,
            width,
            x,
            y,
            font_weight: FontWeight::Bold,
            font_size: FontSize::Unit(height as f32, "px".to_string()),
        };

        let rtree = svgs.load_svg_tree(&coordinate_tree)?;

        let fit_to = FitTo::Height(height);
        resvg::render(&rtree, fit_to, Transform::identity(), pixmap.as_mut()).ok_or(
            DrawerError::SVGRenderError {
                svg: coordinate.to_string(),
            },
        )?;

        Ok(pixmap)
    }

    pub fn square_pixmap(
        &mut self,
        height: u32,
        width: u32,
        square: &Square,
        svgs: &SVGForest,
        skip_flip: bool,
    ) -> Result<Pixmap, DrawerError> {
        let mut pixmap = Pixmap::new(width, height).unwrap();
        match square.is_dark() {
            true => pixmap.fill(self.dark_color()),
            false => pixmap.fill(self.light_color()),
        };
        let flip = self.flip && !skip_flip;
        if utils::has_coordinate(square, flip) {
            if (square.rank() == Rank::First && self.flip == false)
                || (square.rank() == Rank::Eighth && self.flip == true)
            {
                let file_pixmap = self.coordinate_pixmap(
                    square.file().char(),
                    square,
                    self.size / 32,
                    self.size / 32,
                    5,
                    75,
                    svgs,
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

            if (square.file() == File::A && self.flip == false)
                || (square.file() == File::H && self.flip == true)
            {
                let rank_pixmap = self.coordinate_pixmap(
                    square.rank().char(),
                    square,
                    self.size / 32,
                    self.size / 32,
                    5,
                    75,
                    svgs,
                )?;
                let paint = PixmapPaint::default();
                let transform = Transform::default();
                pixmap.draw_pixmap(0, 0, rank_pixmap.as_ref(), &paint, transform, None);
            }
        }

        Ok(pixmap)
    }

    pub fn str_pixmap(
        &mut self,
        height: u32,
        width: u32,
        x: u32,
        y: u32,
        s: &str,
        str_color: Rgba<u8>,
        background_color: Rgba<u8>,
        svgs: &SVGForest,
    ) -> Result<Pixmap, DrawerError> {
        let mut pixmap = Pixmap::new(width, height).unwrap();

        let str_tree = SVGTree::Str {
            s: s.to_string(),
            string_color: str_color,
            background_color: background_color,
            height,
            width,
            x,
            y,
            font_weight: FontWeight::Bold,
            font_size: FontSize::Unit(height as f32 * 0.5, "px".to_string()),
        };

        let rtree = svgs.load_svg_tree(&str_tree)?;

        let fit_to = FitTo::Height(height);
        resvg::render(&rtree, fit_to, Transform::identity(), pixmap.as_mut())
            .ok_or(DrawerError::SVGRenderError { svg: s.to_string() })?;

        Ok(pixmap)
    }

    pub fn draw_player_bar(
        &mut self,
        player: &str,
        player_color: shakmaty::Color,
        bottom: bool,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        let mut pixmap = Pixmap::new(self.size, self.square_size()).unwrap();
        let (color, background_color, y) = match player_color {
            shakmaty::Color::White => {
                pixmap.fill(self.light_color());
                (self.dark, self.light, 65)
            }
            shakmaty::Color::Black => {
                pixmap.fill(self.dark_color());
                (self.light, self.dark, 65)
            }
        };

        let player_pixmap = self.str_pixmap(
            self.square_size(),
            self.size,
            2,
            y,
            player,
            color,
            background_color,
            svgs,
        )?;

        let paint = PixmapPaint::default();
        let transform = Transform::default();
        pixmap.draw_pixmap(0, 0, player_pixmap.as_ref(), &paint, transform, None);

        let player_image = ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
            .ok_or(DrawerError::ImageTooBig {
                image: format!("{}.svg", player),
            })?;

        let y = if bottom == true {
            self.size + self.square_size()
        } else {
            0
        };

        log::debug!("Bottom: {:?}, y: {}", bottom, y);
        imageops::overlay(img, &player_image, 0, y.into());

        Ok(())
    }

    pub fn draw_player_clock(
        &mut self,
        clock: &str,
        player_color: shakmaty::Color,
        bottom: bool,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        let mut pixmap = Pixmap::new(self.square_size() * 2, self.square_size() * 3 / 4).unwrap();
        let (color, background_color) = match player_color {
            shakmaty::Color::White => {
                pixmap.fill(self.dark_color());
                (self.light, self.dark)
            }
            shakmaty::Color::Black => {
                pixmap.fill(self.light_color());
                (self.dark, self.light)
            }
        };

        let clock_pixmap = self.str_pixmap(
            self.square_size() * 3 / 4,
            self.square_size() * 2,
            10,
            65,
            clock,
            color,
            background_color,
            svgs,
        )?;

        let paint = PixmapPaint::default();
        let transform = Transform::default();
        pixmap.draw_pixmap(0, 0, clock_pixmap.as_ref(), &paint, transform, None);

        let player_image = ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take())
            .ok_or(DrawerError::ImageTooBig {
                image: format!("{}.svg", clock),
            })?;

        let y = if bottom == true {
            self.size + self.square_size()
        } else {
            0
        };

        log::debug!("Bottom: {:?}, y: {}", bottom, y);
        imageops::overlay(
            img,
            &player_image,
            (self.size - (self.square_size() * 17 / 8)).into(), // This leaves a 1 / 8 * square_size margin on the right side
            (y + self.square_size() / 8).into(),
        );

        Ok(())
    }

    pub fn add_player_bar_space(&self, img: RgbaImage) -> RgbaImage {
        let mut new_img = RgbaImage::new(self.size, self.size + self.square_size() * 2);
        imageops::replace(&mut new_img, &img, 0, self.square_size().into());
        new_img
    }

    pub fn draw_player_clocks(
        &mut self,
        white_clock: &str,
        black_clock: &str,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        self.draw_player_clock(white_clock, shakmaty::Color::White, !self.flip, img, svgs)?;
        self.draw_player_clock(black_clock, shakmaty::Color::Black, self.flip, img, svgs)?;

        Ok(())
    }

    pub fn draw_one_player_clock(
        &mut self,
        clock: &str,
        color: shakmaty::Color,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        match color {
            shakmaty::Color::White => {
                self.draw_player_clock(clock, color, !self.flip, img, svgs)?
            }
            shakmaty::Color::Black => self.draw_player_clock(clock, color, self.flip, img, svgs)?,
        }

        Ok(())
    }

    pub fn draw_player_bars(
        &mut self,
        white_player: &str,
        black_player: &str,
        img: &mut RgbaImage,
        svgs: &SVGForest,
    ) -> Result<(), DrawerError> {
        self.draw_player_bar(white_player, shakmaty::Color::White, !self.flip, img, svgs)?;
        self.draw_player_bar(black_player, shakmaty::Color::Black, self.flip, img, svgs)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drawer::SVGFontConfig;

    #[test]
    fn test_square_image() {
        let dark_arr: [u8; 4] = [249, 100, 100, 1];
        let light_arr: [u8; 4] = [249, 100, 100, 1];
        let dark: Color = Color(dark_arr);
        let light: Color = Color(light_arr);
        let mut drawer = BoardDrawer::new(false, 80, dark, light).unwrap();

        let square = Square::new(0); // A1 is dark
        let expected = ImageBuffer::from_pixel(10, 10, image::Rgba(dark_arr));
        assert_eq!(expected, drawer.square_image(&square));

        let square = Square::new(7); // H1 is light
        let expected = ImageBuffer::from_pixel(10, 10, image::Rgba(light_arr));
        assert_eq!(expected, drawer.square_image(&square));
    }

    #[test]
    fn test_sizes() {
        let dark: Color = Color([249, 100, 100, 1]);
        let light: Color = Color([255, 253, 253, 1]);
        let drawer = BoardDrawer::new(false, 80, dark, light).unwrap();

        assert_eq!(drawer.size(), 80);
        assert_eq!(drawer.square_size(), 10);
    }

    #[test]
    fn test_square_pixmap() {
        let dark: Color = Color([249, 100, 100, 1]);
        let light: Color = Color([255, 253, 253, 1]);
        let mut drawer = BoardDrawer::new(false, 80, dark, light).unwrap();

        let mut pixmap = Pixmap::new(10, 10).unwrap();
        let square = Square::new(9); // B2 is dark
        pixmap.fill(tiny_skia::Color::from_rgba8(249, 100, 100, 255));

        let config = SVGFontConfig::default();
        let svgs = SVGForest::new(config, "svgs", "cburnett", "terminations").unwrap();
        let result = drawer.square_pixmap(10, 10, &square, &svgs, false).unwrap();
        assert_eq!(pixmap, result);

        let square = Square::new(10); // C2 is dark
        pixmap.fill(tiny_skia::Color::from_rgba8(255, 253, 253, 255));

        let config = SVGFontConfig::default();
        let svgs = SVGForest::new(config, "svgs", "cburnett", "terminations").unwrap();
        let result = drawer.square_pixmap(10, 10, &square, &svgs, false).unwrap();
        assert_eq!(pixmap, result);
    }
}
