use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedDeck {
    pub id: String,
    pub name: String,
    pub houses: Vec<String>,
    pub cards: Vec<String>,
}

fn store_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".keyforge").join("decks.json")
}

pub fn load() -> Vec<SavedDeck> {
    let Ok(data) = std::fs::read_to_string(store_path()) else {
        return vec![];
    };
    serde_json::from_str(&data).unwrap_or_default()
}

/// Adds the deck if not already present (by id), then persists. Returns updated list.
pub fn save_deck(deck: SavedDeck) -> Vec<SavedDeck> {
    let mut decks = load();
    if !decks.iter().any(|d| d.id == deck.id) {
        decks.push(deck);
    }
    let path = store_path();
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    if let Ok(s) = serde_json::to_string_pretty(&decks) {
        let _ = std::fs::write(path, s);
    }
    decks
}
