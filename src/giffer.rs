use std::fs;
use std::io::BufWriter;

use gif::{Encoder, Frame, Repeat};
use log;
use pgn_reader::{SanPlus, Skip, Visitor};
use shakmaty::{Chess, Position, Setup};

use crate::drawer::BoardDrawer;

pub struct PGNGiffer<'a> {
    drawer: BoardDrawer,
    position: Chess,
    encoder: Encoder<BufWriter<fs::File>>,
    delay: u16,
    frames: Vec<Frame<'a>>,
}

impl<'a> PGNGiffer<'a> {
    pub fn new(
        image_path: &str,
        font_path: &str,
        board_size: u32,
        output_path: &str,
        ms_delay: u16,
        dark: [u8; 4],
        light: [u8; 4],
    ) -> Self {
        let file = fs::File::create(output_path).unwrap();
        let buffer = BufWriter::with_capacity(1000, file);

        let drawer = BoardDrawer::new(
            image_path.to_owned(),
            Some(font_path.to_owned()),
            board_size as u32,
            dark,
            light,
        );

        let mut encoder =
            Encoder::new(buffer, drawer.size() as u16, drawer.size() as u16, &[]).unwrap();
        encoder.set_repeat(Repeat::Infinite).unwrap();

        PGNGiffer {
            drawer: drawer,
            position: Chess::default(),
            encoder: encoder,
            delay: ms_delay,
            frames: Vec::new(),
        }
    }
}

impl<'a> Visitor for PGNGiffer<'a> {
    type Result = ();

    fn begin_game(&mut self) {
        log::info!("Rendering initial board");
        let pieces = self.position.board().pieces();
        let board = self.drawer.draw_position_from_empty(pieces);

        let mut frame = Frame::from_rgba_speed(
            self.drawer.size() as u16,
            self.drawer.size() as u16,
            &mut board.into_raw(),
            30,
        );
        frame.delay = self.delay;
        self.frames.push(frame);
    }

    fn begin_variation(&mut self) -> Skip {
        Skip(true) // stay in the mainline
    }

    fn san(&mut self, san_plus: SanPlus) {
        if let Ok(m) = san_plus.san.to_move(&self.position) {
            let mut board = self.drawer.image_buffer();
            self.drawer.draw_move(&m, self.position.turn(), &mut board);

            let mut frame = Frame::from_rgba_speed(
                self.drawer.size() as u16,
                self.drawer.size() as u16,
                &mut board.into_raw(),
                10,
            );
            frame.delay = self.delay;
            self.frames.push(frame);
            log::debug!("Pushing frame for move {:?}", m);
            self.position.play_unchecked(&m);
        }
    }

    fn end_game(&mut self) -> Self::Result {
        if let Some(last) = self.frames.last_mut() {
            (*last).delay = self.delay * 5;
        }
        for f in self.frames.iter() {
            self.encoder.write_frame(f);
        }
    }
}
