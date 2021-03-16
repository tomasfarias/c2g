use shakmaty::{File, Rank, Square};

pub fn has_coordinate(s: &Square) -> bool {
    if s.rank() == Rank::First || s.file() == File::A {
        true
    } else {
        false
    }
}
