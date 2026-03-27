# Keyforge

A hobby project — a digital client for the [KeyForge](https://www.keyforgegame.com/) card game. This is a proof-of-concept focused on functional gameplay mechanics, not visuals.

## Features

- Two-player client-server gameplay over TCP
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

Start the server (waits for 2 client connections):

```sh
cargo run --bin server
```

Optionally pass a bind address (default `127.0.0.1:9999`):

```sh
cargo run --bin server -- 0.0.0.0:9999
```

Then launch two clients (each in its own terminal):

```sh
cargo run                          # connects to 127.0.0.1:9999
cargo run -- 192.168.1.10:9999    # or specify a remote address
```

## Building

```sh
cargo build --release
```

Produces two binaries: `target/release/keyforge` (client) and `target/release/server`.

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

## Architecture

The game uses a client-server model over TCP with newline-delimited JSON messages.

- **Server** owns the authoritative `GameState`, validates all actions, and broadcasts a filtered `ClientGameView` to each player after every move. Opponent hand contents and archive contents are hidden (only counts are sent).
- **Client** renders the `ClientGameView` it receives and sends `ClientMessage` actions (choose house, play card, reap, attack, end turn, etc.) to the server.

## Project structure

```
src/
  lib.rs       — shared library root (re-exports all modules)
  main.rs      — GUI client (macroquad) — connects to server over TCP
  bin/
    server.rs  — TCP server — accepts 2 players, runs game loop
  game.rs      — game state, turn logic, actions
  card.rs      — card data model, keywords, effects
  cards.rs     — static card definitions
  deck.rs      — deck construction
  zones.rs     — player zones (hand, deck, battleline, etc.)
  victory.rs   — win condition (3 keys forged)
  protocol.rs  — ClientMessage, ServerMessage, ClientGameView (serde)
  view.rs      — converts GameState into a filtered ClientGameView
```

## Notes

This is a personal hobby project and is not affiliated with Ghost Galaxy. KeyForge is a trademark of its respective owners.
