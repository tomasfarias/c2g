/// Test the examples provided with C2G.
use c2g::{app::Chess2Gif, config};
use std::fs;

#[test]
fn test_example() {
    let contents = fs::read_to_string("example/example.pgn").expect("Failed to read example PGN");
    let config = config::Config {
        output: config::Output::Buffer,
        ..config::Config::default()
    };
    let app = Chess2Gif::new(contents, config).expect("Failed to initialize Chess2Gif");

    let result = app.run();

    assert!(result.is_ok());

    let maybe_bytes = result.expect("Already checked this is Ok");
    assert!(maybe_bytes.is_some());

    let bytes = maybe_bytes.expect("Already checked this is Ok");
    assert!(bytes.len() > 0);
}

#[test]
fn test_example_bullet() {
    let contents =
        fs::read_to_string("example/example_bullet.pgn").expect("Failed to read example PGN");
    let config = config::Config {
        output: config::Output::Buffer,
        ..config::Config::default()
    };
    let app = Chess2Gif::new(contents, config).expect("Failed to initialize Chess2Gif");

    let result = app.run();

    assert!(result.is_ok());

    let maybe_bytes = result.expect("Already checked this is Ok");
    assert!(maybe_bytes.is_some());

    let bytes = maybe_bytes.expect("Already checked this is Ok");
    assert!(bytes.len() > 0);
}
