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
