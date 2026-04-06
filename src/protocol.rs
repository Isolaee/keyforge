use serde::{Deserialize, Serialize};

use crate::card::{
    BonusIcon, CardId, CardType, Effect, House, Keyword, Rarity,
};
use crate::victory::KeyColor;
use crate::zones::Flank;

// ---------------------------------------------------------------------------
// Client → Server
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub enum ClientMessage {
    ChooseHouse { house: House, pick_up_archives: bool },
    PlayCard { card_id: CardId, flank: Flank },
    PlayCardDeployed { card_id: CardId, index: usize },
    Reap { card_id: CardId },
    Attack { attacker_id: CardId, defender_id: CardId },
    Unstun { card_id: CardId },
    DiscardFromHand { card_id: CardId },
    EndTurn,
}

// ---------------------------------------------------------------------------
// Server → Client
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Tells the client which player index they are (0 or 1).
    Welcome { player_index: usize },
    /// Full filtered game state after every action.
    GameState(ClientGameView),
    /// The action was invalid.
    Error(String),
    /// The game is over.
    GameOver { winner: usize },
}

// ---------------------------------------------------------------------------
// Serializable card snapshot (replaces &'static CardDef + Card)
// ---------------------------------------------------------------------------

/// Everything a client needs to render a single card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardView {
    pub id: CardId,
    pub name: String,
    pub card_type: CardType,
    pub house: House,
    pub power: Option<u32>,
    pub armor: Option<u32>,
    pub keywords: Vec<Keyword>,
    pub bonus_icons: Vec<BonusIcon>,
    pub traits: Vec<String>,
    pub rarity: Rarity,
    pub on_reap: Vec<Effect>,
    pub on_fight: Vec<Effect>,
    pub on_play: Vec<Effect>,
    pub on_destroyed: Vec<Effect>,
    /// Human-readable card text.
    pub text: String,
    // Mutable state
    pub exhausted: bool,
    pub damage: u32,
    pub aember: u32,
    pub stun: bool,
    pub ward: bool,
    pub enrage: bool,
    pub power_counters: i32,
    pub armor_bonus: u32,
}

