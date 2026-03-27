use crate::card::{Card, CardId};
use crate::game::GameState;
use crate::protocol::{CardView, ClientGameView, KeyView, PlayerView};

fn card_view(card: &Card) -> CardView {
    CardView {
        id: card.id,
        name: card.def.name.to_string(),
        card_type: card.def.card_type,
        house: card.def.house,
        power: card.def.power,
        armor: card.def.armor,
        keywords: card.def.keywords.to_vec(),
        bonus_icons: card.def.bonus_icons.to_vec(),
        traits: card.def.traits.iter().map(|s| s.to_string()).collect(),
        rarity: card.def.rarity,
        on_reap: card.def.on_reap.to_vec(),
        on_fight: card.def.on_fight.to_vec(),
        on_play: card.def.on_play.to_vec(),
        on_destroyed: card.def.on_destroyed.to_vec(),
        exhausted: card.exhausted,
        damage: card.damage,
        aember: card.aember,
        stun: card.stun,
        ward: card.ward,
        enrage: card.enrage,
        power_counters: card.power_counters,
        armor_bonus: card.armor_bonus,
    }
}

fn card_views(game: &GameState, ids: &[CardId]) -> Vec<CardView> {
    ids.iter().map(|id| card_view(&game.cards[id])).collect()
}

fn player_view(game: &GameState, player_idx: usize) -> PlayerView {
    let p = &game.players[player_idx].player;
    PlayerView {
        aember_pool: p.aember_pool,
        keys: p
            .keys
            .keys
            .iter()
            .map(|k| KeyView { color: k.color, forged: k.forged })
            .collect(),
        chains: p.chains,
    }
}

pub fn to_client_view(game: &GameState, my_idx: usize) -> ClientGameView {
    let opp_idx = 1 - my_idx;
    let my = &game.players[my_idx];
    let opp = &game.players[opp_idx];

    ClientGameView {
        my_index: my_idx,
        active_player: game.active_player,
        active_house: game.active_house,
        turn: game.turn,

        my_player: player_view(game, my_idx),
        my_hand: card_views(game, &my.zones.hand),
        my_battleline: card_views(game, &my.zones.battleline.creature_ids()),
        my_artifacts: card_views(game, &my.zones.artifacts),
        my_discard: card_views(game, &my.zones.discard),
        my_archives: card_views(game, &my.zones.archives),
        my_deck_count: my.zones.deck.len(),

        opp_player: player_view(game, opp_idx),
        opp_hand_count: opp.zones.hand.len(),
        opp_battleline: card_views(game, &opp.zones.battleline.creature_ids()),
        opp_artifacts: card_views(game, &opp.zones.artifacts),
        opp_discard: card_views(game, &opp.zones.discard),
        opp_archives_count: opp.zones.archives.len(),
        opp_deck_count: opp.zones.deck.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{CardDef, CardType, House, Rarity};
    use crate::game::GameState;
    use std::collections::HashMap;

    static DEF_A: CardDef = CardDef {
        name: "Alpha",
        card_type: CardType::Creature,
        house: House::Brobnar,
        power: Some(3),
        armor: None,
        keywords: &[],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    static DEF_B: CardDef = CardDef {
        name: "Beta",
        card_type: CardType::Creature,
        house: House::Dis,
        power: Some(2),
        armor: Some(1),
        keywords: &[],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
    };

    fn test_game() -> GameState {
        use crate::card::Card;
        use crate::zones::Flank;

        let mut cards = HashMap::new();
        // P0 cards: 1,2,3 in hand; 4 on battleline
        for id in 1..=4 {
            cards.insert(id, Card::new(id, &DEF_A));
        }
        // P1 cards: 5,6 in hand; 7 on battleline; 8 in archives
        for id in 5..=8 {
            cards.insert(id, Card::new(id, &DEF_B));
        }

        let mut game = GameState::new_no_draw(vec![], vec![], cards);
        game.players[0].zones.hand = vec![1, 2, 3];
        game.players[0].zones.battleline.add(4, Flank::Left);
        game.players[1].zones.hand = vec![5, 6];
        game.players[1].zones.battleline.add(7, Flank::Left);
        game.players[1].zones.archives = vec![8];
        game.players[0].player.aember_pool = 4;
        game.players[1].player.aember_pool = 2;
        game
    }

    #[test]
    fn test_own_hand_visible() {
        let game = test_game();
        let view = to_client_view(&game, 0);
        assert_eq!(view.my_hand.len(), 3);
        assert_eq!(view.my_hand[0].name, "Alpha");
    }

    #[test]
    fn test_opponent_hand_hidden() {
        let game = test_game();
        let view = to_client_view(&game, 0);
        assert_eq!(view.opp_hand_count, 2);
    }

    #[test]
    fn test_battlelines_visible() {
        let game = test_game();
        let view = to_client_view(&game, 0);
        assert_eq!(view.my_battleline.len(), 1);
        assert_eq!(view.my_battleline[0].id, 4);
        assert_eq!(view.opp_battleline.len(), 1);
        assert_eq!(view.opp_battleline[0].id, 7);
    }

    #[test]
    fn test_opponent_archives_hidden() {
        let game = test_game();
        let view = to_client_view(&game, 0);
        assert_eq!(view.opp_archives_count, 1);
    }

    #[test]
    fn test_aember_pools() {
        let game = test_game();
        let view = to_client_view(&game, 0);
        assert_eq!(view.my_player.aember_pool, 4);
        assert_eq!(view.opp_player.aember_pool, 2);
    }

    #[test]
    fn test_symmetry() {
        let game = test_game();
        let v0 = to_client_view(&game, 0);
        let v1 = to_client_view(&game, 1);
        assert_eq!(v0.my_hand.len(), v1.opp_hand_count);
        assert_eq!(v1.my_hand.len(), v0.opp_hand_count);
        assert_eq!(v0.my_index, 0);
        assert_eq!(v1.my_index, 1);
    }

    #[test]
    fn test_card_view_mutable_state() {
        let mut game = test_game();
        game.cards.get_mut(&4).unwrap().damage = 2;
        game.cards.get_mut(&4).unwrap().stun = true;
        let view = to_client_view(&game, 0);
        assert_eq!(view.my_battleline[0].damage, 2);
        assert!(view.my_battleline[0].stun);
    }

    #[test]
    fn test_keys_in_view() {
        let mut game = test_game();
        game.players[0].player.keys.forge(crate::victory::KeyColor::Red);
        let view = to_client_view(&game, 0);
        assert_eq!(view.my_player.keys.len(), 3);
        assert!(view.my_player.keys[0].forged); // Red
        assert!(!view.my_player.keys[1].forged); // Blue
    }
}
