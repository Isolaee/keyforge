use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::card::CardId;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum Flank {
    Left,
    Right,
}

/// Ordered line of creatures. Index 0 = left flank.
pub struct Battleline {
    creatures: VecDeque<CardId>,
}

impl Battleline {
    pub fn new() -> Self {
        Self { creatures: VecDeque::new() }
    }

    pub fn add(&mut self, id: CardId, flank: Flank) {
        match flank {
            Flank::Left => self.creatures.push_front(id),
            Flank::Right => self.creatures.push_back(id),
        }
    }

    pub fn deploy_at(&mut self, index: usize, id: CardId) {
        self.creatures.insert(index, id);
    }

    /// Remove a creature; the gap collapses automatically.
    pub fn remove(&mut self, id: CardId) {
        if let Some(pos) = self.creatures.iter().position(|&c| c == id) {
            self.creatures.remove(pos);
        }
    }

    /// Returns (left_neighbor, right_neighbor).
    pub fn neighbors(&self, id: CardId) -> (Option<CardId>, Option<CardId>) {
        let pos = match self.creatures.iter().position(|&c| c == id) {
            Some(p) => p,
            None => return (None, None),
        };
        let left = if pos > 0 { Some(self.creatures[pos - 1]) } else { None };
        let right = self.creatures.get(pos + 1).copied();
        (left, right)
    }

    pub fn left_flank(&self) -> Option<CardId> {
        self.creatures.front().copied()
    }

    pub fn right_flank(&self) -> Option<CardId> {
        self.creatures.back().copied()
    }

    /// Center creature exists only when count is odd.
    pub fn center(&self) -> Option<CardId> {
        let len = self.creatures.len();
        if len == 0 || len % 2 == 0 {
            None
        } else {
            Some(self.creatures[len / 2])
        }
    }

    pub fn is_on_flank(&self, id: CardId) -> bool {
        self.left_flank() == Some(id) || self.right_flank() == Some(id)
    }

    pub fn len(&self) -> usize {
        self.creatures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.creatures.is_empty()
    }

    pub fn creature_ids(&self) -> Vec<CardId> {
        self.creatures.iter().copied().collect()
    }
}

/// All zones belonging to one player.
pub struct PlayerZones {
    pub deck: Vec<CardId>,
    pub hand: Vec<CardId>,
    pub battleline: Battleline,
    pub artifacts: Vec<CardId>,
    pub discard: Vec<CardId>,
    pub archives: Vec<CardId>,
    pub purged: Vec<CardId>,
}

impl PlayerZones {
    pub fn new(deck: Vec<CardId>) -> Self {
        Self {
            deck,
            hand: Vec::new(),
            battleline: Battleline::new(),
            artifacts: Vec::new(),
            discard: Vec::new(),
            archives: Vec::new(),
            purged: Vec::new(),
        }
    }

    /// Draw one card: deck -> hand. When deck is empty, shuffles discard into deck first.
    pub fn draw(&mut self) -> Option<CardId> {
        if self.deck.is_empty() {
            self.shuffle_discard_into_deck();
        }
        let card = self.deck.pop()?;
        self.hand.push(card);
        Some(card)
    }

    /// hand -> battleline flank
    pub fn play_creature(&mut self, id: CardId, flank: Flank) {
        self.hand.retain(|&c| c != id);
        self.battleline.add(id, flank);
    }

    /// hand -> battleline at position (Deploy keyword)
    pub fn deploy_creature(&mut self, id: CardId, index: usize) {
        self.hand.retain(|&c| c != id);
        self.battleline.deploy_at(index, id);
    }

    /// hand -> artifacts
    pub fn play_artifact(&mut self, id: CardId) {
        self.hand.retain(|&c| c != id);
        self.artifacts.push(id);
    }

    /// hand -> discard
    pub fn discard_from_hand(&mut self, id: CardId) {
        self.hand.retain(|&c| c != id);
        self.discard.push(id);
    }

    /// battleline or artifacts -> discard
    pub fn destroy(&mut self, id: CardId) {
        let in_battleline = self.battleline.creatures.contains(&id);
        if in_battleline {
            self.battleline.remove(id);
        } else {
            self.artifacts.retain(|&c| c != id);
        }
        self.discard.push(id);
    }

    /// hand -> archives
    pub fn archive_from_hand(&mut self, id: CardId) {
        self.hand.retain(|&c| c != id);
        self.archives.push(id);
    }

    /// battleline -> archives
    pub fn archive_from_play(&mut self, id: CardId) {
        self.battleline.remove(id);
        self.archives.push(id);
    }

    /// archives -> hand (all at once)
    pub fn pick_up_archives(&mut self) {
        self.hand.extend(self.archives.drain(..));
    }

    /// Remove card from any zone and add to purged.
    pub fn purge(&mut self, id: CardId) {
        self.deck.retain(|&c| c != id);
        self.hand.retain(|&c| c != id);
        self.battleline.remove(id);
        self.artifacts.retain(|&c| c != id);
        self.discard.retain(|&c| c != id);
        self.archives.retain(|&c| c != id);
        self.purged.push(id);
    }

    /// battleline -> hand
    pub fn return_to_hand(&mut self, id: CardId) {
        self.battleline.remove(id);
        self.hand.push(id);
    }

    /// battleline -> deck (shuffled in)
    pub fn shuffle_into_deck(&mut self, id: CardId) {
        self.battleline.remove(id);
        self.deck.push(id);
    }

