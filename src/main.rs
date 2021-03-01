use c2g::Chess2Gif;

fn main() {
    env_logger::init();

    let c2g = Chess2Gif::new();
    c2g.run();
}
