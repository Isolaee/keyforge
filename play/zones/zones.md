# Zones (from Official Rulebook v17.4)

Every card in the game exists in exactly one zone at any time. Zones are divided into two categories: **in-play** and **out-of-play**.

## In-Play Zone

All cards currently in play share a single in-play zone. Cards in play can interact with and be affected by other cards in play.

- When a card is added to the in-play zone it **enters play**.
- When a card leaves the in-play zone it **leaves play**.
- Cards in play exist in one of two states: **ready** (upright) or **exhausted** (turned sideways).

### Battleline (Creatures)

The battleline is the ordered line of creatures a player controls in play.

- Creatures enter play **exhausted** on either flank (controller chooses which).
- The far left and far right positions are the **flanks**. A creature there is a "flank creature."
- If the battleline has exactly one creature, it is on **both** flanks and in the **center**.
- A creature's **neighbors** are the creatures to its immediate left and right (also called "adjacent").
- Center: a creature is in the center when there are equal numbers of creatures to its left and right. Only exists with an odd number of creatures.
- When a creature leaves play, the battleline **shifts inward** to close the gap.
- A creature with **Deploy** can enter anywhere in the battleline, not just the flanks.

### Artifacts

- Artifacts enter play **exhausted**.
- Placed in a row **behind** (below) the controlling player's battleline.
- Remain in play turn to turn.
- Not part of the battleline; have no neighbors or flanking position.

### Upgrades

- Attached to a creature when played (can be on a creature controlled by either player).
- Controlled by the player who played them, even if attached to opponent's creature.
- If the creature leaves play, the upgrade is **discarded**.

## Out-of-Play Zones

Cards not in play exist in one of several out-of-play zones. Each has different visibility rules.

### Deck

- 36 cards (12 per house).
- Cards in deck are **hidden** from both players.
- Order must be maintained unless a card/game effect requires shuffling.
- When deck is empty and a draw is required, shuffle the discard pile to form a new deck.

### Hand

- Visible to **owner only**; hidden from opponent.
- During step 5 (Draw Cards), active player draws until they have 6 cards. No discard to 6 if over.
- Starting hand: first player draws 7, second player draws 6. Each may mulligan once (shuffle back, draw one fewer).

### Discard Pile

- Cards are **faceup**; visible to **both** players at any time.
- Order of cards **must be maintained**.
- When a card is discarded, it is placed faceup **on top** of the discard pile.
- Action cards go here after their effect resolves.
- Destroyed cards go here after "Destroyed:" abilities resolve.
- When deck is empty and a draw is needed, the discard pile is shuffled to become the new deck.

### Archives

- **Facedown** area in front of the player's Archon identity card.
- Visible to **owner only**; hidden from opponent.
- Cards can only be added via **card abilities** (not voluntarily).
- During step 2 (Choose a House), the active player may pick up **all** archived cards and add them to hand (all or nothing).
- No inherent ordering.

### Purged

- Removed from the game entirely.
- Placed **faceup** beneath the owner's identity card.
- Visible to **both** players.
- **No order** to purged cards.
- Only card abilities can interact with purged cards.

## Zone Transitions

| From | To | Trigger |
|------|----|---------|
| Deck | Hand | Draw (step 5, or card effect) |
| Hand | In Play | Playing a creature, artifact, or upgrade |
| Hand | Discard | Discarding from hand, or playing an action card (after resolution) |
| Hand | Archives | Card ability that archives from hand |
| In Play | Discard | Destroyed, sacrificed, or discarded from play |
| In Play | Hand | Returned to hand (by card effect) |
| In Play | Deck | Shuffled into deck (by card effect) |
| In Play | Archives | Archived from play (by card effect) |
| In Play | Purged | Purged from play (by card effect) |
| Discard | Deck | Deck runs out (shuffle discard to form new deck) |
| Archives | Hand | Step 2 (active player may pick up all archives) |

### Leaves Play Rules

When a card leaves play:
- All non-Aember tokens and status counters are removed.
- All upgrades on the card are discarded.
- All lasting effects on the card expire.
- Creature Aember goes to **opponent's** Aember pool.
- Non-creature Aember goes to the **common supply**.
- Card always goes to its **owner's** out-of-play zone (not controller's).

## Play Area Layout (per player)

