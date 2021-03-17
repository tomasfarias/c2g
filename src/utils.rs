use shakmaty::{File, Rank, Square};

pub fn has_coordinate(s: &Square) -> bool {
    if s.rank() == Rank::First || s.file() == File::A {
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
        assert!(has_coordinate(&square));

        let square = Square::new(9); // B2
        assert!(!has_coordinate(&square));

        let square = Square::new(56); // A8
        assert!(has_coordinate(&square));
    }
}
