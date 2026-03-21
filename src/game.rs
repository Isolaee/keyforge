use std::collections::HashMap;

use crate::card::{Card, CardId, CardType, House, Keyword};
use crate::victory::{KeyColor, PlayerKeys};
use crate::zones::{Flank, PlayerZones};

// ---------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------

pub struct Player {
    pub aember_pool: u32,
    pub keys: PlayerKeys,
    pub key_cost_modifier: i32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            aember_pool: 0,
            keys: PlayerKeys::new(),
            key_cost_modifier: 0,
        }
    }

    /// Base cost is 6. Card effects add/subtract. Minimum 0.
    pub fn current_key_cost(&self) -> u32 {
        (6i32 + self.key_cost_modifier).max(0) as u32
    }

    /// Returns the first unforged key color.
    pub fn choose_key_to_forge(&self) -> KeyColor {
        self.keys
            .unforged_keys()
            .into_iter()
            .next()
            .expect("no unforged keys remain")
    }
}

// ---------------------------------------------------------------------------
// Game state
// ---------------------------------------------------------------------------

pub struct PlayerState {
    pub player: Player,
    pub zones: PlayerZones,
}

pub struct GameState {
    pub players: [PlayerState; 2],
    pub cards: HashMap<CardId, Card>,
    pub active_player: usize,
    pub active_house: Option<House>,
    pub turn: u32,
}

