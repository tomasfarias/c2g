use c2g::{cli::Chess2Gif, error::C2GError};

fn main() -> Result<(), C2GError> {
    env_logger::init();

    let c2g = Chess2Gif::new();
    c2g.run()
}
