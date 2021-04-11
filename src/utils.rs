use shakmaty::{File, Rank, Square};

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
