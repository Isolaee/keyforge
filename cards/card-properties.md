# Card Properties (from Official Rulebook v17.4)

## Card Anatomy (numbered on rulebook p.6)

| # | Property | Description |
|---|----------|-------------|
| 1 | House | Icon in upper-left corner. Determines which house the card belongs to. |
| 2 | Card Name | The name of the card. |
| 3 | Card Type | One of: Creature, Action, Artifact, Upgrade. |
| 4 | Bonus Icons | Icons below the house icon. Resolved when card is played. Types: Aember, Capture, Damage, Draw, Discard, House. |
| 5 | Traits | Subtypes/categories (e.g. Giant, Goblin, Beast, Alien, Item). Separated by bullet. |
| 6 | Card Ability Text | The game effect text of the card. |
| 7 | Flavor Text | Italicized narrative text with no game effect. |
| 8 | Power | Creature-only. Numeric value at bottom-left. Determines damage dealt in fights and health. A `~` means no power (not applicable to that card type). |
| 9 | Armor | Creature-only. Numeric value in shield icon to the right of the name. Prevents that much pending damage per turn. A `~` means no armor. |
| 10 | Artist | Credit for card illustration. |
| 11 | Set Icon | Identifies which KeyForge set the card is from. |
| 12 | Card Number | Number within the set. |
| 13 | Rarity | Common, Uncommon, Rare, Special, Token Creature. |
| 14 | Deck Name | Name of the deck this card belongs to. |

Additional properties on Archon Identity card only (not a gameplay card):
- 15: Archon Image
- 16: Deck Registration Code

## Card Types

### Creature
- Has power and armor values
- Enters play on a flank of the battleline (controller chooses which)
- Can be exhausted to: Fight, Reap, or trigger Action/Omni abilities
- Remains in play turn to turn
- Can have upgrades attached

### Action
- Single-use effect
- Played from hand, effect resolves, then card goes to discard pile
- No power or armor

### Artifact
- Enters play exhausted
- Placed in a row behind the battleline
- Remains in play turn to turn
- Used via Action: or Omni: abilities
- No power or armor

### Upgrade
- Attached to a creature when played
- Modifies the creature it is attached to
- If the creature leaves play, the upgrade is discarded
- No power or armor (but may grant power/armor to attached creature)

## Houses

Brobnar, Dis, Ekwidon, Geistoid, Logos, Mars, Redemption, Sanctum, Saurian, Shadows, Skyborn, Star Alliance, Unfathomable, Untamed

## Keywords