impl GameState {
    pub fn new(
        p0_deck: Vec<CardId>,
        p1_deck: Vec<CardId>,
        cards: HashMap<CardId, Card>,
    ) -> Self {
        Self {
            players: [
                PlayerState { player: Player::new(), zones: PlayerZones::new(p0_deck) },
                PlayerState { player: Player::new(), zones: PlayerZones::new(p1_deck) },
            ],
            cards,
            active_player: 0,
            active_house: None,
            turn: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Step 1 — forge a key
// ---------------------------------------------------------------------------

/// Step 1 of turn. Returns true if the player wins.
pub fn step_forge_key(player: &mut Player) -> bool {
    let cost = player.current_key_cost();
    if player.aember_pool >= cost && player.keys.forged_count() < 3 {
        player.aember_pool -= cost;
        let color = player.choose_key_to_forge();
        player.keys.forge(color);
    }
    player.keys.has_won()
}

/// End-of-turn check announcement.
pub fn should_announce_check(player: &Player) -> bool {
    player.aember_pool >= player.current_key_cost() && player.keys.forged_count() < 3
}

/// Forge a key outside of Step 1 (card effect). Returns true if the player wins.
pub fn forge_key_at_cost(player: &mut Player, cost: u32) -> bool {
    if player.aember_pool >= cost && player.keys.forged_count() < 3 {
        player.aember_pool -= cost;
        let color = player.choose_key_to_forge();
        player.keys.forge(color);
    }
    player.keys.has_won()
}

// ---------------------------------------------------------------------------
// Step 2 — choose house
// ---------------------------------------------------------------------------

/// Set the active house for this turn. Optionally pick up all archived cards.
pub fn choose_house(game: &mut GameState, house: House, pick_up_archives: bool) {
    game.active_house = Some(house);
    if pick_up_archives {
        game.players[game.active_player].zones.pick_up_archives();
    }
}

// ---------------------------------------------------------------------------
// Step 3 — play, use, discard cards
// ---------------------------------------------------------------------------

/// Play a card from hand.
/// Creatures go to the battleline at `flank` (ignored for other types).
/// Actions go to discard. Artifacts go to the artifact zone.
/// Upgrades are discarded (full upgrade attachment is out of scope for POC).
pub fn play_card(game: &mut GameState, card_id: CardId, flank: Flank) {
    let card_type = game.cards[&card_id].def.card_type;
    let pi = game.active_player;
    match card_type {
        CardType::Creature => game.players[pi].zones.play_creature(card_id, flank),
        CardType::Artifact => game.players[pi].zones.play_artifact(card_id),
        CardType::Action | CardType::Upgrade => {
            game.players[pi].zones.discard_from_hand(card_id)
        }
    }
}

/// Exhaust a friendly creature and gain 1 Aember.
pub fn reap(game: &mut GameState, card_id: CardId) {
    game.cards.get_mut(&card_id).unwrap().exhausted = true;
    game.players[game.active_player].player.aember_pool += 1;
}

/// Fight: attacker attacks defender.
/// Handles Assault, Hazardous, Skirmish, Poison, SplashAttack keywords.
/// Destroyed creatures are moved to discard automatically.
pub fn attack(game: &mut GameState, attacker_id: CardId, defender_id: CardId) {
    let def_idx = 1 - game.active_player;

    // Gather combat info (immutable reads first)
    let assault = x_keyword(game.cards[&attacker_id].def.keywords, Keyword::Assault(0));
    let hazardous = x_keyword(game.cards[&defender_id].def.keywords, Keyword::Hazardous(0));
    let splash = x_keyword(game.cards[&attacker_id].def.keywords, Keyword::SplashAttack(0));
    let skirmish = game.cards[&attacker_id].has_keyword(Keyword::Skirmish);
    let attacker_poison = game.cards[&attacker_id].has_keyword(Keyword::Poison);
    let defender_poison = game.cards[&defender_id].has_keyword(Keyword::Poison);
    let attacker_power = game.cards[&attacker_id].power();
    let defender_power = game.cards[&defender_id].power();
    let (left_nb, right_nb) = game.players[def_idx].zones.battleline.neighbors(defender_id);

    // Exhaust attacker
    game.cards.get_mut(&attacker_id).unwrap().exhausted = true;

    // Pre-fight damage (Assault / Hazardous)
    if assault > 0 {
        game.cards.get_mut(&defender_id).unwrap().deal_damage(assault);
    }
    if hazardous > 0 {
        game.cards.get_mut(&attacker_id).unwrap().deal_damage(hazardous);
    }

    // Fight damage (only if defender survived pre-fight)
    if !game.cards[&defender_id].is_destroyed() {
        game.cards.get_mut(&defender_id).unwrap().deal_damage(attacker_power);
        if !skirmish {
            game.cards.get_mut(&attacker_id).unwrap().deal_damage(defender_power);
        }
    }

    // Poison: any power damage dealt during fight destroys the target
    if attacker_poison && attacker_power > 0 {
        let c = game.cards.get_mut(&defender_id).unwrap();
        if c.damage > 0 {
            c.damage = c.power().max(1);
        }
    }
    if defender_poison && defender_power > 0 && !skirmish {
        let c = game.cards.get_mut(&attacker_id).unwrap();
        if c.damage > 0 {
            c.damage = c.power().max(1);
        }
    }

    // SplashAttack: deal splash damage to neighbors of defender
    if splash > 0 {
        for nb in [left_nb, right_nb].into_iter().flatten() {
            if let Some(c) = game.cards.get_mut(&nb) {
                c.deal_damage(splash);
            }
        }
    }

    destroy_dead(game);
}

// ---------------------------------------------------------------------------
// End of turn (Step 5)
// ---------------------------------------------------------------------------

/// Ready all friendly cards in play, reset per-turn state, draw up to 6, advance turn.
pub fn end_turn(game: &mut GameState) {
    let pi = game.active_player;

    let in_play: Vec<CardId> = game.players[pi]
        .zones
        .battleline
        .creature_ids()
        .into_iter()
        .chain(game.players[pi].zones.artifacts.iter().copied())
        .collect();

    for id in in_play {
        if let Some(card) = game.cards.get_mut(&id) {
            card.exhausted = false;
            card.reset_turn();
        }
    }

    // Draw up to 6
    while game.players[pi].zones.hand.len() < 6 {
        if game.players[pi].zones.draw().is_none() {
            break;
        }
    }

    game.active_player = 1 - pi;
    game.active_house = None;
    game.turn += 1;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the X value from Assault(X), Hazardous(X), or SplashAttack(X).
/// Pass Keyword::Assault(0) / Keyword::Hazardous(0) / Keyword::SplashAttack(0) as tag.
fn x_keyword(keywords: &[Keyword], tag: Keyword) -> u32 {
    keywords.iter().find_map(|kw| match (kw, &tag) {
        (Keyword::Assault(x), Keyword::Assault(_)) => Some(*x),
        (Keyword::Hazardous(x), Keyword::Hazardous(_)) => Some(*x),
        (Keyword::SplashAttack(x), Keyword::SplashAttack(_)) => Some(*x),
        _ => None,
    }).unwrap_or(0)
}

/// Move all destroyed creatures to their owner's discard pile,
/// transferring any captured Aember to the opponent first.
fn destroy_dead(game: &mut GameState) {
    for pi in 0..2 {
        let dead: Vec<CardId> = game.players[pi]
            .zones
            .battleline
            .creature_ids()
            .into_iter()
            .filter(|id| game.cards.get(id).map(|c| c.is_destroyed()).unwrap_or(false))
            .collect();

        for id in dead {
            let captured = game.cards[&id].aember;
            if captured > 0 {
                game.players[1 - pi].player.aember_pool += captured;
                game.cards.get_mut(&id).unwrap().aember = 0;
            }
            game.players[pi].zones.destroy(id);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cards::{SMAAASH, TROLL, VEZYMA_THINKDRONE, SILVERTOOTH};
    use crate::deck::build_deck;

    fn two_player_game(p0: &[&'static crate::card::CardDef], p1: &[&'static crate::card::CardDef]) -> GameState {
        let (mut cards, ids0) = build_deck(p0);
        let (cards1, ids1) = build_deck(p1);
        cards.extend(cards1);
        GameState::new(ids0, ids1, cards)
    }

    #[test]
    fn test_key_cost_default() {
        let p = Player::new();
        assert_eq!(p.current_key_cost(), 6);
    }

    #[test]
    fn test_key_cost_modifier() {
        let mut p = Player::new();
        p.key_cost_modifier = -2;
        assert_eq!(p.current_key_cost(), 4);
        p.key_cost_modifier = 3;
        assert_eq!(p.current_key_cost(), 9);
        p.key_cost_modifier = -100;
        assert_eq!(p.current_key_cost(), 0); // min 0
    }

    #[test]
    fn test_step_forge_mandatory() {
        let mut p = Player::new();
        p.aember_pool = 6;
        step_forge_key(&mut p);
        assert_eq!(p.keys.forged_count(), 1);
        assert_eq!(p.aember_pool, 0);
    }

    #[test]
    fn test_step_forge_skipped() {
        let mut p = Player::new();
        p.aember_pool = 5;
        step_forge_key(&mut p);
        assert_eq!(p.keys.forged_count(), 0);
        assert_eq!(p.aember_pool, 5);
    }

    #[test]
    fn test_only_one_key_per_step() {
        let mut p = Player::new();
        p.aember_pool = 18;
        step_forge_key(&mut p);
        assert_eq!(p.keys.forged_count(), 1);
        assert_eq!(p.aember_pool, 12);
    }

    #[test]
    fn test_check_announcement() {
        let mut p = Player::new();
        p.aember_pool = 6;
        assert!(should_announce_check(&p));
        p.aember_pool = 5;
        assert!(!should_announce_check(&p));
    }

    #[test]
    fn test_forge_at_no_cost() {
        let mut p = Player::new();
        p.aember_pool = 0;
        forge_key_at_cost(&mut p, 0);
        assert_eq!(p.keys.forged_count(), 1);
        assert_eq!(p.aember_pool, 0);
    }

    #[test]
    fn test_forge_at_current_cost() {
        let mut p = Player::new();
        p.aember_pool = 4;
        p.key_cost_modifier = -2;
        let cost = p.current_key_cost();
        forge_key_at_cost(&mut p, cost);
        assert_eq!(p.keys.forged_count(), 1);
        assert_eq!(p.aember_pool, 0);
    }

    #[test]
    fn test_choose_house() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        choose_house(&mut game, House::Brobnar, false);
        assert_eq!(game.active_house, Some(House::Brobnar));
    }

    #[test]
    fn test_choose_house_picks_up_archives() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let archived_id = *game.players[0].zones.deck.last().unwrap();
        game.players[0].zones.deck.retain(|&id| id != archived_id);
        game.players[0].zones.archives.push(archived_id);
        choose_house(&mut game, House::Brobnar, true);
        assert!(game.players[0].zones.hand.contains(&archived_id));
        assert!(game.players[0].zones.archives.is_empty());
    }

    #[test]
    fn test_play_creature() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        assert_eq!(game.players[0].zones.battleline.left_flank(), Some(id));
        assert!(!game.players[0].zones.hand.contains(&id));
    }

    #[test]
    fn test_play_action_goes_to_discard() {
        use crate::cards::PLAGUE;
        let mut game = two_player_game(&[&PLAGUE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        assert!(game.players[0].zones.discard.contains(&id));
    }

    #[test]
    fn test_reap_exhausts_and_gains_aember() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        reap(&mut game, id);
        assert!(game.cards[&id].exhausted);
        assert_eq!(game.players[0].player.aember_pool, 1);
    }

    #[test]
    fn test_attack_basic_fight() {
        // Troll (5 power, 1 armor) vs Troll (5 power, 1 armor)
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        attack(&mut game, att, def);

        // Each deals 5 damage, 1 absorbed by armor -> 4 damage each; power 5, not destroyed
        assert_eq!(game.cards[&att].damage, 4);
        assert_eq!(game.cards[&def].damage, 4);
        assert!(game.cards[&att].exhausted);
    }

    #[test]
    fn test_attack_destroys_weaker_creature() {
        // Vezyma (1 power, no armor) attacked by Smaaash (3 power, Assault 2)
        let mut game = two_player_game(&[&SMAAASH], &[&VEZYMA_THINKDRONE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        attack(&mut game, att, def);

        // Assault(2) pre-damage destroys Vezyma (power 1) before fight resolves
        assert!(game.players[1].zones.battleline.is_empty());
        assert!(game.players[1].zones.discard.contains(&def));
    }

    #[test]
    fn test_attack_skirmish_no_return_damage() {
        // Silvertooth (2 power, Skirmish) vs Troll (5 power)
        let mut game = two_player_game(&[&SILVERTOOTH], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        attack(&mut game, att, def);

        assert_eq!(game.cards[&att].damage, 0); // Skirmish: no return damage
    }

    #[test]
    fn test_end_turn_readies_cards_and_draws() {
        let mut game = two_player_game(&[&TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        game.cards.get_mut(&id).unwrap().exhausted = true;

        end_turn(&mut game);

        assert!(!game.cards[&id].exhausted);
        assert_eq!(game.players[0].zones.hand.len(), 6);
        assert_eq!(game.active_player, 1);
        assert_eq!(game.turn, 2);
    }

    #[test]
    fn test_captured_aember_transfers_on_destroy() {
        let mut game = two_player_game(&[&SMAAASH], &[&VEZYMA_THINKDRONE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        // Place aember on defender to verify it transfers to opponent on death
        game.cards.get_mut(&def).unwrap().aember = 2;
        attack(&mut game, att, def);

        assert_eq!(game.players[0].player.aember_pool, 2);
    }
}
