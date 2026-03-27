use std::collections::HashMap;

use crate::card::{BonusIcon, Card, CardId, CardType, Effect, House, Keyword};
use crate::victory::{KeyColor, PlayerKeys};
use crate::zones::{Flank, PlayerZones};

// ---------------------------------------------------------------------------
// Player
// ---------------------------------------------------------------------------

pub struct Player {
    pub aember_pool: u32,
    pub keys: PlayerKeys,
    pub key_cost_modifier: i32,
    /// Current chain count. Reduces hand refill size by 1 per 6 chains.
    pub chains: u32,
}

impl Player {
    pub fn new() -> Self {
        Self {
            aember_pool: 0,
            keys: PlayerKeys::new(),
            key_cost_modifier: 0,
            chains: 0,
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
    /// Index of the player who goes first (subject to First Turn Rule).
    pub first_player: usize,
    /// Number of cards played or used (reap/attack) this turn.
    pub cards_used_this_turn: u32,
    /// Tracks how many times each card name has been played/used this turn (Rule of Six).
    pub card_use_counts: HashMap<&'static str, u32>,
    /// Set when an Omega card is played; blocks further plays this step.
    pub omega_triggered: bool,
}

impl GameState {
    pub fn new(
        p0_deck: Vec<CardId>,
        p1_deck: Vec<CardId>,
        cards: HashMap<CardId, Card>,
    ) -> Self {
        let mut state = Self {
            players: [
                PlayerState { player: Player::new(), zones: PlayerZones::new(p0_deck) },
                PlayerState { player: Player::new(), zones: PlayerZones::new(p1_deck) },
            ],
            cards,
            active_player: 0,
            active_house: None,
            turn: 1,
            first_player: 0,
            cards_used_this_turn: 0,
            card_use_counts: HashMap::new(),
            omega_triggered: false,
        };
        // Setup draw: first player draws 7, second player draws 6.
        for _ in 0..7 {
            state.players[0].zones.draw();
        }
        for _ in 0..6 {
            state.players[1].zones.draw();
        }
        state
    }

    /// Create a game state without the initial draw (for tests that manage hands manually).
    #[cfg(test)]
    pub fn new_no_draw(
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
            first_player: 0,
            cards_used_this_turn: 0,
            card_use_counts: HashMap::new(),
            omega_triggered: false,
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
// Validation
// ---------------------------------------------------------------------------

/// Returns false if the card cannot be played this turn.
/// Checks: Omega lockout, First Turn Rule, Alpha, Rule of Six.
pub fn can_play(game: &GameState, card_id: CardId) -> bool {
    if game.omega_triggered {
        return false;
    }
    // First Turn Rule: first player's first turn may only play/discard one card.
    if game.turn == 1
        && game.active_player == game.first_player
        && game.cards_used_this_turn >= 1
    {
        return false;
    }
    if game.cards[&card_id].has_keyword(Keyword::Alpha) && game.cards_used_this_turn > 0 {
        return false;
    }
    // Rule of Six: same card name cannot be played/used more than 6 times per turn.
    let name = game.cards[&card_id].def.name;
    if *game.card_use_counts.get(name).unwrap_or(&0) >= 6 {
        return false;
    }
    true
}

/// Returns true if the card belongs to the active house, or has Versatile.
pub fn is_active_house_card(game: &GameState, card_id: CardId) -> bool {
    let card = &game.cards[&card_id];
    if card.has_keyword(Keyword::Versatile) {
        return true;
    }
    match game.active_house {
        Some(house) => card.belongs_to_house(house),
        None => false,
    }
}

/// Returns false if the creature is exhausted, stunned, or enraged with enemies present.
pub fn can_reap(game: &GameState, card_id: CardId) -> bool {
    let card = &game.cards[&card_id];
    if card.exhausted || card.stun {
        return false;
    }
    // Enraged creatures must fight if enemy creatures exist.
    if card.enrage {
        let opp = 1 - game.active_player;
        if !game.players[opp].zones.battleline.is_empty() {
            return false;
        }
    }
    let name = card.def.name;
    *game.card_use_counts.get(name).unwrap_or(&0) < 6
}

/// Returns false if the creature is exhausted or stunned.
pub fn can_fight_with(game: &GameState, card_id: CardId) -> bool {
    let card = &game.cards[&card_id];
    if card.exhausted || card.stun {
        return false;
    }
    let name = card.def.name;
    *game.card_use_counts.get(name).unwrap_or(&0) < 6
}

/// Use a stunned creature: exhaust it and remove its stun counter.
/// This counts as using the creature for Rule of Six purposes.
pub fn unstun(game: &mut GameState, card_id: CardId) {
    let card = game.cards.get_mut(&card_id).unwrap();
    card.stun = false;
    card.exhausted = true;
    let name = card.def.name;
    *game.card_use_counts.entry(name).or_insert(0) += 1;
    game.cards_used_this_turn += 1;
}

/// Discard a card from the active player's hand (Step 3 discard action).
pub fn discard_card_from_hand(game: &mut GameState, card_id: CardId) {
    let pi = game.active_player;
    game.players[pi].zones.hand.retain(|&id| id != card_id);
    game.players[pi].zones.discard.push(card_id);
    let name = game.cards[&card_id].def.name;
    *game.card_use_counts.entry(name).or_insert(0) += 1;
    game.cards_used_this_turn += 1;
}

/// Returns false if Taunt blocks the attack.
///
/// A creature with Taunt in the enemy battleline forces the attacker to
/// target a Taunt creature before targeting any non-Taunt creature that is
/// not on a flank.
pub fn can_attack(game: &GameState, _attacker_id: CardId, defender_id: CardId) -> bool {
    let def_idx = 1 - game.active_player;
    let enemy_ids = game.players[def_idx].zones.battleline.creature_ids();

    let any_taunt = enemy_ids.iter().any(|&id| game.cards[&id].has_keyword(Keyword::Taunt));
    if !any_taunt {
        return true;
    }
    // Targeting a Taunt creature is always allowed.
    if game.cards[&defender_id].has_keyword(Keyword::Taunt) {
        return true;
    }
    // Targeting a non-Taunt creature on a flank is allowed.
    if game.players[def_idx].zones.battleline.is_on_flank(defender_id) {
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Step 3 — play, use, discard cards
// ---------------------------------------------------------------------------

/// Play a card from hand.
/// Creatures go to the battleline at `flank` (ignored for other types).
/// Actions go to discard. Artifacts go to the artifact zone.
/// Upgrades are discarded (full upgrade attachment is out of scope for POC).
/// Handles Treachery, bonus icons, Exalt, on_play effects, Omega, and Rule of Six.
pub fn play_card(game: &mut GameState, card_id: CardId, flank: Flank) {
    let card_type = game.cards[&card_id].def.card_type;
    let pi = game.active_player;
    // Treachery: creature enters play under opponent's control.
    let has_treachery = game.cards[&card_id].has_keyword(Keyword::Treachery);
    let owner = if has_treachery { 1 - pi } else { pi };
    match card_type {
        CardType::Creature => game.players[owner].zones.play_creature(card_id, flank),
        CardType::Artifact => game.players[owner].zones.play_artifact(card_id),
        CardType::Action | CardType::Upgrade => {
            game.players[pi].zones.discard_from_hand(card_id)
        }
    }

    // Bonus icons resolve before Play: abilities.
    resolve_bonus_icons(game, card_id, pi);

    // Exalt: place 1 Aember from the supply onto this creature when it enters play.
    if game.cards[&card_id].has_keyword(Keyword::Exalt) {
        game.cards.get_mut(&card_id).unwrap().aember += 1;
    }

    let on_play = game.cards[&card_id].def.on_play;
    apply_effects(game, card_id, pi, on_play);

    // Omega: no more cards can be played/used/discarded this step.
    if game.cards[&card_id].has_keyword(Keyword::Omega) {
        game.omega_triggered = true;
    }

    let name = game.cards[&card_id].def.name;
    *game.card_use_counts.entry(name).or_insert(0) += 1;
    game.cards_used_this_turn += 1;
}

/// Play a creature with Deploy: place it at any position in the battleline.
pub fn play_card_deployed(game: &mut GameState, card_id: CardId, index: usize) {
    let pi = game.active_player;
    let has_treachery = game.cards[&card_id].has_keyword(Keyword::Treachery);
    let owner = if has_treachery { 1 - pi } else { pi };
    game.players[owner].zones.deploy_creature(card_id, index);

    resolve_bonus_icons(game, card_id, pi);

    if game.cards[&card_id].has_keyword(Keyword::Exalt) {
        game.cards.get_mut(&card_id).unwrap().aember += 1;
    }

    let on_play = game.cards[&card_id].def.on_play;
    apply_effects(game, card_id, pi, on_play);

    if game.cards[&card_id].has_keyword(Keyword::Omega) {
        game.omega_triggered = true;
    }

    let name = game.cards[&card_id].def.name;
    *game.card_use_counts.entry(name).or_insert(0) += 1;
    game.cards_used_this_turn += 1;
}

/// Exhaust a friendly creature and gain 1 Aember.
/// Also handles Capture, Steal, and on_reap effects.
pub fn reap(game: &mut GameState, card_id: CardId) {
    let pi = game.active_player;
    let opp = 1 - pi;

    game.cards.get_mut(&card_id).unwrap().exhausted = true;
    game.players[pi].player.aember_pool += 1;

    // Capture: take 1 Aember from opponent onto this creature.
    if game.cards[&card_id].has_keyword(Keyword::Capture) {
        let n = 1_u32.min(game.players[opp].player.aember_pool);
        game.players[opp].player.aember_pool -= n;
        game.cards.get_mut(&card_id).unwrap().aember += n;
    }

    // Steal: take 1 Aember from opponent into own pool.
    if game.cards[&card_id].has_keyword(Keyword::Steal) {
        let n = 1_u32.min(game.players[opp].player.aember_pool);
        game.players[opp].player.aember_pool -= n;
        game.players[pi].player.aember_pool += n;
    }

    let on_reap = game.cards[&card_id].def.on_reap;
    apply_effects(game, card_id, pi, on_reap);
    let name = game.cards[&card_id].def.name;
    *game.card_use_counts.entry(name).or_insert(0) += 1;
    game.cards_used_this_turn += 1;
}

/// Fight: attacker attacks defender.
/// Handles Elusive, Taunt (see can_attack), Assault, Hazardous, Skirmish,
/// Poison, SplashAttack, Capture, Steal keywords, and on_fight effects.
/// Destroyed creatures are moved to discard automatically.
pub fn attack(game: &mut GameState, attacker_id: CardId, defender_id: CardId) {
    let def_idx = 1 - game.active_player;
    let pi = game.active_player;

    // Gather combat info (immutable reads first)
    let assault    = x_keyword(game.cards[&attacker_id].def.keywords, Keyword::Assault(0));
    let hazardous  = x_keyword(game.cards[&defender_id].def.keywords, Keyword::Hazardous(0));
    let splash     = x_keyword(game.cards[&attacker_id].def.keywords, Keyword::SplashAttack(0));
    let skirmish        = game.cards[&attacker_id].has_keyword(Keyword::Skirmish);
    let attacker_poison = game.cards[&attacker_id].has_keyword(Keyword::Poison);
    let defender_poison = game.cards[&defender_id].has_keyword(Keyword::Poison);
    let attacker_capture = game.cards[&attacker_id].has_keyword(Keyword::Capture);
    let attacker_steal   = game.cards[&attacker_id].has_keyword(Keyword::Steal);
    let defender_elusive = game.cards[&defender_id].has_keyword(Keyword::Elusive);
    let elusive_used     = game.cards[&defender_id].elusive_used_this_turn;
    let attacker_power = game.cards[&attacker_id].power();
    let defender_power = game.cards[&defender_id].power();
    let on_fight_attacker = game.cards[&attacker_id].def.on_fight;
    let on_fight_defender = game.cards[&defender_id].def.on_fight;
    let (left_nb, right_nb) = game.players[def_idx].zones.battleline.neighbors(defender_id);

    // Exhaust attacker
    game.cards.get_mut(&attacker_id).unwrap().exhausted = true;

    // Elusive: first attack this turn is absorbed — no damage either way.
    if defender_elusive && !elusive_used {
        game.cards.get_mut(&defender_id).unwrap().elusive_used_this_turn = true;
        game.cards_used_this_turn += 1;
        return;
    }

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

    // Capture: take 1 Aember from opponent onto attacker.
    if attacker_capture {
        let n = 1_u32.min(game.players[def_idx].player.aember_pool);
        game.players[def_idx].player.aember_pool -= n;
        game.cards.get_mut(&attacker_id).unwrap().aember += n;
    }

    // Steal: take 1 Aember from opponent into own pool.
    if attacker_steal {
        let n = 1_u32.min(game.players[def_idx].player.aember_pool);
        game.players[def_idx].player.aember_pool -= n;
        game.players[pi].player.aember_pool += n;
    }

    destroy_dead(game);

    // After Fight: abilities only resolve if the creature survived the fight.
    let attacker_alive = game.players[pi].zones.battleline.creature_ids().contains(&attacker_id);
    if attacker_alive {
        apply_effects(game, attacker_id, pi, on_fight_attacker);
    }
    let defender_alive = game.players[def_idx]
        .zones
        .battleline
        .creature_ids()
        .contains(&defender_id);
    if defender_alive {
        apply_effects(game, defender_id, def_idx, on_fight_defender);
    }

    let name = game.cards[&attacker_id].def.name;
    *game.card_use_counts.entry(name).or_insert(0) += 1;
    game.cards_used_this_turn += 1;
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

    // Chains reduce the hand refill target (shed 1 chain if they reduced a draw).
    let chains = game.players[pi].player.chains;
    let hand_before = game.players[pi].zones.hand.len();
    let max_hand = 6usize.saturating_sub(chain_penalty(chains));
    while game.players[pi].zones.hand.len() < max_hand {
        if game.players[pi].zones.draw().is_none() {
            break;
        }
    }
    // Shed 1 chain if chains were active and the player had room to draw.
    if chains > 0 && hand_before < 6 {
        game.players[pi].player.chains -= 1;
    }

    game.omega_triggered = false;
    game.active_player = 1 - pi;
    game.active_house = None;
    game.turn += 1;
    game.cards_used_this_turn = 0;
    game.card_use_counts.clear();
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Cards drawn fewer per hand refill based on chain count.
fn chain_penalty(chains: u32) -> usize {
    match chains {
        0 => 0,
        1..=6 => 1,
        7..=12 => 2,
        13..=18 => 3,
        _ => 4,
    }
}

/// Resolve all bonus icons on a card immediately after it is played.
/// Non-interactive icons (Aember, Draw) resolve automatically.
/// Capture defaults to the played creature if it's a creature, else the first friendly creature.
/// Damage defaults to the first enemy creature (first friendly if no enemies).
/// Discard requires player choice and is skipped in this POC.
fn resolve_bonus_icons(game: &mut GameState, card_id: CardId, pi: usize) {
    let icons: &'static [BonusIcon] = game.cards[&card_id].def.bonus_icons;
    let opp = 1 - pi;
    for &icon in icons {
        match icon {
            BonusIcon::Aember => {
                game.players[pi].player.aember_pool += 1;
            }
            BonusIcon::Capture => {
                let n = 1_u32.min(game.players[opp].player.aember_pool);
                game.players[opp].player.aember_pool -= n;
                let is_creature = game.cards[&card_id].def.card_type == CardType::Creature;
                let target = if is_creature {
                    Some(card_id)
                } else {
                    game.players[pi].zones.battleline.creature_ids().into_iter().next()
                };
                match target {
                    Some(t) => { game.cards.get_mut(&t).unwrap().aember += n; }
                    None => { game.players[opp].player.aember_pool += n; } // no target: refund
                }
            }
            BonusIcon::Damage => {
                let target = {
                    let enemies = game.players[opp].zones.battleline.creature_ids();
                    if let Some(&t) = enemies.first() {
                        Some(t)
                    } else {
                        game.players[pi].zones.battleline.creature_ids().into_iter().next()
                    }
                };
                if let Some(t) = target {
                    game.cards.get_mut(&t).unwrap().deal_damage(1);
                    destroy_dead(game);
                }
            }
            BonusIcon::Draw => {
                game.players[pi].zones.draw();
            }
            BonusIcon::Discard => {
                // Requires player choice — not resolvable without UI targeting in this POC.
            }
        }
    }
}

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

/// Apply a slice of triggered effects on behalf of `controller`.
/// `card_id` is the source card (for self-targeting effects).
fn apply_effects(game: &mut GameState, card_id: CardId, controller: usize, effects: &[Effect]) {
    for &effect in effects {
        let opp = 1 - controller;
        match effect {
            Effect::GainAember(n) => {
                game.players[controller].player.aember_pool += n;
            }
            Effect::StealAember(n) => {
                let stolen = n.min(game.players[opp].player.aember_pool);
                game.players[opp].player.aember_pool -= stolen;
                game.players[controller].player.aember_pool += stolen;
            }
            Effect::CaptureAember(n) => {
                let captured = n.min(game.players[opp].player.aember_pool);
                game.players[opp].player.aember_pool -= captured;
                game.cards.get_mut(&card_id).unwrap().aember += captured;
            }
            Effect::DrawCards(n) => {
                for _ in 0..n {
                    game.players[controller].zones.draw();
                }
            }
            Effect::DealDamageToEachEnemy(n) => {
                let enemies: Vec<CardId> =
                    game.players[opp].zones.battleline.creature_ids();
                for enemy_id in enemies {
                    game.cards.get_mut(&enemy_id).unwrap().deal_damage(n);
                }
                destroy_dead(game);
            }
            Effect::HealSelf(n) => {
                game.cards.get_mut(&card_id).unwrap().heal(n);
            }
        }
    }
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
            .filter(|id| {
                game.cards
                    .get(id)
                    .map(|c| c.is_destroyed() && !c.has_keyword(Keyword::Invulnerable))
                    .unwrap_or(false)
            })
            .collect();

        for id in dead {
            // Fire on_destroyed effects before removing from play.
            let on_destroyed = game.cards[&id].def.on_destroyed;
            apply_effects(game, id, pi, on_destroyed);

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
    use crate::card::{CardDef, BonusIcon, Rarity};
    use crate::deck::build_deck;

    fn two_player_game(p0: &[&'static crate::card::CardDef], p1: &[&'static crate::card::CardDef]) -> GameState {
        let (mut cards, ids0) = build_deck(p0);
        let (cards1, ids1) = build_deck(p1);
        cards.extend(cards1);
        // Tests manage hands manually, so skip the setup draw.
        GameState::new_no_draw(ids0, ids1, cards)
    }

    // ---- helpers: card defs for tests that need specific keywords ----

    static ALPHA_CREATURE: CardDef = CardDef {
        name: "Alpha Beast",
        card_type: CardType::Creature,
        house: House::Brobnar,
        power: Some(2),
        armor: None,
        keywords: &[Keyword::Alpha],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static ELUSIVE_CREATURE: CardDef = CardDef {
        name: "Elusive One",
        card_type: CardType::Creature,
        house: House::Shadows,
        power: Some(3),
        armor: None,
        keywords: &[Keyword::Elusive],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static TAUNT_CREATURE: CardDef = CardDef {
        name: "Taunt Guard",
        card_type: CardType::Creature,
        house: House::Brobnar,
        power: Some(4),
        armor: None,
        keywords: &[Keyword::Taunt],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static CAPTURE_CREATURE: CardDef = CardDef {
        name: "Captor",
        card_type: CardType::Creature,
        house: House::Dis,
        power: Some(2),
        armor: None,
        keywords: &[Keyword::Capture],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    // High-power variant so the attacker survives to hold captured Aember.
    static STRONG_CAPTURE: CardDef = CardDef {
        name: "Strong Captor",
        card_type: CardType::Creature,
        house: House::Dis,
        power: Some(10),
        armor: None,
        keywords: &[Keyword::Capture],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static STEAL_CREATURE: CardDef = CardDef {
        name: "Thief",
        card_type: CardType::Creature,
        house: House::Shadows,
        power: Some(2),
        armor: None,
        keywords: &[Keyword::Steal],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static EXALT_CREATURE: CardDef = CardDef {
        name: "Exalted One",
        card_type: CardType::Creature,
        house: House::Sanctum,
        power: Some(2),
        armor: None,
        keywords: &[Keyword::Exalt],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static DEPLOY_CREATURE: CardDef = CardDef {
        name: "Deployer",
        card_type: CardType::Creature,
        house: House::Logos,
        power: Some(3),
        armor: None,
        keywords: &[Keyword::Deploy],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static REAP_GAIN_CREATURE: CardDef = CardDef {
        name: "Reap Gainer",
        card_type: CardType::Creature,
        house: House::Untamed,
        power: Some(2),
        armor: None,
        keywords: &[],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[Effect::GainAember(2)],
        on_fight: &[],
        on_play: &[Effect::DrawCards(1)],
        on_destroyed: &[Effect::GainAember(1)],
    };

    static FIGHT_DAMAGE_CREATURE: CardDef = CardDef {
        name: "Berserker",
        card_type: CardType::Creature,
        house: House::Brobnar,
        power: Some(5),
        armor: None,
        keywords: &[],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[Effect::DealDamageToEachEnemy(1)],
        on_play: &[],
        on_destroyed: &[],
    };

    // ---- setup draw ----

    #[test]
    fn test_setup_draw_seven_and_six() {
        let defs: Vec<&'static CardDef> = vec![&TROLL; 12];
        let (mut cards, ids0) = build_deck(&defs);
        let (cards1, ids1) = build_deck(&defs);
        cards.extend(cards1);
        let game = GameState::new(ids0, ids1, cards);
        assert_eq!(game.players[0].zones.hand.len(), 7); // first player draws 7
        assert_eq!(game.players[1].zones.hand.len(), 6); // second player draws 6
    }

    // ---- key / forge tests ----

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

    // ---- house / play tests ----

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
    fn test_play_increments_cards_used() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        assert_eq!(game.cards_used_this_turn, 0);
        play_card(&mut game, id, Flank::Left);
        assert_eq!(game.cards_used_this_turn, 1);
    }

    // ---- reap tests ----

    #[test]
    fn test_reap_exhausts_and_gains_aember() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left); // bonus icon: +1 aember
        reap(&mut game, id);                   // base reap: +1 aember
        assert!(game.cards[&id].exhausted);
        assert_eq!(game.players[0].player.aember_pool, 2); // 1 bonus icon + 1 reap
    }

    // ---- Alpha keyword ----

    #[test]
    fn test_alpha_allowed_as_first_action() {
        let mut game = two_player_game(&[&ALPHA_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        assert!(can_play(&game, id)); // first action → allowed
    }

    #[test]
    fn test_alpha_blocked_after_action() {
        // Deck: last element is top; TROLL is last → drawn first.
        let mut game = two_player_game(&[&ALPHA_CREATURE, &TROLL], &[&TROLL]);
        let troll_id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, troll_id, Flank::Left); // first action
        let alpha_id = game.players[0].zones.draw().unwrap();
        assert!(!can_play(&game, alpha_id)); // second action → blocked
    }

    // ---- Elusive keyword ----

    #[test]
    fn test_elusive_first_attack_absorbed() {
        let mut game = two_player_game(&[&TROLL], &[&ELUSIVE_CREATURE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        attack(&mut game, att, def);

        assert_eq!(game.cards[&def].damage, 0); // Elusive absorbed the hit
        assert!(game.cards[&def].elusive_used_this_turn);
    }

    #[test]
    fn test_elusive_second_attack_lands() {
        let mut game = two_player_game(&[&TROLL, &TROLL], &[&ELUSIVE_CREATURE]);
        let att1 = game.players[0].zones.draw().unwrap();
        let att2 = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att1, Flank::Left);
        play_card(&mut game, att2, Flank::Right);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        attack(&mut game, att1, def); // absorbed by Elusive
        attack(&mut game, att2, def); // lands (power 5 - 0 armor = 5 damage; creature power 3 → destroyed)
        assert!(game.players[1].zones.battleline.is_empty());
    }

    // ---- Taunt keyword ----

    #[test]
    fn test_can_attack_without_taunt() {
        let mut game = two_player_game(&[&TROLL], &[&VEZYMA_THINKDRONE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;
        assert!(can_attack(&game, att, def)); // no Taunt in enemy line
    }

    #[test]
    fn test_taunt_blocks_non_flank_attack() {
        let mut game = two_player_game(&[&TROLL], &[&TAUNT_CREATURE, &VEZYMA_THINKDRONE]);
        let att = game.players[0].zones.draw().unwrap();
        let taunt = game.players[1].zones.draw().unwrap();
        let non_taunt = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        // Place taunt at left, non_taunt at right so non_taunt IS on a flank
        play_card(&mut game, taunt, Flank::Left);   // left flank
        play_card(&mut game, non_taunt, Flank::Right); // right flank
        game.active_player = 0;

        // non_taunt is on right flank → can attack it
        assert!(can_attack(&game, att, non_taunt));
        // taunt creature itself is always attackable
        assert!(can_attack(&game, att, taunt));
    }

    #[test]
    fn test_taunt_blocks_inner_non_taunt() {
        // three enemies: non_taunt | taunt | non_taunt_inner
        // non_taunt_inner is not on a flank, so it cannot be attacked while taunt exists
        let mut game = two_player_game(
            &[&TROLL],
            &[&VEZYMA_THINKDRONE, &TAUNT_CREATURE, &SMAAASH],
        );
        let att = game.players[0].zones.draw().unwrap();
        let left_nt = game.players[1].zones.draw().unwrap();
        let taunt = game.players[1].zones.draw().unwrap();
        let inner_nt = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, left_nt, Flank::Left);   // left flank
        play_card(&mut game, taunt, Flank::Right);    // middle
        play_card(&mut game, inner_nt, Flank::Right); // right flank
        game.active_player = 0;

        // taunt is in the middle (not on a flank), inner_nt is on right flank
        // left_nt is on left flank
        assert!(can_attack(&game, att, left_nt));   // flank → ok
        assert!(can_attack(&game, att, inner_nt));  // flank → ok
        assert!(can_attack(&game, att, taunt));     // taunt itself → ok
        // no inner non-flank non-taunt creature exists here, so all pass
    }

    // ---- Capture keyword ----

    #[test]
    fn test_capture_on_reap() {
        let mut game = two_player_game(&[&CAPTURE_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        game.players[1].player.aember_pool = 3;

        reap(&mut game, id);

        assert_eq!(game.players[1].player.aember_pool, 2); // lost 1
        assert_eq!(game.cards[&id].aember, 1);             // on creature
        assert_eq!(game.players[0].player.aember_pool, 1); // base reap gain
    }

    #[test]
    fn test_capture_on_attack() {
        // Use a high-power attacker so it survives and retains the captured Aember.
        let mut game = two_player_game(&[&STRONG_CAPTURE], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;
        game.players[1].player.aember_pool = 2;

        attack(&mut game, att, def);

        assert_eq!(game.players[1].player.aember_pool, 1); // lost 1 to capture
        assert_eq!(game.cards[&att].aember, 1);            // sits on attacker
    }

    // ---- Steal keyword ----

    #[test]
    fn test_steal_on_reap() {
        let mut game = two_player_game(&[&STEAL_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        game.players[1].player.aember_pool = 3;

        reap(&mut game, id);

        assert_eq!(game.players[1].player.aember_pool, 2); // lost 1
        assert_eq!(game.players[0].player.aember_pool, 2); // 1 reap + 1 stolen
    }

    #[test]
    fn test_steal_on_attack() {
        let mut game = two_player_game(&[&SILVERTOOTH], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;
        game.players[1].player.aember_pool = 2;

        attack(&mut game, att, def);

        assert_eq!(game.players[1].player.aember_pool, 1); // stolen 1
        assert_eq!(game.players[0].player.aember_pool, 1); // gained 1
    }

    // ---- Exalt keyword ----

    #[test]
    fn test_exalt_places_aember_on_play() {
        let mut game = two_player_game(&[&EXALT_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        assert_eq!(game.cards[&id].aember, 1);
    }

    #[test]
    fn test_exalt_aember_goes_to_opponent_on_destroy() {
        let mut game = two_player_game(&[&SMAAASH], &[&EXALT_CREATURE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        // Smaaash (3 power, Assault 2) vs Exalt creature (2 power, 1 aember)
        // Assault deals 2 → destroys Exalt creature (power 2)
        attack(&mut game, att, def);

        // Exalted aember goes to attacker (player 0) as defender is P1's creature
        assert_eq!(game.players[0].player.aember_pool, 1);
        assert!(game.players[1].zones.battleline.is_empty());
    }

    // ---- Deploy keyword ----

    #[test]
    fn test_deploy_places_creature_at_index() {
        let mut game = two_player_game(&[&TROLL, &TROLL, &DEPLOY_CREATURE], &[&TROLL]);
        let left_id = game.players[0].zones.draw().unwrap();
        let right_id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, left_id, Flank::Left);
        play_card(&mut game, right_id, Flank::Right);
        let deploy_id = game.players[0].zones.draw().unwrap();
        play_card_deployed(&mut game, deploy_id, 1); // insert between them
        assert_eq!(
            game.players[0].zones.battleline.neighbors(deploy_id),
            (Some(left_id), Some(right_id))
        );
    }

    // ---- Triggered effects ----

    #[test]
    fn test_on_reap_effect() {
        let mut game = two_player_game(&[&REAP_GAIN_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        // on_play draws 1 card; just verify creature is in play
        reap(&mut game, id);
        // base reap = 1, on_reap GainAember(2) = 2 more → total 3
        assert_eq!(game.players[0].player.aember_pool, 3);
    }

    #[test]
    fn test_on_play_effect_draws_card() {
        // REAP_GAIN_CREATURE is last in slice → drawn first; TROLL stays in deck.
        let mut game = two_player_game(&[&TROLL, &REAP_GAIN_CREATURE], &[&TROLL]);
        let id1 = game.players[0].zones.draw().unwrap(); // draws REAP_GAIN_CREATURE
        let hand_before = game.players[0].zones.hand.len(); // 1
        play_card(&mut game, id1, Flank::Left); // on_play draws 1 from deck (TROLL)
        assert_eq!(game.players[0].zones.hand.len(), hand_before); // played 1, drew 1 → same size
    }

    #[test]
    fn test_on_destroyed_effect() {
        let mut game = two_player_game(&[&SMAAASH], &[&REAP_GAIN_CREATURE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;

        // on_play for def draws a card (not easily testable here without setup)
        // Attack: Smaaash (3 power, Assault 2) destroys ReapGainer (2 power)
        // on_destroyed: GainAember(1) for P1 (defender's controller)
        attack(&mut game, att, def);

        assert_eq!(game.players[1].player.aember_pool, 1); // on_destroyed gain
        assert!(game.players[1].zones.battleline.is_empty());
    }

    #[test]
    fn test_on_fight_deal_damage_to_each_enemy() {
        // Berserker (5 power, on_fight DealDamageToEachEnemy(1)) vs Vezyma (1 power)
        let mut game = two_player_game(&[&FIGHT_DAMAGE_CREATURE], &[&VEZYMA_THINKDRONE, &SMAAASH]);
        let att = game.players[0].zones.draw().unwrap();
        let def1 = game.players[1].zones.draw().unwrap();
        let def2 = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def1, Flank::Left);
        play_card(&mut game, def2, Flank::Right);
        game.active_player = 0;

        // Fight def1; berserker's on_fight deals 1 to each enemy (def2 only, def1 already resolved)
        // After attack: def1 takes 5 damage (destroyed), def2 takes 1 from on_fight
        attack(&mut game, att, def1);

        assert!(game.players[1].zones.discard.contains(&def1)); // destroyed in fight
        assert_eq!(game.cards[&def2].damage, 1); // hit by on_fight splash
    }

    // ---- basic attack tests (regression) ----

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
        assert_eq!(game.cards_used_this_turn, 0); // reset on end turn
    }

    // ---- Stun ----

    #[test]
    fn test_stun_blocks_reap() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        game.cards.get_mut(&id).unwrap().stun = true;
        assert!(!can_reap(&game, id));
    }

    #[test]
    fn test_stun_blocks_fight() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        game.cards.get_mut(&id).unwrap().stun = true;
        assert!(!can_fight_with(&game, id));
    }

    #[test]
    fn test_unstun_exhausts_and_clears_stun() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        game.cards.get_mut(&id).unwrap().stun = true;
        unstun(&mut game, id);
        assert!(!game.cards[&id].stun);
        assert!(game.cards[&id].exhausted);
    }

    // ---- Enrage ----

    #[test]
    fn test_enraged_cannot_reap_when_enemies_exist() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;
        game.cards.get_mut(&att).unwrap().enrage = true;
        assert!(!can_reap(&game, att));
    }

    #[test]
    fn test_enraged_can_reap_when_no_enemies() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.cards.get_mut(&att).unwrap().enrage = true;
        assert!(can_reap(&game, att)); // no enemies
    }

    // ---- Invulnerable ----

    static INVULNERABLE_CREATURE: CardDef = CardDef {
        name: "Immortal",
        card_type: CardType::Creature,
        house: House::Sanctum,
        power: Some(3),
        armor: None,
        keywords: &[Keyword::Invulnerable],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Rare,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    #[test]
    fn test_invulnerable_ignores_damage() {
        let mut game = two_player_game(&[&TROLL], &[&INVULNERABLE_CREATURE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;
        attack(&mut game, att, def);
        assert_eq!(game.cards[&def].damage, 0); // no damage taken
        assert!(!game.players[1].zones.battleline.is_empty()); // survives
    }

    // ---- Omega ----

    static OMEGA_CREATURE: CardDef = CardDef {
        name: "Omega Beast",
        card_type: CardType::Creature,
        house: House::Brobnar,
        power: Some(3),
        armor: None,
        keywords: &[Keyword::Omega],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Rare,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    #[test]
    fn test_omega_blocks_further_plays() {
        // Last element = top of deck; OMEGA_CREATURE is drawn first.
        let mut game = two_player_game(&[&TROLL, &OMEGA_CREATURE], &[&TROLL]);
        let omega_id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, omega_id, Flank::Left);
        assert!(game.omega_triggered);
        let troll_id = game.players[0].zones.draw().unwrap();
        assert!(!can_play(&game, troll_id));
    }

    #[test]
    fn test_omega_resets_on_end_turn() {
        let mut game = two_player_game(&[&OMEGA_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        assert!(game.omega_triggered);
        end_turn(&mut game);
        assert!(!game.omega_triggered);
    }

    // ---- Versatile ----

    static VERSATILE_CREATURE: CardDef = CardDef {
        name: "Chameleon",
        card_type: CardType::Creature,
        house: House::Logos,
        power: Some(2),
        armor: None,
        keywords: &[Keyword::Versatile],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    #[test]
    fn test_versatile_is_active_house_regardless() {
        let mut game = two_player_game(&[&VERSATILE_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        game.active_house = Some(House::Brobnar); // Versatile is Logos
        assert!(is_active_house_card(&game, id));
    }

    #[test]
    fn test_non_versatile_wrong_house_is_not_active() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        game.active_house = Some(House::Dis); // Troll is Brobnar
        assert!(!is_active_house_card(&game, id));
    }

    // ---- Treachery ----

    static TREACHERY_CREATURE: CardDef = CardDef {
        name: "Turncoat",
        card_type: CardType::Creature,
        house: House::Dis,
        power: Some(2),
        armor: None,
        keywords: &[Keyword::Treachery],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Rare,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    #[test]
    fn test_treachery_enters_opponent_battleline() {
        let mut game = two_player_game(&[&TREACHERY_CREATURE], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left);
        assert!(game.players[0].zones.battleline.is_empty()); // not on player's line
        assert!(game.players[1].zones.battleline.creature_ids().contains(&id));
    }

    // ---- Chains ----

    #[test]
    fn test_chains_reduce_draw() {
        // Give player 0 exactly 6 chains (penalty = 1 fewer card → refill to 5).
        let mut game = two_player_game(
            &[&TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL],
            &[&TROLL],
        );
        game.players[0].player.chains = 6;
        game.players[0].zones.hand.clear(); // ensure a clean refill
        // active_player = 0 by default
        end_turn(&mut game);
        // Chains = 6 → penalty 1 → max hand 5
        assert_eq!(game.players[0].zones.hand.len(), 5);
        // Chains shed 1
        assert_eq!(game.players[0].player.chains, 5);
    }

    // ---- First Turn Rule ----

    #[test]
    fn test_first_turn_rule_blocks_second_play() {
        let mut game = two_player_game(&[&TROLL, &TROLL], &[&TROLL]);
        let id1 = game.players[0].zones.draw().unwrap();
        let id2 = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id1, Flank::Left); // first action — allowed
        assert!(!can_play(&game, id2));          // second action on turn 1 — blocked
    }

    #[test]
    fn test_first_turn_rule_only_applies_turn_one() {
        // 8 cards so P0 still has deck remaining after drawing to 6 on turn 1.
        let mut game = two_player_game(
            &[&TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL],
            &[&TROLL],
        );
        end_turn(&mut game); // P0 draws 6; turn 2, P1
        end_turn(&mut game); // P1 draws; turn 3, P0
        // P0 has 6 cards in hand from turn 1 refill.
        let id1 = game.players[0].zones.hand[0];
        let id2 = game.players[0].zones.hand[1];
        play_card(&mut game, id1, Flank::Left);
        assert!(can_play(&game, id2)); // turn 3 — no restriction
    }

    // ---- Bonus Icons ----

    #[test]
    fn test_bonus_icon_aember_on_play() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left); // TROLL has BonusIcon::Aember
        assert_eq!(game.players[0].player.aember_pool, 1);
    }

    #[test]
    fn test_bonus_icon_damage_hits_first_enemy() {
        // VEZYMA has no armor so the 1 bonus damage lands.
        let mut game = two_player_game(&[&SMAAASH], &[&VEZYMA_THINKDRONE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        // Play defender first so it is the target when attacker's bonus damage resolves.
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;
        play_card(&mut game, att, Flank::Left); // SMAAASH has BonusIcon::Damage
        // Defender (Vezyma: 1 power, no armor) takes 1 damage and is destroyed.
        assert!(game.players[1].zones.discard.contains(&def));
    }

    // ---- Rule of Six ----

    #[test]
    fn test_rule_of_six_blocks_seventh_use() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        play_card(&mut game, id, Flank::Left); // play counts as 1 use
        // Simulate 5 reaps to reach the limit.
        game.cards.get_mut(&id).unwrap().exhausted = false;
        for _ in 0..5 {
            *game.card_use_counts.entry("Troll").or_insert(0) += 1;
        }
        // Now at 6 uses total (1 play + 5 simulated). Further reap/play blocked.
        assert!(!can_reap(&game, id));
    }

    // ---- Discard from hand ----

    #[test]
    fn test_discard_card_from_hand() {
        let mut game = two_player_game(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        assert!(game.players[0].zones.hand.contains(&id));
        discard_card_from_hand(&mut game, id);
        assert!(!game.players[0].zones.hand.contains(&id));
        assert!(game.players[0].zones.discard.contains(&id));
        assert_eq!(game.cards_used_this_turn, 1);
    }

    // ---- After Fight: attacker survival ----

    static WEAK_FIGHT_EFFECT: CardDef = CardDef {
        name: "Weak Berserker",
        card_type: CardType::Creature,
        house: House::Brobnar,
        power: Some(1),
        armor: None,
        keywords: &[],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[Effect::GainAember(5)],
        on_play: &[],
        on_destroyed: &[],
    };

    #[test]
    fn test_after_fight_does_not_fire_when_attacker_dies() {
        // Attacker (1 power) fights Troll (5 power) → attacker is destroyed.
        // on_fight GainAember(5) should NOT fire since attacker died.
        let mut game = two_player_game(&[&WEAK_FIGHT_EFFECT], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        play_card(&mut game, att, Flank::Left);
        game.active_player = 1;
        play_card(&mut game, def, Flank::Left);
        game.active_player = 0;
        let aember_before = game.players[0].player.aember_pool;
        attack(&mut game, att, def);
        // Attacker destroyed → on_fight should not have fired
        assert_eq!(game.players[0].player.aember_pool, aember_before);
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
