use std::fmt;
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
    title: Option<String>,
    elo: Option<u32>,
}

impl Default for Player {
    fn default() -> Self {
        Player {
            name: None,
            title: None,
            elo: None,
        }
    }
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let player_string = match &self.title {
            Some(s) => format!(
                "{} {}",
                s,
                self.name.as_ref().unwrap_or(&"Anonymous".to_string())
            ),
            None => format!("{}", self.name.as_ref().unwrap_or(&"Anonymous".to_string())),
        };
        match &self.elo {
            Some(n) => write!(f, "{} ({})", player_string, n),
            None => write!(f, "{}", player_string),
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

impl Players {
    /// Convenience method to check if player headers were found
    pub fn exist(&self) -> bool {
        log::debug!("White: {:?}, Black: {:?}", self.white, self.black);
        self.white.is_some() && self.black.is_some()
    }

    pub fn update_player_name(&mut self, color: shakmaty::Color, name: &str) {
        match color {
            shakmaty::Color::White => {
                match self.white.as_mut() {
                    Some(p) => (*p).name = Some(name.to_string()),
                    None => {}
                };
            }
            shakmaty::Color::Black => {
                match self.black.as_mut() {
                    Some(p) => (*p).name = Some(name.to_string()),
                    None => {}
                };
            }
        };
    }

    pub fn update_player_title(&mut self, color: shakmaty::Color, title: &str) {
        match color {
            shakmaty::Color::White => {
                match self.white.as_mut() {
                    Some(p) => (*p).title = Some(title.to_string()),
                    None => {}
                };
            }
            shakmaty::Color::Black => {
                match self.black.as_mut() {
                    Some(p) => (*p).title = Some(title.to_string()),
                    None => {}
                };
            }
        };
    }

    pub fn update_player_elo(&mut self, color: shakmaty::Color, elo: u32) {
        match color {
            shakmaty::Color::White => {
                match self.white.as_mut() {
                    Some(p) => (*p).elo = Some(elo),
                    None => {}
                };
            }
            shakmaty::Color::Black => {
                match self.black.as_mut() {
                    Some(p) => (*p).elo = Some(elo),
                    None => {}
                };
            }
        };
    }

    pub fn create_player(
        &mut self,
        color: shakmaty::Color,
        name: Option<String>,
        title: Option<String>,
        elo: Option<u32>,
    ) {
        match color {
            shakmaty::Color::White => {
                self.white = Some(Player {
                    name: name,
                    title: title,
                    elo: elo,
                })
            }
            shakmaty::Color::Black => {
                self.black = Some(Player {
                    name: name,
                    title: title,
                    elo: elo,
                })
            }
        };
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

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let millis = self.duration.as_millis();
        let mut tenth_secs = millis / 100;
        let mut secs = millis / 1000;
        let mut minutes = secs / 60;
        let hours = minutes / 60;

        tenth_secs -= secs * 10;
        secs -= minutes * 60;
        minutes -= hours * 60;

        write!(f, "{}:{:02}:{:02}.{:01}", hours, minutes, secs, tenth_secs,)
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
            let increment = self.increment.unwrap_or(0);
            log::debug!(
                "Turn clock: {:?}, previous: {:?}, increment: {:?}",
                turn_clock,
                prev_turn_clock,
                increment,
            );

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
    output_path: String,
    delay: Delay,
    player_bars: bool,
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
        player_bars: bool,
        board_size: u32,
        output_path: &str,
        delay: Delay,
        first_frame_delay: u16,
        last_frame_delay: u16,
        dark: [u8; 4],
        light: [u8; 4],
    ) -> Result<Self, GifferError> {
        let drawer = BoardDrawer::new(pieces_path, flip, font_path, board_size as u32, dark, light)
            .map_err(|source| GifferError::DrawerError { source: source })?;

        Ok(PGNGiffer {
            drawer: drawer,
            position: Chess::default(),
            output_path: output_path.to_owned(),
            player_bars: player_bars,
            delay: delay,
            first_frame_delay: first_frame_delay,
            last_frame_delay: last_frame_delay,
            // frames: Vec::new(),
            players: Players::default(),
            boards: Vec::new(),
            clocks: GameClocks::default(),
        })
    }

    pub fn build_encoder(
        &mut self,
        width: u16,
        height: u16,
    ) -> Result<Encoder<BufWriter<fs::File>>, GifferError> {
        let file = fs::File::create(&self.output_path)
            .map_err(|source| GifferError::CreateOutput { source })?;
        let buffer = BufWriter::with_capacity(1000, file);

        let mut encoder = Encoder::new(buffer, width, height, &[])
            .map_err(|source| GifferError::InitializeEncoder { source })?;
        encoder
            .set_repeat(Repeat::Infinite)
            .map_err(|source| GifferError::InitializeEncoder { source })?;

        Ok(encoder)
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
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true) // stay in the mainline
    }

    /// Parses PGN headers to extract player information
    fn header(&mut self, key: &[u8], value: RawHeader<'_>) {
        match std::str::from_utf8(key) {
            Ok("White") => {
                let name = &value.decode_utf8_lossy().to_string();

                log::debug!("White: {}", name);
                match self.players.white {
                    Some(_) => self
                        .players
                        .update_player_name(shakmaty::Color::White, name),
                    None => self.players.create_player(
                        shakmaty::Color::White,
                        Some(name.to_string()),
                        None,
                        None,
                    ),
                };
            }
            Ok("Black") => {
                let name = &value.decode_utf8_lossy().to_string();

                log::debug!("Black: {}", name);
                match self.players.black {
                    Some(_) => self
                        .players
                        .update_player_name(shakmaty::Color::Black, name),
                    None => self.players.create_player(
                        shakmaty::Color::Black,
                        Some(name.to_string()),
                        None,
                        None,
                    ),
                };
            }
            Ok("WhiteElo") => {
                let elo = &value
                    .decode_utf8_lossy()
                    .to_string()
                    .parse::<u32>()
                    .expect("WhiteElo could not be parsed");

                match self.players.white {
                    Some(_) => self.players.update_player_elo(shakmaty::Color::White, *elo),
                    None => {
                        self.players
                            .create_player(shakmaty::Color::White, None, None, Some(*elo))
                    }
                };
            }
            Ok("BlackElo") => {
                let elo = &value
                    .decode_utf8_lossy()
                    .to_string()
                    .parse::<u32>()
                    .expect("BlackElo could not be parsed");

                log::debug!("BlackElo: {}", elo);
                match self.players.black {
                    Some(_) => self.players.update_player_elo(shakmaty::Color::Black, *elo),
                    None => {
                        self.players
                            .create_player(shakmaty::Color::Black, None, None, Some(*elo))
                    }
                };
            }
            Ok("TimeControl") => {
                let inc = &value
                    .decode_utf8_lossy()
                    .to_string()
                    .split("+")
                    .collect::<Vec<&str>>()
                    .get(1)
                    .map_or_else(|| None, |s| Some(s.parse::<u16>().unwrap() * 1000));
                self.clocks.increment = *inc;
            }
            _ => (),
        }
    }

    /// Check if we managed to parse players and adjust the initial board
    fn end_headers(&mut self) -> Skip {
        log::debug!("Players: {}", self.players.exist());
        if self.players.exist() && self.player_bars == true {
            log::debug!("Adding player bars to first board");
            let board = self.boards.pop().expect("Initial board should exist");
            let mut new_board = self.drawer.add_player_bar_space(board);
            log::debug!(
                "New board width: {}, height: {}",
                new_board.width(),
                new_board.height()
            );

            let white_player = self.players.white.as_ref().unwrap().to_string();
            let black_player = self.players.black.as_ref().unwrap().to_string();
            self.drawer
                .draw_player_bars(&white_player, &black_player, &mut new_board)
                .expect("Failed to draw player bars");

            self.boards.push(new_board);
        }

        Skip(false)
    }

    /// Calls BoardDrawer.draw_move with every move and stores the resulting board
    fn san(&mut self, san_plus: SanPlus) {
        if let Ok(m) = san_plus.san.to_move(&self.position) {
            let mut board = self.drawer.image_buffer();
            self.drawer
                .draw_move(&m, self.position.turn(), &mut board)
                .expect(&format!("Failed to draw move: {}", m));

            if self.players.exist() && self.player_bars == true {
                log::debug!("Adding player bars");
                let mut new_board = self.drawer.add_player_bar_space(board);
                log::debug!(
                    "New board width: {}, height: {}",
                    new_board.width(),
                    new_board.height()
                );
                let white_player = self.players.white.as_ref().unwrap().to_string();
                let black_player = self.players.black.as_ref().unwrap().to_string();
                self.drawer
                    .draw_player_bars(&white_player, &black_player, &mut new_board)
                    .expect("Failed to draw player bars");

                self.boards.push(new_board);
            } else {
                self.boards.push(board);
            }

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
        let (height, width) = if self.players.exist() && self.player_bars == true {
            let bar_size = self.drawer.square_size() * 2;
            (
                (self.drawer.size() + bar_size) as u16,
                self.drawer.size() as u16,
            )
        } else {
            (self.drawer.size() as u16, self.drawer.size() as u16)
        };
        log::debug!(
            "Size: {}, width: {}, height: {}",
            self.drawer.size(),
            width,
            height
        );

        let mut encoder = self.build_encoder(width, height)?;

        for (n, mut b) in self.boards.drain(..).enumerate() {
            log::debug!("Building frame for board number: {}", n);
            log::debug!("Board width: {}, height: {}", b.width(), b.height());

            let turn = if n == 0 { n } else { (n - 1) / 2 };

            let white_clock = self.clocks.white.get(turn);
            let mut black_clock = self.clocks.black.get(turn);

            if turn > 0 && n % 2 != 0 {
                black_clock = self.clocks.black.get(turn - 1);
            }

            if white_clock.is_some()
                && black_clock.is_some()
                && self.players.exist()
                && self.player_bars == true
            {
                self.drawer.draw_player_clocks(
                    &white_clock.unwrap().to_string(),
                    &black_clock.unwrap().to_string(),
                    &mut b,
                )?;
            }

            let mut frame = Frame::from_rgba_speed(width, height, &mut b.into_raw(), 10);

            log::debug!("Calculating delay for turn: {}", turn);
            if n == (total_frames - 1) {
                log::debug!("LAST FRAME");
                frame.delay = self.last_frame_delay / 10;
            } else if n == 0 || n == 1 {
                frame.delay = self.first_frame_delay / 10;
            } else {
                match self.delay {
                    Delay::Duration(d) => {
                        frame.delay = d / 10;
                    }
                    Delay::Real => {
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
            encoder
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

    #[test]
    fn test_display_clocks() {
        let clock = Clock::from_time_str("0:01:00");
        assert_eq!(format!("{}", clock), "0:01:00.0");

        let clock = Clock::from_millis(60000 as u32);
        assert_eq!(format!("{}", clock), "0:01:00.0");

        let clock = Clock::from_millis(55100 as u32);
        assert_eq!(format!("{}", clock), "0:00:55.1");
    }

    #[test]
    fn test_clocks_as_millis() {
        let clock = Clock::from_time_str("0:01:05.1");
        assert_eq!(clock.as_millis(), 65100);
    }
}