| Keyword | Description |
|---------|-------------|
| Alpha | Can only be played if no other cards have been played, used, or discarded this step. |
| Assault (X) | Deals X pending damage to the fought creature before the fight resolves. |
| Capture | Captures 1 aember from opponent onto this creature. |
| Deploy | Can enter play anywhere in your battleline (not just flanks). |
| Elusive | First time this creature is attacked each turn, no damage is dealt. |
| Exalt | Place 1 aember from common supply on this creature. |
| Graft | When this creature enters play, you may attach a card from your hand to it as an upgrade. |
| Haunted | Opponent gains 1 aember when this creature is destroyed. |
| Hazardous (X) | When an enemy creature attacks this creature, deal X damage to the attacker before the fight. |
| Heal | Fully heal this creature when it reaps. |
| Invulnerable | Cannot be dealt damage, cannot be destroyed. |
| Omega | After playing this card, the current step ends. |
| Poison | Any damage dealt by this creature's power during a fight destroys the damaged creature. |
| Skirmish | When this creature fights, it takes no damage in return. |
| Splash-Attack (X) | When this creature fights, deal X damage to each neighbor of the fought creature. |
| Steal | Steal 1 aember (move from opponent's pool to yours). |
| Taunt | Neighbors of this creature cannot be attacked unless they also have taunt. |
| Treachery | This card counts as belonging to the opposing player for deckbuilding/house-check purposes. |
| Versatile | Allows playing one off-house card per turn. |

## Bonus Icons

- Aember: Gain 1 aember from common supply
- Capture: A friendly creature captures 1 aember from opponent
- Damage: Deal 1 damage to a creature in play
- Draw: Draw 1 card
- Discard: Choose and discard 1 card from hand
- House: Card gains an additional house identity (resolved at deck generation, not at play time)

## Ability Timing Triggers

- **Play:** - Resolves when the card is played (after bonus icons)
- **Action:** - Resolves when the card is used (exhausts the card)
- **Omni:** - Like Action but can be used on any house's turn
- **Reap:** - Resolves when a creature reaps (exhausts, gain 1 aember)
- **Fight:** / **Before Fight:** / **After Fight:** - Resolves around fight resolution
- **Destroyed:** - Resolves when the card is destroyed
- **Leaves Play:** - Resolves when the card leaves play for any reason

## Armor Rules

- Prevents pending damage equal to armor value each turn
- Resets each turn
- Additive when gained from multiple sources
- `~` in armor field means creature has no armor (may gain through effects)

## Token Creatures

- Generic creatures created by card effects
- Have no card text, no traits beyond what is specified
- Belong to the house of the card that created them

---

## Implementation Plan

### Core Enums

```rust
// src/card.rs

pub type CardId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum House {
    Brobnar,
    Dis,
    Ekwidon,
    Geistoid,
    Logos,
    Mars,
    Redemption,
    Sanctum,
    Saurian,
    Shadows,
    Skyborn,
    StarAlliance,
    Unfathomable,
    Untamed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CardType {
    Creature,
    Action,
    Artifact,
    Upgrade,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Special,
    TokenCreature,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BonusIcon {
    Aember,
    Capture,
    Damage,
    Draw,
    Discard,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Alpha,
    Assault(u32),
    Capture,
    Deploy,
    Elusive,
    Exalt,
    Hazardous(u32),
    Poison,
    Skirmish,
    SplashAttack(u32),
    Steal,
    Taunt,
}
```

Only keywords relevant to the POC are included. Others (Graft, Haunted, Heal, Invulnerable, Omega, Treachery, Versatile) can be added later.

### Card Definition (static data)

```rust
/// Template for a card — shared across all instances of the same card.
pub struct CardDef {
    pub name: &'static str,
    pub card_type: CardType,
    pub house: House,
    pub power: Option<u32>,       // None for non-creatures
    pub armor: Option<u32>,       // None for non-creatures
    pub keywords: Vec<Keyword>,
    pub bonus_icons: Vec<BonusIcon>,
    pub traits: Vec<&'static str>,
    pub rarity: Rarity,
}
```

### Card Instance (runtime state)

```rust
/// A specific card in a game, with mutable state.
pub struct Card {
    pub id: CardId,
    pub def: &'static CardDef,    // points to shared definition
    pub exhausted: bool,
    pub damage: u32,
    pub aember: u32,              // captured/exalted aember on this card
    pub upgrades: Vec<CardId>,    // only for creatures in play
    pub stun: bool,
    pub ward: bool,
    pub enrage: bool,
    pub power_counters: i32,      // +1 power counters (can be negative via effects)
    pub armor_used_this_turn: u32,
    pub extra_houses: Vec<House>, // from house bonus icons
}
```

### Card Methods

```rust
impl Card {
    pub fn new(id: CardId, def: &'static CardDef) -> Self;

    /// Effective power = base + power_counters. Minimum 0.
    pub fn power(&self) -> u32;

    /// Effective armor = base + gained armor. Resets usage each turn.
    pub fn armor(&self) -> u32;

    /// Remaining armor this turn (armor - armor_used_this_turn).
    pub fn remaining_armor(&self) -> u32;

    /// True if damage >= power (should be destroyed).
    pub fn is_destroyed(&self) -> bool;

    /// Apply pending damage after armor reduction.
    pub fn deal_damage(&mut self, amount: u32);

    /// Remove damage (heal). Cannot go below 0.
    pub fn heal(&mut self, amount: u32);

    /// Remove all damage.
    pub fn full_heal(&mut self);

    /// Reset per-turn state (armor usage, elusive trigger).
    pub fn reset_turn(&mut self);

    /// True if this card has the given keyword.
    pub fn has_keyword(&self, kw: Keyword) -> bool;

    /// True if this card belongs to the given house (including bonus house icons).
    pub fn belongs_to_house(&self, house: House) -> bool;
}
```

### Design Decisions

- **CardDef vs Card split**: `CardDef` is the static template (loaded once). `Card` is the per-game instance with mutable state. This avoids duplicating strings/keywords across copies of the same card.
- **Keyword enum with values**: `Assault(u32)` and `Hazardous(u32)` carry their X value directly. If a creature gains a second instance, the values are summed at resolution time (not stored combined).
- **Abilities are NOT modeled in CardDef**: For this POC, card effect text is handled by game logic keyed on card name, not by a scripting engine. Only inherent properties (power, armor, keywords, house) are on the card struct.
- **Traits as strings**: Traits have no inherent game effect — only referenced by card abilities. Strings suffice for the POC.

### Tests (100% happy path coverage)

| Test | Validates |
|------|-----------|
| `test_card_new` | card starts ready, 0 damage, no aember, no upgrades |
| `test_power_with_counters` | base power + counters, min 0 |
| `test_armor_prevents_damage` | damage reduced by remaining armor |
| `test_armor_resets_each_turn` | `reset_turn()` clears armor usage |
| `test_armor_stacks` | multiple armor sources are additive |
| `test_deal_damage` | damage applied, armor subtracted |
| `test_is_destroyed` | true when damage >= power |
| `test_heal` | removes damage, cannot go below 0 |
| `test_full_heal` | removes all damage |
| `test_has_keyword` | finds keyword in list |
| `test_assault_value` | correct X value extracted |
| `test_belongs_to_house` | matches printed house |
| `test_belongs_to_bonus_house` | matches house bonus icon |
| `test_ward_prevents_damage` | ward blocks all damage, then removed |
| `test_stun_state` | stun flag set/cleared correctly |
