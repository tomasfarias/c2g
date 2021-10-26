use std::fmt;
use std::fs;
use std::io::BufWriter;
use std::ops::Sub;
use std::time::Duration;

use gif::{self, Encoder, Frame, Repeat};
use image::RgbaImage;
use pgn_reader::{Outcome, RawComment, RawHeader, SanPlus, Skip, Visitor};
use regex::Regex;
use shakmaty::{Chess, Color, Position, Role, Setup, Square};
use thiserror::Error;

use crate::config::Config;
use crate::delay::Delay;
use crate::drawer::{
    BoardDrawer, DrawerError, PieceInBoard, SVGFontConfig, SVGForest, TerminationDrawer,
    TerminationReason,
};

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
            shakmaty::Color::White => self.white = Some(Player { name, title, elo }),
            shakmaty::Color::Black => self.black = Some(Player { name, title, elo }),
        };
    }
}

/// A player's clock in a chess game
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
    /// Create a clock from milliseconds
    fn from_millis<S>(millis: S) -> Self
    where
        S: Into<u64>,
    {
        Clock {
            duration: Duration::from_millis(millis.into()),
        }
    }

    /// Create a new clock with the milliseconds added
    fn add_millis<S>(&self, millis: S) -> Self
    where
        S: Into<u64>,
    {
        Clock {
            duration: self.duration + Duration::from_millis(millis.into()),
        }
    }

    /// Construct a clock from a time string
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
    /// Calculate the delay between a turn and the previous one
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

pub struct PGNGiffer {
    drawer: BoardDrawer,
    termination_drawer: TerminationDrawer,
    position: Chess,
    config: Config,
    termination: Option<String>,
    players: Players,
    boards: Vec<RgbaImage>,
    clocks: GameClocks,
    to_clear: Vec<(Square, Role, Color)>,
    svgs: SVGForest,
}

impl PGNGiffer {
    pub fn new(config: Config) -> Result<Self, GifferError> {
        let drawer = BoardDrawer::new(
            config.flip,
            config.size,
            config.colors.dark.clone(),
            config.colors.light.clone(),
        )
        .map_err(|source| GifferError::DrawerError { source })?;
        let circle_size = config.size / 8 / 3;
        let termination_drawer = TerminationDrawer::new(circle_size as u32, circle_size as u32)
            .map_err(|source| GifferError::DrawerError { source })?;

        let svg_font_config = SVGFontConfig {
            font_path: config.font_path.clone(),
            font_family: Some(config.font_family.clone()),
            ..Default::default()
        };

        let svgs = SVGForest::new(
            svg_font_config,
            &config.svgs_path,
            &config.pieces_family,
            "terminations",
        )?;

        Ok(PGNGiffer {
            drawer,
            termination_drawer,
            position: Chess::default(),
            config: config,
            termination: None,
            players: Players::default(),
            boards: Vec::new(),
            clocks: GameClocks::default(),
            to_clear: Vec::new(),
            svgs,
        })
    }

