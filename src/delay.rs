use std::str::FromStr;

use crate::error::C2GError;

/// Represents the delays between GIF frames, which can be either a duration in ms, or real time given by %clk comments in PGN headers.
#[derive(Debug, Clone)]
pub enum Delay {
    Duration(u16),
    Real,
}

impl FromStr for Delay {
    type Err = C2GError;

    fn from_str(s: &str) -> Result<Self, C2GError> {
        match s {
            "real" => Ok(Delay::Real),
            n => match n.parse::<u16>() {
                Ok(duration) => Ok(Delay::Duration(duration)),
                Err(_) => Err(C2GError::CannotParseDuration(n.to_string())),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct Delays {
    /// Delay between frames except for the delay after the first and last frames.
    pub frame: Delay,

    /// Delay after the first frame of the game. Must be set separately as otherwise games start without delay.
    pub first_frame: Delay,

    /// Delay after the last frame of the game. Must be set separately as otherwise there is no delay after game ends to digest a position.
    pub last_frame: Delay,
}

impl Delays {
    pub fn new(frame: &Delay, first_frame: &Delay, last_frame: &Delay) -> Delays {
        Delays {
            frame: frame.clone(),
            last_frame: last_frame.clone(),
            first_frame: first_frame.clone(),
        }
    }

    pub fn is_delay_real(&self) -> bool {
        match self.frame {
            Delay::Real => true,
            _ => false,
        }
    }

    pub fn frame_delay(&self) -> Option<u16> {
        match self.frame {
            Delay::Real => None,
            Delay::Duration(d) => Some(d),
        }
    }

    pub fn last_frame_delay(&self) -> Option<u16> {
        match self.last_frame {
            Delay::Real => None,
            Delay::Duration(d) => Some(d),
        }
    }

    pub fn first_frame_delay(&self) -> Option<u16> {
        match self.last_frame {
            Delay::Real => None,
            Delay::Duration(d) => Some(d),
        }
    }
}

impl Default for Delays {
    fn default() -> Self {
        let delay = Delay::Duration(1000);
        Delays::new(&delay, &delay, &delay)
    }
}
