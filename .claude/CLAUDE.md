# Keyforge

Card game POC — functional client-server with basic gameplay, no visuals.

## Commands

- Build: `cargo build`
- Run: `cargo run`
- Test: `cargo test`
- Test coverage target: 100% happy paths

## Stack

- Language: Rust (edition 2024)
- Framework: macroquad 0.4 (docs: https://macroquad.rs/docs/)
- Style: snake_case everywhere

## Domain Model

Rulesbook: https://keyforging.com/wp-content/uploads/2024/09/KeyForge-Rulebook-v17-4.pdf

### Cards
- Properties: house, power, armor, effect, extra, keywords, card_type, subtype
- Inherent actions: reap, attack
- Additional actions only if granted by card effect

### Zones
- Player zones: play, hand, deck, archive, discard, info
- Play sub-zones: main, left_flank, right_flank, non_creature

### Other objects
- Tokens
    - damage marker (represent damage)
    - Stun (Prevents one action)
    - +1 power (Gives +1 power)
    - Ward (prevent next damage)
    - Rage (must attack whena able)
    - Chain (reduces draws)
    - aember (six aember is a key)
    - Key (Three keys are victory condition)

### Keywords
- Alpha
- Assault (X)
- Capture
- Deploy
- Elusive
- Exalt
- Graft
- Haunted
- Hazardous (X)
- Heal
- Invulnerable
- Omega
- Poison
- Skirmish
- Splash-Attack (X)
- Steal
- Taunt
- Treachery
- Versatile

## Scope

This is a simple POC. Prefer minimal implementations. Only a few keywords will exist.