```
+-----------------------------------------------------+
|  [Archon ID]  [Key 1] [Key 2] [Key 3]  [Aember Pool]|
|                                                       |
|  [Archives]        (facedown, behind identity)        |
|                                                       |
|  [Artifacts]       (row behind battleline)            |
|                                                       |
|  [Battleline]      L_flank ... creatures ... R_flank  |
|                                                       |
|  [Deck]            [Discard Pile]                      |
|                                                       |
|  [Purged]          (faceup, beneath identity)         |
+-----------------------------------------------------+
```

---

## Implementation Plan

### Data Structures

```rust
// src/zones.rs

use std::collections::VecDeque;

pub type CardId = u32;

pub enum Flank {
    Left,
    Right,
}

/// Ordered line of creatures. Index 0 = left flank.
pub struct Battleline {
    creatures: VecDeque<CardId>,
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
```

### Battleline Methods

```rust
impl Battleline {
    pub fn new() -> Self;
    pub fn add(&mut self, id: CardId, flank: Flank);
    pub fn deploy_at(&mut self, index: usize, id: CardId);
    pub fn remove(&mut self, id: CardId);  // auto-collapses gap
    pub fn neighbors(&self, id: CardId) -> (Option<CardId>, Option<CardId>);
    pub fn left_flank(&self) -> Option<CardId>;
    pub fn right_flank(&self) -> Option<CardId>;
    pub fn center(&self) -> Option<CardId>;
    pub fn is_on_flank(&self, id: CardId) -> bool;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}
```

- `VecDeque` gives O(1) insert at both flanks.
- `remove()` inherently collapses the gap (no explicit shift needed).

### Zone Transition Methods

```rust
impl PlayerZones {
    pub fn new(deck: Vec<CardId>) -> Self;

    // deck -> hand
    pub fn draw(&mut self) -> Option<CardId>;

    // hand -> battleline
    pub fn play_creature(&mut self, id: CardId, flank: Flank);

    // hand -> battleline (at position)
    pub fn deploy_creature(&mut self, id: CardId, index: usize);

    // hand -> artifacts
    pub fn play_artifact(&mut self, id: CardId);

    // hand -> discard
    pub fn discard_from_hand(&mut self, id: CardId);

    // battleline/artifacts -> discard
    pub fn destroy(&mut self, id: CardId);

    // hand -> archives
    pub fn archive_from_hand(&mut self, id: CardId);

    // play -> archives
    pub fn archive_from_play(&mut self, id: CardId);

    // archives -> hand (all at once)
    pub fn pick_up_archives(&mut self);

    // any zone -> purged
    pub fn purge(&mut self, id: CardId);

    // play -> hand
    pub fn return_to_hand(&mut self, id: CardId);

    // play -> deck
    pub fn shuffle_into_deck(&mut self, id: CardId);

    // discard -> deck (when deck empty)
    pub fn shuffle_discard_into_deck(&mut self);
}
```

### Upgrades

Upgrades are not a zone — they attach to creatures. Stored on the card itself:

```rust
// in card.rs
pub struct Card {
    pub id: CardId,
    // ...other fields...
    pub upgrades: Vec<CardId>,  // only populated for creatures in play
}
```

When a creature leaves play, its `upgrades` are moved to their owner's discard pile.

### Tests (100% happy path coverage)

| Test | Validates |
|------|-----------|
| `test_draw` | deck -> hand, top card removed |
| `test_draw_empty_reshuffles` | empty deck triggers discard shuffle |
| `test_play_creature_left_flank` | hand -> battleline left |
| `test_play_creature_right_flank` | hand -> battleline right |
| `test_deploy_creature` | hand -> battleline at index |
| `test_play_artifact` | hand -> artifacts |
| `test_discard_from_hand` | hand -> discard top |
| `test_destroy_creature` | battleline -> discard, gap collapses |
| `test_destroy_artifact` | artifacts -> discard |
| `test_archive_from_hand` | hand -> archives |
| `test_archive_from_play` | battleline -> archives |
| `test_pick_up_archives` | archives -> hand (all), archives empty |
| `test_purge` | card removed from any zone, added to purged |
| `test_return_to_hand` | battleline -> hand |
| `test_neighbors` | correct left/right neighbors |
| `test_neighbors_single` | single creature has no neighbors |
| `test_flanks` | left/right flank identification |
| `test_single_creature_both_flanks` | one creature is on both flanks |
| `test_center` | center with odd count, none with even |
| `test_is_on_flank` | flank detection |