    /// discard -> deck (when deck empty)
    pub fn shuffle_discard_into_deck(&mut self) {
        self.deck.extend(self.discard.drain(..));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zones_with_deck(ids: Vec<CardId>) -> PlayerZones {
        PlayerZones::new(ids)
    }

    #[test]
    fn test_draw() {
        let mut z = zones_with_deck(vec![1, 2, 3]);
        let drawn = z.draw();
        // deck is treated as a stack, last element is "top"
        assert_eq!(drawn, Some(3));
        assert_eq!(z.deck.len(), 2);
        assert!(z.hand.contains(&3));
    }

    #[test]
    fn test_draw_empty_reshuffles() {
        let mut z = zones_with_deck(vec![]);
        z.discard.push(10);
        z.discard.push(11);
        let drawn = z.draw();
        assert!(drawn.is_some());
        assert!(z.discard.is_empty());
    }

    #[test]
    fn test_play_creature_left_flank() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(1);
        z.play_creature(1, Flank::Left);
        assert!(!z.hand.contains(&1));
        assert_eq!(z.battleline.left_flank(), Some(1));
    }

    #[test]
    fn test_play_creature_right_flank() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(1);
        z.hand.push(2);
        z.play_creature(1, Flank::Left);
        z.hand.push(2);
        z.play_creature(2, Flank::Right);
        assert_eq!(z.battleline.right_flank(), Some(2));
    }

    #[test]
    fn test_deploy_creature() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(1);
        z.hand.push(2);
        z.hand.push(3);
        z.play_creature(1, Flank::Left);
        z.play_creature(3, Flank::Right);
        z.hand.push(2);
        z.deploy_creature(2, 1); // insert between 1 and 3
        assert_eq!(z.battleline.neighbors(2), (Some(1), Some(3)));
    }

    #[test]
    fn test_play_artifact() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(5);
        z.play_artifact(5);
        assert!(!z.hand.contains(&5));
        assert!(z.artifacts.contains(&5));
    }

    #[test]
    fn test_discard_from_hand() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(7);
        z.discard_from_hand(7);
        assert!(!z.hand.contains(&7));
        assert_eq!(z.discard.last(), Some(&7));
    }

    #[test]
    fn test_destroy_creature() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(1);
        z.hand.push(2);
        z.hand.push(3);
        z.play_creature(1, Flank::Left);
        z.play_creature(2, Flank::Right);
        z.play_creature(3, Flank::Right);
        // battleline: [1, 2, 3]
        z.destroy(2);
        // gap collapses: [1, 3]
        assert_eq!(z.battleline.len(), 2);
        assert!(z.discard.contains(&2));
        assert_eq!(z.battleline.neighbors(1), (None, Some(3)));
    }

    #[test]
    fn test_destroy_artifact() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(9);
        z.play_artifact(9);
        z.destroy(9);
        assert!(!z.artifacts.contains(&9));
        assert!(z.discard.contains(&9));
    }

    #[test]
    fn test_archive_from_hand() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(4);
        z.archive_from_hand(4);
        assert!(!z.hand.contains(&4));
        assert!(z.archives.contains(&4));
    }

    #[test]
    fn test_archive_from_play() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(6);
        z.play_creature(6, Flank::Left);
        z.archive_from_play(6);
        assert_eq!(z.battleline.len(), 0);
        assert!(z.archives.contains(&6));
    }

    #[test]
    fn test_pick_up_archives() {
        let mut z = zones_with_deck(vec![]);
        z.archives.push(10);
        z.archives.push(11);
        z.pick_up_archives();
        assert!(z.archives.is_empty());
        assert!(z.hand.contains(&10));
        assert!(z.hand.contains(&11));
    }

    #[test]
    fn test_purge() {
        let mut z = zones_with_deck(vec![]);
        z.discard.push(99);
        z.purge(99);
        assert!(!z.discard.contains(&99));
        assert!(z.purged.contains(&99));
    }

    #[test]
    fn test_return_to_hand() {
        let mut z = zones_with_deck(vec![]);
        z.hand.push(20);
        z.play_creature(20, Flank::Left);
        z.return_to_hand(20);
        assert_eq!(z.battleline.len(), 0);
        assert!(z.hand.contains(&20));
    }

    #[test]
    fn test_neighbors() {
        let mut b = Battleline::new();
        b.add(1, Flank::Right);
        b.add(2, Flank::Right);
        b.add(3, Flank::Right);
        assert_eq!(b.neighbors(2), (Some(1), Some(3)));
    }

    #[test]
    fn test_neighbors_single() {
        let mut b = Battleline::new();
        b.add(1, Flank::Left);
        assert_eq!(b.neighbors(1), (None, None));
    }

    #[test]
    fn test_flanks() {
        let mut b = Battleline::new();
        b.add(1, Flank::Right);
        b.add(2, Flank::Right);
        b.add(3, Flank::Right);
        assert_eq!(b.left_flank(), Some(1));
        assert_eq!(b.right_flank(), Some(3));
    }

    #[test]
    fn test_single_creature_both_flanks() {
        let mut b = Battleline::new();
        b.add(1, Flank::Left);
        assert_eq!(b.left_flank(), Some(1));
        assert_eq!(b.right_flank(), Some(1));
    }

    #[test]
    fn test_center() {
        let mut b = Battleline::new();
        b.add(1, Flank::Right);
        b.add(2, Flank::Right);
        b.add(3, Flank::Right);
        assert_eq!(b.center(), Some(2)); // odd count
        b.add(4, Flank::Right);
        assert_eq!(b.center(), None); // even count
    }

    #[test]
    fn test_is_on_flank() {
        let mut b = Battleline::new();
        b.add(1, Flank::Right);
        b.add(2, Flank::Right);
        b.add(3, Flank::Right);
        assert!(b.is_on_flank(1));
        assert!(b.is_on_flank(3));
        assert!(!b.is_on_flank(2));
    }
}
