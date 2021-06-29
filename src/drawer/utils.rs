use shakmaty::{self, File, Rank, Role, Square};

/// A piece in a chess board
#[derive(Debug)]
pub struct PieceInBoard {
    pub square: Square,
    pub role: Role,
    pub color: shakmaty::Color,
}

impl PieceInBoard {
    /// Create a new King in the board
    pub fn new_king(square: shakmaty::Square, color: shakmaty::Color) -> Self {
        PieceInBoard {
            square,
            color,
            role: Role::King,
        }
    }

    /// Flip at the h1-a8 diagonal in place
    pub fn flip_anti_diagonal(&mut self) {
        self.square = self.square.flip_anti_diagonal();
    }

    /// Flip vertically and horizontally
    pub fn flip_both(&mut self) {
        self.square = self.square.flip_vertical().flip_horizontal();
    }
}

/// Check if a square contains a coordinate. Coordindates are found in the A file
/// and first rank
pub fn has_coordinate(s: &Square, flip: bool) -> bool {
    if (s.rank() == Rank::First || s.file() == File::A) && flip == false {
        true
    } else if (s.rank() == Rank::Eighth || s.file() == File::H) && flip == true {
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_in_board_new_king() {
        let piece = PieceInBoard::new_king(Square::new(0), shakmaty::Color::Black);
        assert_eq!(piece.role, Role::King);
    }

    #[test]
    fn test_has_coordinate() {
        let square = Square::new(0); // A1
        assert!(has_coordinate(&square, false));

        let square = Square::new(9); // B2
        assert!(!has_coordinate(&square, false));

        let square = Square::new(56); // A8
        assert!(has_coordinate(&square, false));
    }

    #[test]
    fn test_has_coordinate_flip() {
        let square = Square::new(0); // A1
        assert!(!has_coordinate(&square, true));

        let square = Square::new(7); // H1
        assert!(has_coordinate(&square, true));

        let square = Square::new(63); // H8
        assert!(has_coordinate(&square, true));
    }
}
