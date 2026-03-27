use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum KeyColor {
    Red,
    Blue,
    Yellow,
}

pub struct Key {
    pub color: KeyColor,
    pub forged: bool,
}

pub struct PlayerKeys {
    pub keys: [Key; 3],
}

impl PlayerKeys {
    pub fn new() -> Self {
        Self {
            keys: [
                Key { color: KeyColor::Red, forged: false },
                Key { color: KeyColor::Blue, forged: false },
                Key { color: KeyColor::Yellow, forged: false },
            ],
        }
    }

    /// Forge the key of the given color.
    pub fn forge(&mut self, color: KeyColor) {
        if let Some(k) = self.keys.iter_mut().find(|k| k.color == color) {
            k.forged = true;
        }
    }

    /// Unforge the key of the given color. No-op if already unforged.
    pub fn unforge(&mut self, color: KeyColor) {
        if let Some(k) = self.keys.iter_mut().find(|k| k.color == color) {
            k.forged = false;
        }
    }

    /// Count of forged keys.
    pub fn forged_count(&self) -> u8 {
        self.keys.iter().filter(|k| k.forged).count() as u8
    }

    /// True if all 3 keys are forged — immediate win.
    pub fn has_won(&self) -> bool {
        self.forged_count() == 3
    }

    /// True if the specific key is forged.
    pub fn is_forged(&self, color: KeyColor) -> bool {
        self.keys.iter().any(|k| k.color == color && k.forged)
    }

    /// Returns colors of all unforged keys.
    pub fn unforged_keys(&self) -> Vec<KeyColor> {
        self.keys.iter().filter(|k| !k.forged).map(|k| k.color).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_keys_unforged() {
        let pk = PlayerKeys::new();
        assert_eq!(pk.forged_count(), 0);
        assert_eq!(pk.unforged_keys().len(), 3);
    }

    #[test]
    fn test_forge_key() {
        let mut pk = PlayerKeys::new();
        pk.forge(KeyColor::Red);
        assert!(pk.is_forged(KeyColor::Red));
        assert_eq!(pk.forged_count(), 1);
    }

    #[test]
    fn test_forge_three_wins() {
        let mut pk = PlayerKeys::new();
        pk.forge(KeyColor::Red);
        pk.forge(KeyColor::Blue);
        pk.forge(KeyColor::Yellow);
        assert!(pk.has_won());
    }

    #[test]
    fn test_forge_two_not_won() {
        let mut pk = PlayerKeys::new();
        pk.forge(KeyColor::Red);
        pk.forge(KeyColor::Blue);
        assert!(!pk.has_won());
    }

    #[test]
    fn test_unforge_key() {
        let mut pk = PlayerKeys::new();
        pk.forge(KeyColor::Blue);
        pk.unforge(KeyColor::Blue);
        assert!(!pk.is_forged(KeyColor::Blue));
        assert_eq!(pk.forged_count(), 0);
    }

    #[test]
    fn test_unforge_revokes_win() {
        let mut pk = PlayerKeys::new();
        pk.forge(KeyColor::Red);
        pk.forge(KeyColor::Blue);
        pk.forge(KeyColor::Yellow);
        assert!(pk.has_won());
        pk.unforge(KeyColor::Yellow);
        assert!(!pk.has_won());
    }
}
