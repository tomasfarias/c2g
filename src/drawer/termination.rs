use std::fmt;

use image::{imageops, ImageBuffer, RgbaImage};
use pgn_reader::Outcome;
use shakmaty;
use tiny_skia::{self, Pixmap};
use usvg::FitTo;

use super::error::DrawerError;
use super::svgs::SVGForest;
use super::utils::PieceInBoard;

/// All possible endings for a chess game
#[derive(Debug)]
pub enum TerminationReason {
    Checkmate { winner: shakmaty::Color },
    Stalemate,
    DrawAgreement,
    DrawByRepetition,
    Timeout { winner: shakmaty::Color },
    Resignation { winner: shakmaty::Color },
    InsufficientMaterial,
    DrawByTimeoutVsInsufficientMaterial,
}

impl TerminationReason {
    /// Create a TerminationReason from a pgn_reader Outcome. Requires a reason to
    /// decide between similar outcomes.
    pub fn from_outcome(outcome: Outcome, reason: Option<&str>) -> Self {
        match outcome {
            Outcome::Decisive { winner: w } => {
                let winner = shakmaty::Color::from_char(w.char()).unwrap();
                match reason {
                    None | Some("checkmate") => TerminationReason::Checkmate { winner },
                    Some("timeout") => TerminationReason::Timeout { winner },
                    Some("resignation") => TerminationReason::Resignation { winner },
                    Some(&_) => panic!("Unknown termination reason"),
                }
            }
            Outcome::Draw => match reason {
                Some("insufficient material") => TerminationReason::InsufficientMaterial,
                Some("timeout") => TerminationReason::DrawByTimeoutVsInsufficientMaterial,
                Some("stalemate") => TerminationReason::Stalemate,
                Some("repetition") => TerminationReason::DrawByRepetition,
                Some("agreement") | None => TerminationReason::DrawAgreement,
                Some(&_) => panic!("Unknown termination reason"),
            },
        }
    }

    pub fn is_draw(&self) -> bool {
        match self {
            TerminationReason::Stalemate
            | TerminationReason::DrawAgreement
            | TerminationReason::DrawByRepetition
            | TerminationReason::DrawByTimeoutVsInsufficientMaterial
            | TerminationReason::InsufficientMaterial => true,
            _ => false,
        }
    }
}

impl fmt::Display for TerminationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminationReason::Checkmate { winner: _ } => write!(f, "checkmate"),
            // Each draw variation should have it's own circle eventually
            TerminationReason::Stalemate
            | TerminationReason::DrawAgreement
            | TerminationReason::DrawByRepetition
            | TerminationReason::DrawByTimeoutVsInsufficientMaterial
            | TerminationReason::InsufficientMaterial => write!(f, "draw"),
            TerminationReason::Resignation { winner: _ } => write!(f, "resignation"),
            TerminationReason::Timeout { winner: _ } => write!(f, "timeout"),
        }
    }
}

/// Draws highlights to indicate the game's termination reason
pub struct TerminationDrawer {
    width: u32,
    height: u32,
    svgs: SVGForest,
}

impl TerminationDrawer {
    pub fn new(
        width: u32,
        height: u32,
        termination_path: &str,
        font_path: &str,
    ) -> Result<Self, DrawerError> {
        let svgs = SVGForest::new(font_path, None, Some(termination_path))?;

        Ok(TerminationDrawer {
            width,
            height,
            svgs,
        })
    }

    pub fn termination_circle_pixmap(
        &self,
        color: shakmaty::Color,
        reason: &TerminationReason,
    ) -> Result<Pixmap, DrawerError> {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();

        let rtree = self.svgs.termination_tree(reason, Some(color))?;

        let fit_to = FitTo::Height(self.height);
        resvg::render(&rtree, fit_to, pixmap.as_mut()).ok_or(DrawerError::SVGRenderError {
            svg: format!("{}", reason),
        })?;

        Ok(pixmap)
    }

    pub fn win_circle_pixmap(&self) -> Result<Pixmap, DrawerError> {
        let mut pixmap = Pixmap::new(self.width, self.height).unwrap();

        let rtree = self.svgs.termination_tree("win", None)?;

        let fit_to = FitTo::Height(self.height);
        resvg::render(&rtree, fit_to, pixmap.as_mut()).ok_or(DrawerError::SVGRenderError {
            svg: "win".to_string(),
        })?;

        Ok(pixmap)
    }

    pub fn termination_circle_image(
        &self,
        color: shakmaty::Color,
        reason: &TerminationReason,
    ) -> Result<RgbaImage, DrawerError> {
        let pixmap = self.termination_circle_pixmap(color, reason)?;

        ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).ok_or(
            DrawerError::ImageTooBig {
                image: format!("{}_{:?}.svg", reason, color),
            },
        )
    }

    pub fn win_circle_image(&self) -> Result<RgbaImage, DrawerError> {
        let pixmap = self.win_circle_pixmap()?;

        ImageBuffer::from_raw(pixmap.width(), pixmap.height(), pixmap.take()).ok_or(
            DrawerError::ImageTooBig {
                image: "win.svg".to_string(),
            },
        )
    }

    pub fn draw_termination_circles(
        &mut self,
        reason: TerminationReason,
        winner: PieceInBoard,
        loser: PieceInBoard,
        img: &mut RgbaImage,
    ) -> Result<(), DrawerError> {
        let (circle_winner, circle_loser) = if reason.is_draw() {
            let c1 = self.termination_circle_image(loser.color, &reason)?;
            let c2 = self.termination_circle_image(winner.color, &reason)?;
            (c1, c2)
        } else {
            let c1 = self.win_circle_image()?;
            let c2 = self.termination_circle_image(loser.color, &reason)?;
            (c1, c2)
        };

        let height = img.height();
        let width = img.width();

        let winner_x = (width / 8) * u32::from(winner.square.file());
        let winner_y = height - (width / 8) * (u32::from(winner.square.rank()) + 2);

        let loser_x = (width / 8) * u32::from(loser.square.file());
        let loser_y = height - (width / 8) * (u32::from(loser.square.rank()) + 2);

        imageops::overlay(img, &circle_winner, winner_x, winner_y);
        imageops::overlay(img, &circle_loser, loser_x, loser_y);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circle_pixmap_draw() {
        let drawer = TerminationDrawer::new(16, 16, "terminations", "roboto.ttf").unwrap();
        let circle = drawer
            .termination_circle_pixmap(shakmaty::Color::Black, &TerminationReason::DrawAgreement)
            .unwrap();

        assert_eq!(circle.width(), 16);
        assert_eq!(circle.height(), 16);
    }

    #[test]
    fn test_circle_pixmap_win() {
        let drawer = TerminationDrawer::new(16, 16, "terminations", "roboto.ttf").unwrap();
        let circle = drawer.win_circle_pixmap().unwrap();

        assert_eq!(circle.width(), 16);
        assert_eq!(circle.height(), 16);
    }
}
