use crate::game::{
    attack, choose_house, end_turn, play_card, play_card_deployed, reap, step_forge_key, unstun,
    GameState,
};
use crate::protocol::ClientMessage;

/// Process one client message on behalf of `player_idx`.
/// Returns `Ok(())` on success or `Err(reason)` for invalid actions.
pub fn dispatch_message(
    game: &mut GameState,
    player_idx: usize,
    msg: ClientMessage,
) -> Result<(), String> {
    match msg {
        ClientMessage::ChooseHouse { house, pick_up_archives } => {
            choose_house(game, house, pick_up_archives);
            Ok(())
        }
        ClientMessage::PlayCard { card_id, flank } => {
            if !game.players[player_idx].zones.hand.contains(&card_id) {
                Err("Card not in hand".into())
            } else {
                play_card(game, card_id, flank);
                Ok(())
            }
        }
        ClientMessage::PlayCardDeployed { card_id, index } => {
            if !game.players[player_idx].zones.hand.contains(&card_id) {
                Err("Card not in hand".into())
            } else {
                play_card_deployed(game, card_id, index);
                Ok(())
            }
        }
        ClientMessage::Reap { card_id } => {
            if !game.players[player_idx].zones.battleline.creature_ids().contains(&card_id) {
                Err("Creature not on your battleline".into())
            } else {
                reap(game, card_id);
                Ok(())
            }
        }
        ClientMessage::Attack { attacker_id, defender_id } => {
            let own = game.players[player_idx].zones.battleline.creature_ids();
            let opp = game.players[1 - player_idx].zones.battleline.creature_ids();
            if !own.contains(&attacker_id) {
                Err("Attacker not on your battleline".into())
            } else if !opp.contains(&defender_id) {
                Err("Defender not on opponent battleline".into())
            } else {
                attack(game, attacker_id, defender_id);
                Ok(())
            }
        }
        ClientMessage::Unstun { card_id } => {
            if !game.players[player_idx].zones.battleline.creature_ids().contains(&card_id) {
                Err("Creature not on your battleline".into())
            } else {
                unstun(game, card_id);
                Ok(())
            }
        }
        ClientMessage::DiscardFromHand { card_id } => {
            if !game.players[player_idx].zones.hand.contains(&card_id) {
                Err("Card not in hand".into())
            } else {
                game.players[player_idx].zones.discard_from_hand(card_id);
                Ok(())
            }
        }
        ClientMessage::EndTurn => {
            end_turn(game);
            let new_ap = game.active_player;
            step_forge_key(&mut game.players[new_ap].player);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{CardDef, CardType, House, Keyword, Rarity};
    use crate::cards::{SMAAASH, TROLL};
    use crate::deck::build_deck;
    use crate::game::GameState;
    use crate::protocol::ClientMessage;
    use crate::zones::Flank;
    use std::collections::HashMap;

    fn game_with(p0: &[&'static CardDef], p1: &[&'static CardDef]) -> GameState {
        let (mut cards, ids0) = build_deck(p0);
        let (cards1, ids1) = build_deck(p1);
        cards.extend(cards1);
        GameState::new_no_draw(ids0, ids1, cards)
    }

    // ---- ChooseHouse ----

    #[test]
    fn test_dispatch_choose_house() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let msg = ClientMessage::ChooseHouse { house: House::Brobnar, pick_up_archives: false };
        assert!(dispatch_message(&mut game, 0, msg).is_ok());
        assert_eq!(game.active_house, Some(House::Brobnar));
    }

    #[test]
    fn test_dispatch_choose_house_picks_up_archives() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let archived_id = *game.players[0].zones.deck.last().unwrap();
        game.players[0].zones.deck.retain(|&id| id != archived_id);
        game.players[0].zones.archives.push(archived_id);

        let msg = ClientMessage::ChooseHouse { house: House::Brobnar, pick_up_archives: true };
        dispatch_message(&mut game, 0, msg).unwrap();

        assert!(game.players[0].zones.hand.contains(&archived_id));
        assert!(game.players[0].zones.archives.is_empty());
    }

    // ---- PlayCard ----

    #[test]
    fn test_dispatch_play_card() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        let msg = ClientMessage::PlayCard { card_id: id, flank: Flank::Left };
        dispatch_message(&mut game, 0, msg).unwrap();
        assert!(game.players[0].zones.battleline.creature_ids().contains(&id));
    }

    #[test]
    fn test_dispatch_play_card_not_in_hand() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let bogus_id = 9999;
        let msg = ClientMessage::PlayCard { card_id: bogus_id, flank: Flank::Left };
        let err = dispatch_message(&mut game, 0, msg).unwrap_err();
        assert_eq!(err, "Card not in hand");
    }

    // ---- PlayCardDeployed ----

    static DEPLOY_DEF: CardDef = CardDef {
        name: "Deploy",
        card_type: CardType::Creature,
        house: House::Logos,
        power: Some(2),
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

    #[test]
    fn test_dispatch_play_card_deployed() {
        let mut game = game_with(&[&TROLL, &TROLL, &DEPLOY_DEF], &[&TROLL]);
        let left = game.players[0].zones.draw().unwrap();
        let right = game.players[0].zones.draw().unwrap();
        dispatch_message(
            &mut game, 0,
            ClientMessage::PlayCard { card_id: left, flank: Flank::Left },
        ).unwrap();
        dispatch_message(
            &mut game, 0,
            ClientMessage::PlayCard { card_id: right, flank: Flank::Right },
        ).unwrap();
        let deploy_id = game.players[0].zones.draw().unwrap();
        let msg = ClientMessage::PlayCardDeployed { card_id: deploy_id, index: 1 };
        dispatch_message(&mut game, 0, msg).unwrap();
        let neighbors = game.players[0].zones.battleline.neighbors(deploy_id);
        assert_eq!(neighbors, (Some(left), Some(right)));
    }

    #[test]
    fn test_dispatch_play_card_deployed_not_in_hand() {
        let mut game = game_with(&[&DEPLOY_DEF], &[&TROLL]);
        let err = dispatch_message(
            &mut game, 0,
            ClientMessage::PlayCardDeployed { card_id: 9999, index: 0 },
        ).unwrap_err();
        assert_eq!(err, "Card not in hand");
    }

    // ---- Reap ----

    #[test]
    fn test_dispatch_reap() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        dispatch_message(
            &mut game, 0,
            ClientMessage::PlayCard { card_id: id, flank: Flank::Left },
        ).unwrap();
        dispatch_message(&mut game, 0, ClientMessage::Reap { card_id: id }).unwrap();
        assert!(game.cards[&id].exhausted);
        assert_eq!(game.players[0].player.aember_pool, 2); // bonus icon + reap
    }

    #[test]
    fn test_dispatch_reap_not_on_battleline() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap(); // in hand, not played
        let err = dispatch_message(&mut game, 0, ClientMessage::Reap { card_id: id }).unwrap_err();
        assert_eq!(err, "Creature not on your battleline");
    }

    // ---- Attack ----

    #[test]
    fn test_dispatch_attack() {
        use crate::cards::VEZYMA_THINKDRONE;
        // Smaaash (3 power, Assault 2) vs Vezyma (1 power).
        // Assault(2) pre-damage destroys Vezyma before fight resolves.
        let mut game = game_with(&[&SMAAASH], &[&VEZYMA_THINKDRONE]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap();
        dispatch_message(
            &mut game, 0,
            ClientMessage::PlayCard { card_id: att, flank: Flank::Left },
        ).unwrap();
        game.active_player = 1;
        dispatch_message(
            &mut game, 1,
            ClientMessage::PlayCard { card_id: def, flank: Flank::Left },
        ).unwrap();
        game.active_player = 0;

        dispatch_message(
            &mut game, 0,
            ClientMessage::Attack { attacker_id: att, defender_id: def },
        ).unwrap();

        assert!(game.cards[&att].exhausted);
        assert!(game.players[1].zones.battleline.is_empty());
    }

    #[test]
    fn test_dispatch_attack_attacker_not_on_own_battleline() {
        let mut game = game_with(&[&SMAAASH], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap(); // in hand, not played
        let def = game.players[1].zones.draw().unwrap();
        game.active_player = 1;
        dispatch_message(
            &mut game, 1,
            ClientMessage::PlayCard { card_id: def, flank: Flank::Left },
        ).unwrap();
        game.active_player = 0;

        let err = dispatch_message(
            &mut game, 0,
            ClientMessage::Attack { attacker_id: att, defender_id: def },
        ).unwrap_err();
        assert_eq!(err, "Attacker not on your battleline");
    }

    #[test]
    fn test_dispatch_attack_defender_not_on_opp_battleline() {
        let mut game = game_with(&[&SMAAASH], &[&TROLL]);
        let att = game.players[0].zones.draw().unwrap();
        let def = game.players[1].zones.draw().unwrap(); // not played
        dispatch_message(
            &mut game, 0,
            ClientMessage::PlayCard { card_id: att, flank: Flank::Left },
        ).unwrap();

        let err = dispatch_message(
            &mut game, 0,
            ClientMessage::Attack { attacker_id: att, defender_id: def },
        ).unwrap_err();
        assert_eq!(err, "Defender not on opponent battleline");
    }

    // ---- Unstun ----

    #[test]
    fn test_dispatch_unstun() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        dispatch_message(
            &mut game, 0,
            ClientMessage::PlayCard { card_id: id, flank: Flank::Left },
        ).unwrap();
        game.cards.get_mut(&id).unwrap().stun = true;

        dispatch_message(&mut game, 0, ClientMessage::Unstun { card_id: id }).unwrap();

        assert!(!game.cards[&id].stun);
        assert!(game.cards[&id].exhausted);
    }

    #[test]
    fn test_dispatch_unstun_not_on_battleline() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        let err = dispatch_message(
            &mut game, 0,
            ClientMessage::Unstun { card_id: id },
        ).unwrap_err();
        assert_eq!(err, "Creature not on your battleline");
    }

    // ---- DiscardFromHand ----

    #[test]
    fn test_dispatch_discard_from_hand() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let id = game.players[0].zones.draw().unwrap();
        dispatch_message(
            &mut game, 0,
            ClientMessage::DiscardFromHand { card_id: id },
        ).unwrap();
        assert!(!game.players[0].zones.hand.contains(&id));
        assert!(game.players[0].zones.discard.contains(&id));
    }

    #[test]
    fn test_dispatch_discard_from_hand_not_in_hand() {
        let mut game = game_with(&[&TROLL], &[&TROLL]);
        let err = dispatch_message(
            &mut game, 0,
            ClientMessage::DiscardFromHand { card_id: 9999 },
        ).unwrap_err();
        assert_eq!(err, "Card not in hand");
    }

    // ---- EndTurn ----

    #[test]
    fn test_dispatch_end_turn_switches_active_player() {
        let mut game = game_with(
            &[&TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL],
            &[&TROLL],
        );
        assert_eq!(game.active_player, 0);
        dispatch_message(&mut game, 0, ClientMessage::EndTurn).unwrap();
        assert_eq!(game.active_player, 1);
    }

    #[test]
    fn test_dispatch_end_turn_advances_turn_counter() {
        let mut game = game_with(
            &[&TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL],
            &[&TROLL],
        );
        assert_eq!(game.turn, 1);
        dispatch_message(&mut game, 0, ClientMessage::EndTurn).unwrap();
        assert_eq!(game.turn, 2);
    }

    #[test]
    fn test_dispatch_end_turn_forges_key_when_eligible() {
        let mut game = game_with(
            &[&TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL, &TROLL],
            &[&TROLL],
        );
        // After end_turn, new active player is P1. Give P1 enough aember to forge.
        game.players[1].player.aember_pool = 6;
        dispatch_message(&mut game, 0, ClientMessage::EndTurn).unwrap();
        // step_forge_key runs for P1 (new active player)
        assert_eq!(game.players[1].player.keys.forged_count(), 1);
        assert_eq!(game.players[1].player.aember_pool, 0);
    }
}
