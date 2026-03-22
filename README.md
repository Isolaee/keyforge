# Keyforge

A hobby project — a digital client for the [KeyForge](https://www.keyforgegame.com/) card game. This is a proof-of-concept focused on functional gameplay mechanics, not visuals.

## Features

- Two-player local gameplay on a shared screen
- Turn-based game loop: choose a house, take actions, end turn
- Three houses playable: Brobnar, Dis, Shadows
- Card types: Creatures, Actions, Artifacts, Upgrades
- Creature actions: reap (gain aember) and attack
- Aember pool and key forging — first to forge 3 keys wins
- Card zones: hand, deck, discard, archive, battleline (with left/right flanks), artifacts
- Keyword mechanics: Assault, Elusive, Poison, Skirmish, Steal, Taunt, and more
- Card status tokens: damage, stun, ward, power counters, exhausted state
- Card effects on reap, fight, play, and destroyed triggers (e.g. draw cards, deal damage to all enemies, steal aember)
- Drag-and-drop card play with click-to-select fallback
- Resizable window with dynamic layout scaling

### Cards included

| Card | House | Type |
|---|---|---|
| Troll | Brobnar | Creature (Taunt) |
| Smaaash | Brobnar | Creature (Assault 2) |
| Banner of Battle | Brobnar | Artifact |
| Vezyma Thinkdrone | Dis | Creature (Poison, draws on reap) |
| Plague | Dis | Action (deals 1 damage to all enemies) |
| Silvertooth | Shadows | Creature (Skirmish, Steal) |
| Shadow Self | Shadows | Upgrade (Elusive) |

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)

## Running

```sh
cargo run
```

## Building

```sh
cargo build --release
```

## Testing

```sh
cargo test
```

## Controls

| Input | Action |
|---|---|
| Click house button | Choose active house for the turn |
| Click card in hand | Select card |
| Drag card to flank zone | Play creature to left or right flank |
| Drag card to artifact zone | Play artifact |
| Click own creature once | Select it |
| Click own creature again | Reap (gain 1 aember, exhausts creature) |
| Select own creature + click enemy | Attack |
| Drag card to Discard zone | Discard from hand |
| Right-click | Deselect |
| End Turn button | Pass turn to opponent |

## Project structure

```
src/
  main.rs    — window, rendering, input handling
  game.rs    — game state, turn logic, actions
  card.rs    — card data model, keywords, effects
  cards.rs   — static card definitions
  deck.rs    — deck construction
  zones.rs   — player zones (hand, deck, battleline, etc.)
  victory.rs — win condition (3 keys forged)
```

## Notes

This is a personal hobby project and is not affiliated with Ghost Galaxy. KeyForge is a trademark of its respective owners.
