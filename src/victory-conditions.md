# Victory Conditions (from Official Rulebook v17.4)

## Primary Victory Condition: Forge Three Keys

A player **immediately wins the game** when they forge their third key. This is checked at any point during gameplay — if a card effect causes a key to be forged outside of Step 1, the win is still immediate.

### Keys

- Each player has **three key tokens**: red, blue, and yellow.
- Each key has two sides: **unforged** and **forged**.
- All keys start unforged.
- Some card abilities reference specific key colors or whether keys are forged/unforged.

### Forging a Key (Step 1 of Turn)

If the active player has enough Aember to forge a key, they **must** do so (it is not optional).

1. Spend Aember from the Aember pool equal to the **current key cost**.
2. Flip any one unforged key token to its forged side.
3. Spent Aember is returned to the common supply.

Rules:
- **Default key cost**: 6 Aember.
- Card abilities may increase or decrease the cost. Key cost **cannot be less than zero**.
- **Only one key** can be forged during Step 1 per turn, even if the player has enough Aember for multiple keys.
- Some cards allow Aember **on those cards** to be spent alongside pool Aember when forging.

### Unforge

- If a forged key is **unforged** (by a card effect), flip it back to its unforged side.
- It no longer counts toward victory and must be forged again.

### Check

- At the **end of a player's turn**, if that player has enough Aember in their pool to afford a key, they announce **"Check!"** to warn the opponent that forging is imminent on their next turn.

## No Other Standard Victory Conditions

There is no win-by-decking, no win-by-elimination, and no draw condition in the base rules. The game continues until one player forges their third key.

Some card effects can forge keys **outside of Step 1** (e.g., "forge a key at no cost" or "forge a key at current cost"). These still count and can trigger an immediate win.

## Summary

| Condition | Result |
|-----------|--------|
| Player forges their 3rd key | That player **wins immediately** |
| Key is unforged by card effect | Key must be re-forged; victory revoked if now < 3 |
| Player has >= key cost Aember at end of turn | Announce "Check!" (warning, not a win) |
| Deck runs out | No effect on victory — shuffle discard to form new deck |

---

## Implementation Plan

### Data Structures

```rust
// src/victory.rs

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
```

### Methods

```rust
impl PlayerKeys {
    pub fn new() -> Self;

    /// Forge a key of any color. Returns the color forged.
    /// Panics if no unforged keys remain (should not happen in valid game).
    pub fn forge(&mut self, color: KeyColor);

    /// Unforge a key (card effect). No-op if already unforged.
    pub fn unforge(&mut self, color: KeyColor);

    /// Count of forged keys.
    pub fn forged_count(&self) -> u8;

    /// True if 3 keys forged — immediate win.
    pub fn has_won(&self) -> bool;

    /// True if a specific key is forged.
    pub fn is_forged(&self, color: KeyColor) -> bool;

    /// Returns colors of all unforged keys.
    pub fn unforged_keys(&self) -> Vec<KeyColor>;
}
```

### Forging Logic (in game.rs)

```rust
/// Step 1 of turn. Returns true if the player wins.
pub fn step_forge_key(player: &mut Player) -> bool {
    let cost = player.current_key_cost();  // default 6, modified by effects
    if player.aember_pool >= cost && player.keys.forged_count() < 3 {
        player.aember_pool -= cost;
        let color = player.choose_key_to_forge(); // any unforged key
        player.keys.forge(color);
    }
    player.keys.has_won()
}

/// End-of-turn check announcement.
pub fn should_announce_check(player: &Player) -> bool {
    player.aember_pool >= player.current_key_cost()
        && player.keys.forged_count() < 3
}
```

### Key cost modifiers

```rust
impl Player {
    /// Base cost is 6. Card effects add/subtract. Minimum 0.
    pub fn current_key_cost(&self) -> u32 {
        (6i32 + self.key_cost_modifier).max(0) as u32
    }
}
```

### Off-step forging (card effects)

Some cards say "forge a key at no cost" or "forge a key at current cost." These bypass Step 1 but still trigger `has_won()`:

```rust
pub fn forge_key_at_cost(player: &mut Player, cost: u32) -> bool {
    if player.aember_pool >= cost && player.keys.forged_count() < 3 {
        player.aember_pool -= cost;
        let color = player.choose_key_to_forge();
        player.keys.forge(color);
    }
    player.keys.has_won()
}
```

### Tests (100% happy path coverage)

| Test | Validates |
|------|-----------|
| `test_initial_keys_unforged` | all 3 keys start unforged |
| `test_forge_key` | forging flips key, reduces aember |
| `test_forge_three_wins` | `has_won()` true after 3rd forge |
| `test_forge_two_not_won` | `has_won()` false after 2 forges |
| `test_unforge_key` | unforged key no longer counts |
| `test_unforge_revokes_win` | 3 forged -> unforge 1 -> `has_won()` false |
| `test_key_cost_default` | default cost is 6 |
| `test_key_cost_modifier` | positive/negative modifiers work, min 0 |
| `test_step_forge_mandatory` | step 1 forges if aember >= cost |
| `test_step_forge_skipped` | step 1 skips if aember < cost |
| `test_only_one_key_per_step` | max 1 key forged in step 1 |
| `test_check_announcement` | true when aember >= cost and < 3 keys |
| `test_forge_at_no_cost` | card effect forges without spending aember |
| `test_forge_at_current_cost` | card effect uses current (modified) cost |
