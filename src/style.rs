use std::collections::HashSet;
use std::str::FromStr;

use crate::error::C2GError;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Hash)]
pub enum StyleComponent {
    Plain,
    Full,
    PlayerBars,
    Terminations,
    Coordinates,
    Ranks,
    Files,
}

impl StyleComponent {
    pub fn components(self) -> &'static [StyleComponent] {
        match self {
            StyleComponent::Coordinates => &[StyleComponent::Ranks, StyleComponent::Files],
            StyleComponent::Ranks => &[StyleComponent::Ranks],
            StyleComponent::Files => &[StyleComponent::Files],
            StyleComponent::PlayerBars => &[StyleComponent::PlayerBars],
            StyleComponent::Terminations => &[StyleComponent::Terminations],
            StyleComponent::Full => &[
                StyleComponent::Ranks,
                StyleComponent::Files,
                StyleComponent::PlayerBars,
                StyleComponent::Terminations,
            ],
            StyleComponent::Plain => &[],
        }
    }
}

impl FromStr for StyleComponent {
    type Err = C2GError;

    fn from_str(s: &str) -> Result<Self, C2GError> {
        match s {
            "ranks" => Ok(StyleComponent::Ranks),
            "files" => Ok(StyleComponent::Files),
            "player-bars" => Ok(StyleComponent::PlayerBars),
            "terminations" => Ok(StyleComponent::Terminations),
            "full" => Ok(StyleComponent::Full),
            "plain" => Ok(StyleComponent::Plain),
            _ => Err(C2GError::UnknownStyle(s.to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct StyleComponents(pub HashSet<StyleComponent>);

impl StyleComponents {
    pub fn new(components: &[StyleComponent]) -> StyleComponents {
        StyleComponents(components.iter().cloned().collect())
    }

    pub fn player_bars(&self) -> bool {
        self.0.contains(&StyleComponent::PlayerBars)
    }

    pub fn terminations(&self) -> bool {
        self.0.contains(&StyleComponent::Terminations)
    }

    pub fn ranks(&self) -> bool {
        self.0.contains(&StyleComponent::Ranks)
    }

    pub fn files(&self) -> bool {
        self.0.contains(&StyleComponent::Files)
    }

    pub fn plain(&self) -> bool {
        self.0.iter().all(|c| c == &StyleComponent::Plain)
    }
}

impl Default for StyleComponents {
    fn default() -> Self {
        StyleComponents::new(&[StyleComponent::Full])
    }
}