// ---------------------------------------------------------------------------
// Filtered game view for one player
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerView {
    pub aember_pool: u32,
    pub keys: Vec<KeyView>,
    pub chains: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyView {
    pub color: KeyColor,
    pub forged: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientGameView {
    /// Which player this view is for (0 or 1).
    pub my_index: usize,
    pub active_player: usize,
    pub active_house: Option<House>,
    pub turn: u32,

    // Own state — full visibility
    pub my_player: PlayerView,
    pub my_hand: Vec<CardView>,
    pub my_battleline: Vec<CardView>,
    pub my_artifacts: Vec<CardView>,
    pub my_discard: Vec<CardView>,
    pub my_archives: Vec<CardView>,
    pub my_deck_count: usize,

    // Opponent state — limited visibility
    pub opp_player: PlayerView,
    pub opp_hand_count: usize,
    pub opp_battleline: Vec<CardView>,
    pub opp_artifacts: Vec<CardView>,
    pub opp_discard: Vec<CardView>,
    pub opp_archives_count: usize,
    pub opp_deck_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{BonusIcon, CardType, House, Keyword, Rarity};
    use crate::victory::KeyColor;
    use crate::zones::Flank;

    fn roundtrip_client<T: serde::Serialize + for<'de> serde::Deserialize<'de> + std::fmt::Debug>(msg: T) {
        let json = serde_json::to_string(&msg).expect("serialize");
        let decoded: T = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(format!("{:?}", msg), format!("{:?}", decoded));
    }

    fn roundtrip_server(msg: &ServerMessage) -> ServerMessage {
        let json = serde_json::to_string(msg).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    // ---- ClientMessage variants ----

    #[test]
    fn test_serialize_choose_house() {
        roundtrip_client(ClientMessage::ChooseHouse {
            house: House::Brobnar,
            pick_up_archives: true,
        });
    }

    #[test]
    fn test_serialize_play_card() {
        roundtrip_client(ClientMessage::PlayCard { card_id: 42, flank: Flank::Right });
    }

    #[test]
    fn test_serialize_play_card_deployed() {
        roundtrip_client(ClientMessage::PlayCardDeployed { card_id: 7, index: 2 });
    }

    #[test]
    fn test_serialize_reap() {
        roundtrip_client(ClientMessage::Reap { card_id: 5 });
    }

    #[test]
    fn test_serialize_attack() {
        roundtrip_client(ClientMessage::Attack { attacker_id: 1, defender_id: 2 });
    }

    #[test]
    fn test_serialize_unstun() {
        roundtrip_client(ClientMessage::Unstun { card_id: 3 });
    }

    #[test]
    fn test_serialize_discard_from_hand() {
        roundtrip_client(ClientMessage::DiscardFromHand { card_id: 10 });
    }

    #[test]
    fn test_serialize_end_turn() {
        roundtrip_client(ClientMessage::EndTurn);
    }

    // ---- ServerMessage variants ----

    #[test]
    fn test_serialize_welcome() {
        let msg = ServerMessage::Welcome { player_index: 1 };
        let rt = roundtrip_server(&msg);
        assert!(matches!(rt, ServerMessage::Welcome { player_index: 1 }));
    }

    #[test]
    fn test_serialize_error() {
        let msg = ServerMessage::Error("Card not in hand".into());
        let rt = roundtrip_server(&msg);
        assert!(matches!(rt, ServerMessage::Error(ref s) if s == "Card not in hand"));
    }

    #[test]
    fn test_serialize_game_over() {
        let msg = ServerMessage::GameOver { winner: 0 };
        let rt = roundtrip_server(&msg);
        assert!(matches!(rt, ServerMessage::GameOver { winner: 0 }));
    }

    #[test]
    fn test_serialize_game_state() {
        let view = ClientGameView {
            my_index: 0,
            active_player: 0,
            active_house: Some(House::Logos),
            turn: 3,
            my_player: PlayerView {
                aember_pool: 4,
                keys: vec![
                    KeyView { color: KeyColor::Red, forged: true },
                    KeyView { color: KeyColor::Blue, forged: false },
                    KeyView { color: KeyColor::Yellow, forged: false },
                ],
                chains: 0,
            },
            my_hand: vec![CardView {
                id: 1,
                name: "Troll".into(),
                card_type: CardType::Creature,
                house: House::Brobnar,
                power: Some(5),
                armor: Some(1),
                keywords: vec![Keyword::Taunt],
                bonus_icons: vec![BonusIcon::Aember],
                traits: vec!["Giant".into()],
                rarity: Rarity::Common,
                on_reap: vec![],
                on_fight: vec![],
                on_play: vec![],
                on_destroyed: vec![],
                text: "Taunt.".into(),
                exhausted: false,
                damage: 0,
                aember: 0,
                stun: false,
                ward: false,
                enrage: false,
                power_counters: 0,
                armor_bonus: 0,
            }],
            my_battleline: vec![],
            my_artifacts: vec![],
            my_discard: vec![],
            my_archives: vec![],
            my_deck_count: 5,
            opp_player: PlayerView {
                aember_pool: 2,
                keys: vec![
                    KeyView { color: KeyColor::Red, forged: false },
                    KeyView { color: KeyColor::Blue, forged: false },
                    KeyView { color: KeyColor::Yellow, forged: false },
                ],
                chains: 6,
            },
            opp_hand_count: 4,
            opp_battleline: vec![],
            opp_artifacts: vec![],
            opp_discard: vec![],
            opp_archives_count: 1,
            opp_deck_count: 3,
        };
        let msg = ServerMessage::GameState(view.clone());
        let rt = roundtrip_server(&msg);
        if let ServerMessage::GameState(rt_view) = rt {
            assert_eq!(rt_view.my_index, 0);
            assert_eq!(rt_view.turn, 3);
            assert_eq!(rt_view.active_house, Some(House::Logos));
            assert_eq!(rt_view.my_player.aember_pool, 4);
            assert_eq!(rt_view.my_hand.len(), 1);
            assert_eq!(rt_view.my_hand[0].name, "Troll");
            assert_eq!(rt_view.my_player.keys[0].color, KeyColor::Red);
            assert!(rt_view.my_player.keys[0].forged);
            assert_eq!(rt_view.opp_player.chains, 6);
            assert_eq!(rt_view.my_deck_count, 5);
            assert_eq!(rt_view.opp_hand_count, 4);
        } else {
            panic!("wrong variant");
        }
    }

    // ---- Wire format spot-checks ----

    #[test]
    fn test_end_turn_wire_format() {
        let json = serde_json::to_string(&ClientMessage::EndTurn).unwrap();
        assert_eq!(json, r#""EndTurn""#);
    }

    #[test]
    fn test_welcome_wire_format() {
        let json = serde_json::to_string(&ServerMessage::Welcome { player_index: 0 }).unwrap();
        assert!(json.contains("Welcome"));
        assert!(json.contains("player_index"));
        assert!(json.contains('0'));
    }

    #[test]
    fn test_error_wire_format() {
        let json = serde_json::to_string(&ServerMessage::Error("oops".into())).unwrap();
        assert_eq!(json, r#"{"Error":"oops"}"#);
    }

    #[test]
    fn test_client_message_unknown_field_rejected() {
        // Malformed JSON should not deserialize as a valid message.
        let bad = r#"{"NotAMessage":{}}"#;
        let result: Result<ClientMessage, _> = serde_json::from_str(bad);
        assert!(result.is_err());
    }
}
