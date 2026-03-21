use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::card::{Card, CardDef, CardId};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

fn next_id() -> CardId {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Build a deck from a slice of card definitions.
/// Returns (card registry, ordered deck ids — last element is top of deck).
pub fn build_deck(defs: &[&'static CardDef]) -> (HashMap<CardId, Card>, Vec<CardId>) {
    let mut cards = HashMap::new();
    let mut ids = Vec::new();
    for &def in defs {
        let id = next_id();
        cards.insert(id, Card::new(id, def));
        ids.push(id);
    }
    (cards, ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::TROLL;

    #[test]
    fn test_build_deck() {
        let (cards, ids) = build_deck(&[&TROLL, &TROLL]);
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0], ids[1]); // unique ids
        assert!(cards.contains_key(&ids[0]));
        assert!(cards.contains_key(&ids[1]));
    }

    #[test]
    fn test_build_deck_preserves_def() {
        let (cards, ids) = build_deck(&[&TROLL]);
        assert_eq!(cards[&ids[0]].def.name, "Troll");
    }
}
