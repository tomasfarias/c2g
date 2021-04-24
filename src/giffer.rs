use std::fs;
use std::io::BufWriter;
use std::ops::Sub;
use std::time::Duration;

use gif::{self, Encoder, Frame, Repeat};
use image::RgbaImage;
use log;
use pgn_reader::{RawComment, RawHeader, SanPlus, Skip, Visitor};
use regex::Regex;
use shakmaty::{Chess, Color, Position, Setup};
use thiserror::Error;

use crate::drawer::{BoardDrawer, DrawerError};

/// A player during a GIF frame. Used to add player bars at the top and the bottom of the GIF.
#[derive(Clone, Debug)]
pub struct Player {
    name: Option<String>,
}

impl Player {
    fn new(name: &str) -> Self {
        Player {
            name: Some(name.to_string()),
        }
    }
}

/// Both players during a frame or turn in the game.
#[derive(Clone, Debug)]
pub struct Players {
    white: Option<Player>,
    black: Option<Player>,
}

impl Default for Players {
    fn default() -> Self {
        Players {
            white: None,
            black: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Clock {
    duration: Duration,
}

impl Default for Clock {
    fn default() -> Self {
        Clock {
            duration: Duration::default(),
        }
    }
}

impl<'a, 'b> Sub<&'b Clock> for &'a Clock {
    type Output = Clock;

    fn sub(self, other: &'b Clock) -> Self::Output {
        Clock {
            duration: self.duration - other.duration,
        }
    }
}

impl Clock {
    fn from_millis<S>(millis: S) -> Self
    where
        S: Into<u64>,
    {
        Clock {
            duration: Duration::from_millis(millis.into()),
        }
    }

    fn add_millis<S>(&self, millis: S) -> Self
    where
        S: Into<u64>,
    {
        Clock {
            duration: self.duration + Duration::from_millis(millis.into()),
        }
    }

    fn from_time_str(s: &str) -> Self {
        let splitted: Vec<&str> = s.split(":").collect();
        let hours_ms = splitted[0].parse::<u64>().unwrap() * 60 * 60 * 1000;
        let minutes_ms = splitted[1].parse::<u64>().unwrap() * 60 * 1000;
        let milliseconds = splitted[2].parse::<f64>().unwrap() * 1000.0;
        let total_ms = milliseconds as u64 + minutes_ms + hours_ms;

        Clock::from_millis(total_ms)
    }

    fn as_millis(&self) -> u128 {
        self.duration.as_millis()
    }
}

/// Clocks in a chess move. One for each player.
#[derive(Clone, Debug)]
pub struct GameClocks {
    white: Vec<Clock>,
    black: Vec<Clock>,
    increment: Option<u16>,
}

impl Default for GameClocks {
    fn default() -> Self {
        GameClocks {
            white: Vec::new(),
            black: Vec::new(),
            increment: None,
        }
    }
}

impl GameClocks {
    fn turn_delay<U>(&self, turn: U, color: Color) -> Option<u16>
    where
        U: Into<usize>,
    {
        let clocks = match color {
            Color::White => self.white(),
            Color::Black => self.black(),
        };
        let turn = turn.into();
        if turn <= 0 {
            log::debug!("FIRST TURN");
            return None;
        }

        let turn_clock = clocks.get(turn);
        let prev_turn_clock = clocks.get(turn - 1);

        if turn_clock.is_none() || prev_turn_clock.is_none() {
            None
        } else {
            log::debug!(
                "Turn clock: {:?}, previous: {:?}",
                turn_clock,
                prev_turn_clock
            );
            let increment = self.increment.unwrap_or(0);
            let prev = prev_turn_clock.unwrap().add_millis(increment);
            let curr = turn_clock.unwrap();

            let diff = &prev - curr;
            Some(diff.as_millis() as u16)
        }
    }

    fn append(&mut self, clock: Clock, color: Color) {
        let clocks = match color {
            Color::White => self.white_mut(),
            Color::Black => self.black_mut(),
        };

        clocks.push(clock);
    }

    fn white_mut(&mut self) -> &mut Vec<Clock> {
        &mut self.white
    }

    fn black_mut(&mut self) -> &mut Vec<Clock> {
        &mut self.black
    }

    fn white(&self) -> &Vec<Clock> {
        &self.white
    }

    fn black(&self) -> &Vec<Clock> {
        &self.black
    }
}

/// Represents the delays between GIF frames, which can be either a duration in ms, or real time given by %clk comments in PGN headers.
pub enum Delay {
    Duration(u16),
    Real,
}

pub struct PGNGiffer {
    drawer: BoardDrawer,
    position: Chess,
    encoder: Encoder<BufWriter<fs::File>>,
    delay: Delay,
    // frames: Vec<Frame<'a>>,
    last_frame_delay: u16,
    first_frame_delay: u16,
    players: Players,
    boards: Vec<RgbaImage>,
    clocks: GameClocks,
}

#[derive(Error, Debug)]
pub enum GifferError {
    #[error(transparent)]
    CreateOutput {
        #[from]
        source: std::io::Error,
    },
    #[error("A GIF encoder could not be initialized")]
    InitializeEncoder { source: gif::EncodingError },
    #[error("A GIF frame could not be encoded")]
    FrameEncoding { source: gif::EncodingError },
    #[error(transparent)]
    DrawerError {
        #[from]
        source: DrawerError,
    },
}

impl PGNGiffer {
    pub fn new(
        pieces_path: &str,
        font_path: &str,
        flip: bool,
        board_size: u32,
        output_path: &str,
        delay: Delay,
        first_frame_delay: u16,
        last_frame_delay: u16,
        dark: [u8; 4],
        light: [u8; 4],
    ) -> Result<Self, GifferError> {
        let file =
            fs::File::create(output_path).map_err(|source| GifferError::CreateOutput { source })?;
        let buffer = BufWriter::with_capacity(1000, file);

        let drawer = BoardDrawer::new(pieces_path, flip, font_path, board_size as u32, dark, light)
            .map_err(|source| GifferError::DrawerError { source: source })?;

        let mut encoder = Encoder::new(buffer, drawer.size() as u16, drawer.size() as u16, &[])
            .map_err(|source| GifferError::InitializeEncoder { source })?;
        encoder
            .set_repeat(Repeat::Infinite)
            .map_err(|source| GifferError::InitializeEncoder { source })?;

        Ok(PGNGiffer {
            drawer: drawer,
            position: Chess::default(),
            encoder: encoder,
            delay: delay,
            first_frame_delay: first_frame_delay,
            last_frame_delay: last_frame_delay,
            // frames: Vec::new(),
            players: Players::default(),
            boards: Vec::new(),
            clocks: GameClocks::default(),
        })
    }
}

impl Visitor for PGNGiffer {
    type Result = Result<(), GifferError>;

    fn begin_game(&mut self) {
        log::info!("Rendering initial board");
        let pieces = self.position.board().pieces();
        let board = self
            .drawer
            .draw_position_from_empty(pieces)
            .expect(&format!(
                "Failed to draw initial position: {}",
                self.position.board()
            ));
        self.boards.push(board);
        // let mut frame = Frame::from_rgba_speed(
        //     self.drawer.size() as u16,
        //     self.drawer.size() as u16,
        //     &mut board.into_raw(),
        //     30,
        // );
        // frame.delay = self.delay;
        // self.frames.push(frame);
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true) // stay in the mainline
    }

    /// Parses PGN headers to extract player information
    fn header(&mut self, key: &[u8], value: RawHeader<'_>) {
        match std::str::from_utf8(key) {
            Ok("White") => {
                self.players.white = Some(Player::new(&value.decode_utf8_lossy().to_string()));
            }
            Ok("Black") => {
                self.players.black = Some(Player::new(&value.decode_utf8_lossy().to_string()));
            }
            Ok("TimeControl") => {
                let inc = &value
                    .decode_utf8_lossy()
                    .to_string()
                    .split(":")
                    .collect::<Vec<&str>>()
                    .get(1)
                    .map_or_else(|| None, |s| Some(s.parse::<u16>().unwrap() * 1000));
                self.clocks.increment = *inc;
            }
            _ => (),
        }
    }

    /// Calls BoardDrawer.draw_move with every move and stores the resulting board
    fn san(&mut self, san_plus: SanPlus) {
        if let Ok(m) = san_plus.san.to_move(&self.position) {
            let mut board = self.drawer.image_buffer();
            self.drawer
                .draw_move(&m, self.position.turn(), &mut board)
                .expect(&format!("Failed to draw move: {}", m));

            // let mut frame = Frame::from_rgba_speed(
            //     self.drawer.size() as u16,
            //     self.drawer.size() as u16,
            //     &mut board.into_raw(),
            //     10,
            // );
            // frame.delay = self.delay;
            // self.frames.push(frame);
            self.boards.push(board);
            log::debug!("Pushing board for move {:?}", m);
            self.position.play_unchecked(&m);
        }
    }

    /// Parses comments to extract %clk (clock) comments
    fn comment(&mut self, comment: RawComment<'_>) {
        match std::str::from_utf8(comment.as_bytes()) {
            Ok(s) => {
                // Capture clock comments with regexp, assuming
                // no other time-like comment appears
                let re = Regex::new(r"\d{1,2}:\d{2}:(\d{2}.\d{1}|\d{2})").unwrap();
                if let Some(m) = re.find(s) {
                    log::debug!("Found clock time: {}", m.as_str());
                    let clock = Clock::from_time_str(m.as_str());
                    log::debug!("Appending clock: {:?}", clock);
                    match self.position.turn() {
                        // This represents the player that moves next, we need to
                        // set the clock of the player that moved last
                        Color::Black => {
                            self.clocks.append(clock, Color::White);
                        }
                        Color::White => {
                            self.clocks.append(clock, Color::Black);
                        }
                    }
                }
            }
            Err(_) => (),
        }
    }

    /// Iterates over boards collected for every move to encode GIF frames for each move.
    /// Assigns delays to each frame based on self.delay and self.last_frame_multiplier.
    fn end_game(&mut self) -> Self::Result {
        let total_frames = self.boards.len();
        for (n, b) in self.boards.drain(..).enumerate() {
            log::debug!("Building frame for board number: {}", n);
            let mut frame = Frame::from_rgba_speed(
                self.drawer.size() as u16,
                self.drawer.size() as u16,
                &mut b.into_raw(),
                10,
            );

            if n == (total_frames - 1) {
                log::debug!("LAST FRAME");
                frame.delay = self.last_frame_delay / 10;
            } else if n == 0 || n == 1 {
                frame.delay = self.first_frame_delay / 10;
            } else {
                match self.delay {
                    Delay::Duration(d) => {
                        frame.delay = d;
                    }
                    Delay::Real => {
                        let turn = n / 2;
                        log::debug!("Calculating delay for turn: {}", turn);
                        if n & 1 != 0 {
                            frame.delay = match self.clocks.turn_delay(turn, Color::Black) {
                                Some(d) => d / 10,
                                None => self.first_frame_delay / 10, // First move, no previous clock
                            };
                        } else {
                            frame.delay = match self.clocks.turn_delay(turn, Color::White) {
                                Some(d) => d / 10,
                                None => self.first_frame_delay / 10, // First move, no previous clock
                            };
                        }
                    }
                }
            }
            log::debug!("Frame delay set to: {}", frame.delay);
            log::debug!("Encoding frame for board number: {}", n);
            self.encoder
                .write_frame(&frame)
                .map_err(|source| GifferError::FrameEncoding { source })?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_from_time_str() {
        let clock = Clock::from_time_str("1:10:45.1");
        assert_eq!(clock.duration, Duration::from_millis(4245100));

        let clock = Clock::from_time_str("2:52:01");
        assert_eq!(clock.duration, Duration::from_millis(10321000));
    }

    #[test]
    fn test_clock_substract_ref() {
        let clock_1 = Clock::from_time_str("1:10:45.1");
        let clock_2 = Clock::from_time_str("1:00:00");
        let result = &clock_1 - &clock_2;
        assert_eq!(result.duration, Duration::from_millis(645100));
    }

    #[test]
    fn test_game_clocks_turn_delay() {
        let white_clocks = vec![
            Clock::from_time_str("0:01:00"),
            Clock::from_time_str("0:00:59.1"),
            Clock::from_time_str("0:00:55.3"),
        ];
        let black_clocks = vec![
            Clock::from_time_str("0:01:00"),
            Clock::from_time_str("0:00:58.5"),
            Clock::from_time_str("0:00:52.2"),
        ];
        let game_clocks = GameClocks {
            white: white_clocks,
            black: black_clocks,
            increment: None,
        };
        let turn: usize = 0;

        assert_eq!(game_clocks.turn_delay(turn, Color::Black), None);
        assert_eq!(game_clocks.turn_delay(turn, Color::White), None);

        assert_eq!(game_clocks.turn_delay(turn + 1, Color::Black), Some(1500));
        assert_eq!(game_clocks.turn_delay(turn + 1, Color::White), Some(900));

        assert_eq!(game_clocks.turn_delay(turn + 2, Color::Black), Some(6300));
        assert_eq!(game_clocks.turn_delay(turn + 2, Color::White), Some(3800));
    }

    #[test]
    fn test_game_clocks_turn_delay_with_increment() {
        let white_clocks = vec![
            Clock::from_time_str("0:01:00"),
            Clock::from_time_str("0:01:01.1"),
            Clock::from_time_str("0:00:57.3"),
        ];
        let black_clocks = vec![
            Clock::from_time_str("0:01:00"),
            Clock::from_time_str("0:01:02.5"),
            Clock::from_time_str("0:01:05.2"),
        ];
        let game_clocks = GameClocks {
            white: white_clocks,
            black: black_clocks,
            increment: Some(3000),
        };
        let turn: usize = 0;

        assert_eq!(game_clocks.turn_delay(turn, Color::Black), None);
        assert_eq!(game_clocks.turn_delay(turn, Color::White), None);

        assert_eq!(game_clocks.turn_delay(turn + 1, Color::Black), Some(500));
        assert_eq!(game_clocks.turn_delay(turn + 1, Color::White), Some(1900));

        assert_eq!(game_clocks.turn_delay(turn + 2, Color::Black), Some(300));
        assert_eq!(game_clocks.turn_delay(turn + 2, Color::White), Some(6800));
    }
}