    pub fn build_encoder(
        &mut self,
        width: u16,
        height: u16,
    ) -> Result<Encoder<BufWriter<fs::File>>, GifferError> {
        let file = fs::File::create(&self.config.output_path)
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
            .draw_position_from_empty(pieces, &self.svgs)
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
            Ok("Termination") => {
                self.termination = Some(value.decode_utf8_lossy().to_string());
            }
            _ => (),
        }
    }

    /// Check if we managed to parse players and adjust the initial board
    fn end_headers(&mut self) -> Skip {
        log::debug!("Players: {}", self.players.exist());
        if self.players.exist() && self.config.style_components.player_bars() == true {
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
                .draw_player_bars(&white_player, &black_player, &mut new_board, &self.svgs)
                .expect("Failed to draw player bars");

            self.boards.push(new_board);
        }

        Skip(false)
    }

    /// Calls BoardDrawer.draw_move with every move and stores the resulting board
    fn san(&mut self, san_plus: SanPlus) {
        if let Ok(m) = san_plus.san.to_move(&self.position) {
            let mut board = self.drawer.image_buffer();
            for (square, role, color) in self.to_clear.drain(..) {
                self.drawer
                    .draw_piece(
                        &square, &role, color, false, &mut board, None, &self.svgs, false,
                    )
                    .expect(&format!("Failed to clear piece"));
            }

            self.drawer
                .draw_move(&m, self.position.turn(), &mut board, &self.svgs)
                .expect(&format!("Failed to draw move: {}", m));

            log::debug!("Pushing board for move {:?}", m);
            self.position.play_unchecked(&m);

            if self.position.is_check() {
                let color = self.position.turn();
                let king_square = self
                    .position
                    .board()
                    .king_of(color)
                    .expect("King square should exist");
                let king_piece = PieceInBoard::new_king(king_square, color);
                self.drawer
                    .draw_checked_king(king_piece, &mut board, &self.svgs)
                    .expect(&format!("Failed to draw checked king: {}", king_square));

                let to_be_cleared = (king_square, Role::King, color);
                self.to_clear.push(to_be_cleared);
            };

            if self.players.exist() && self.config.style_components.player_bars() == true {
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
                    .draw_player_bars(&white_player, &black_player, &mut new_board, &self.svgs)
                    .expect("Failed to draw player bars");

                self.boards.push(new_board);
            } else {
                self.boards.push(board);
            }
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

    /// Check the outcome of the game to draw the appropiate termination circle
    fn outcome(&mut self, outcome: Option<Outcome>) {
        if !self.config.style_components.terminations() {
            return;
        }

        let mut latest_board = self.boards.pop().expect("No boards drawn!");
        match outcome {
            Some(o) => {
                let reason = if self.position.is_checkmate() {
                    "checkmate"
                } else if self.position.is_stalemate() {
                    "stalemate"
                } else if self.position.is_insufficient_material() {
                    "insufficient material"
                } else {
                    match &self.termination {
                        Some(s) => {
                            if s.contains("resignation") {
                                "resignation"
                            } else if s.contains("agreement") {
                                "agreement"
                            } else if s.contains("repetition") {
                                "repetition"
                            } else {
                                "timeout"
                            }
                        }
                        None => match o {
                            Outcome::Draw => "agreement",
                            Outcome::Decisive { winner: _ } => "resignation",
                        },
                    }
                };
                let termination_reason = TerminationReason::from_outcome(o, Some(reason));

                let (mut winner_king, mut loser_king) = match o {
                    Outcome::Draw => {
                        // Doesn't really matter which king is which, since in draw there is no
                        // winner or loser.
                        let square1 = self
                            .position
                            .board()
                            .king_of(shakmaty::Color::White)
                            .expect("King doesn't exist");
                        let square2 = self
                            .position
                            .board()
                            .king_of(shakmaty::Color::Black)
                            .expect("King doesn't exist");

                        let king1 = PieceInBoard::new_king(square1, shakmaty::Color::White);
                        let king2 = PieceInBoard::new_king(square2, shakmaty::Color::Black);

                        (king1, king2)
                    }
                    Outcome::Decisive { winner: w } => {
                        let winner = self
                            .position
                            .board()
                            .king_of(w)
                            .expect("King doesn't exist");
                        let loser_color = match w {
                            shakmaty::Color::Black => shakmaty::Color::White,
                            shakmaty::Color::White => shakmaty::Color::Black,
                        };
                        let loser = self
                            .position
                            .board()
                            .king_of(loser_color)
                            .expect("King doesn't exist");

                        let winner_king = PieceInBoard::new_king(winner, w);
                        let loser_king = PieceInBoard::new_king(loser, loser_color);

                        (winner_king, loser_king)
                    }
                };

                if self.drawer.flip() {
                    // This should be moved to the drawer
                    winner_king.flip_both();
                    loser_king.flip_both();
                }

                log::debug!(
                    "Drawing termination: {:?}, {:?}, {:?}, {:?}",
                    o,
                    termination_reason,
                    winner_king,
                    loser_king
                );
                self.termination_drawer
                    .draw_termination_circles(
                        termination_reason,
                        winner_king,
                        loser_king,
                        &mut latest_board,
                        &self.svgs,
                    )
                    .expect("Failed to draw termination circle");
                self.boards.push(latest_board);
            }
            // If the game didn't end, we don't do anything
            None => (),
        };
    }

    /// Iterates over boards collected for every move to encode GIF frames for each move.
    /// Assigns delays to each frame based on self.config.delay and self.last_frame_multiplier.
    fn end_game(&mut self) -> Self::Result {
        let total_frames = self.boards.len();
        let (height, width) =
            if self.players.exist() && self.config.style_components.player_bars() == true {
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
                && self.config.style_components.player_bars() == true
            {
                self.drawer.draw_player_clocks(
                    &white_clock.unwrap().to_string(),
                    &black_clock.unwrap().to_string(),
                    &mut b,
                    &self.svgs,
                )?;
            }

            let mut frame = Frame::from_rgba_speed(width, height, &mut b.into_raw(), 10);

            log::debug!("Calculating delay for turn: {}", turn);
            if n == (total_frames - 1) {
                log::debug!("LAST FRAME");
                frame.delay = self
                    .config
                    .delays
                    .last_frame_delay()
                    .expect("Last frame delay not defined")
                    / 10;
            } else if n == 0 || n == 1 {
                frame.delay = self
                    .config
                    .delays
                    .first_frame_delay()
                    .expect("First frame delay not defined")
                    / 10;
            } else {
                match self.config.delays.frame {
                    Delay::Duration(d) => {
                        frame.delay = d / 10;
                    }
                    Delay::Real => {
                        if n & 1 != 0 {
                            frame.delay = match self.clocks.turn_delay(turn, Color::Black) {
                                Some(d) => d / 10,
                                // First move, no previous clock
                                None => {
                                    self.config
                                        .delays
                                        .first_frame_delay()
                                        .expect("First frame delay not defined")
                                        / 10
                                }
                            };
                        } else {
                            frame.delay = match self.clocks.turn_delay(turn, Color::White) {
                                Some(d) => d / 10,
                                // First move, no previous clock
                                None => {
                                    self.config
                                        .delays
                                        .first_frame_delay()
                                        .expect("First frame delay not defined")
                                        / 10
                                }
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
