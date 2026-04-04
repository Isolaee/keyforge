use serde::{Deserialize, Serialize};

pub type CardId = u32;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum House {
    Brobnar,
    Dis,
    Ekwidon,
    Geistoid,
    Logos,
    Mars,
    Redemption,
    Sanctum,
    Saurian,
    Shadows,
    Skyborn,
    StarAlliance,
    Unfathomable,
    Untamed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum CardType {
    Creature,
    Action,
    Artifact,
    Upgrade,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Special,
    TokenCreature,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum BonusIcon {
    Aember,
    Capture,
    Damage,
    Draw,
    Discard,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Keyword {
    Alpha,
    Assault(u32),
    Capture,
    Deploy,
    Elusive,
    Exalt,
    Hazardous(u32),
    Invulnerable,
    Omega,
    Poison,
    Skirmish,
    SplashAttack(u32),
    Steal,
    Taunt,
    Treachery,
    Versatile,
}

/// Triggered ability effects applied when the associated trigger fires.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum Effect {
    /// Controller gains N Aember.
    GainAember(u32),
    /// Steal N Aember from opponent into controller's pool.
    StealAember(u32),
    /// Capture N Aember from opponent onto this creature.
    CaptureAember(u32),
    /// Controller draws N cards.
    DrawCards(u32),
    /// Deal N damage to each enemy creature.
    DealDamageToEachEnemy(u32),
    /// Heal N damage from this creature.
    HealSelf(u32),
}

/// Template for a card — shared across all instances of the same card.
/// Uses slices so instances can be declared as statics.
pub struct CardDef {
    pub name: &'static str,
    pub card_type: CardType,
    pub house: House,
    pub power: Option<u32>,
    pub armor: Option<u32>,
    pub keywords: &'static [Keyword],
    pub bonus_icons: &'static [BonusIcon],
    pub traits: &'static [&'static str],
    pub rarity: Rarity,
    /// Effects that fire when this card reaps.
    pub on_reap: &'static [Effect],
    /// Effects that fire when this card fights.
    pub on_fight: &'static [Effect],
    /// Effects that fire when this card is played.
    pub on_play: &'static [Effect],
    /// Effects that fire when this card is destroyed.
    pub on_destroyed: &'static [Effect],
    /// Human-readable card text (rules text printed on the card).
    pub text: &'static str,
}

/// A specific card in a game, with mutable state.
pub struct Card {
    pub id: CardId,
    pub def: &'static CardDef,
    pub exhausted: bool,
    pub damage: u32,
    pub aember: u32,
    pub upgrades: Vec<CardId>,
    pub stun: bool,
    pub ward: bool,
    pub enrage: bool,
    pub power_counters: i32,
    pub armor_used_this_turn: u32,
    pub armor_bonus: u32,
    pub extra_houses: Vec<House>,
    /// Elusive: true once the first attack this turn has been absorbed.
    pub elusive_used_this_turn: bool,
}

impl Card {
    pub fn new(id: CardId, def: &'static CardDef) -> Self {
        Self {
            id,
            def,
            exhausted: false,
            damage: 0,
            aember: 0,
            upgrades: Vec::new(),
            stun: false,
            ward: false,
            enrage: false,
            power_counters: 0,
            armor_used_this_turn: 0,
            armor_bonus: 0,
            extra_houses: Vec::new(),
            elusive_used_this_turn: false,
        }
    }

    /// Effective power = base + power_counters. Minimum 0.
    pub fn power(&self) -> u32 {
        let base = self.def.power.unwrap_or(0) as i32;
        (base + self.power_counters).max(0) as u32
    }

    /// Effective armor = base + gained armor.
    pub fn armor(&self) -> u32 {
        self.def.armor.unwrap_or(0) + self.armor_bonus
    }

    /// Remaining armor this turn (armor - armor_used_this_turn).
    pub fn remaining_armor(&self) -> u32 {
        self.armor().saturating_sub(self.armor_used_this_turn)
    }

    /// True if damage >= power (should be destroyed).
    pub fn is_destroyed(&self) -> bool {
        self.power() > 0 && self.damage >= self.power()
    }

    /// Apply pending damage after invulnerability, ward, and armor reduction.
    pub fn deal_damage(&mut self, amount: u32) {
        if self.has_keyword(Keyword::Invulnerable) {
            return;
        }
        if self.ward {
            self.ward = false;
            return;
        }
        let absorbed = self.remaining_armor().min(amount);
        self.armor_used_this_turn += absorbed;
        self.damage += amount - absorbed;
    }

    /// Remove damage. Cannot go below 0.
    pub fn heal(&mut self, amount: u32) {
        self.damage = self.damage.saturating_sub(amount);
    }

    /// Remove all damage.
    pub fn full_heal(&mut self) {
        self.damage = 0;
    }

    /// Reset per-turn state (armor usage, elusive).
    pub fn reset_turn(&mut self) {
        self.armor_used_this_turn = 0;
        self.elusive_used_this_turn = false;
    }

    /// True if this card has the given keyword.
    pub fn has_keyword(&self, kw: Keyword) -> bool {
        self.def.keywords.contains(&kw)
    }

    /// True if this card belongs to the given house (including bonus house icons).
    pub fn belongs_to_house(&self, house: House) -> bool {
        self.def.house == house || self.extra_houses.contains(&house)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static CREATURE_DEF: CardDef = CardDef {
        name: "Test Creature",
        card_type: CardType::Creature,
        house: House::Brobnar,
        power: Some(4),
        armor: Some(2),
        keywords: &[],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
        text: "Reap: Gain 1 aember.",
    };

    static NO_ARMOR_DEF: CardDef = CardDef {
        name: "Fragile",
        card_type: CardType::Creature,
        house: House::Dis,
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
        text: "Fight: Deal 1 damage to a friendly creature.",
    };

    static KEYWORD_DEF: CardDef = CardDef {
        name: "Assaulter",
        card_type: CardType::Creature,
        house: House::Logos,
        power: Some(2),
        armor: None,
        keywords: &[Keyword::Assault(3), Keyword::Skirmish],
        bonus_icons: &[],
        traits: &[],
        rarity: Rarity::Common,
        on_reap: &[],
        on_fight: &[],
        on_play: &[],
        on_destroyed: &[],
        text: "Assault 3. Skirmish.",
    };

    #[test]
    fn test_card_new() {
        let c = Card::new(1, &CREATURE_DEF);
        assert!(!c.exhausted);
        assert_eq!(c.damage, 0);
        assert_eq!(c.aember, 0);
        assert!(c.upgrades.is_empty());
        assert!(!c.stun);
        assert!(!c.ward);
        assert!(!c.elusive_used_this_turn);
    }

    #[test]
    fn test_power_with_counters() {
        let mut c = Card::new(1, &CREATURE_DEF);
        assert_eq!(c.power(), 4);
        c.power_counters = 2;
        assert_eq!(c.power(), 6);
        c.power_counters = -10;
        assert_eq!(c.power(), 0); // min 0
    }

    #[test]
    fn test_armor_prevents_damage() {
        let mut c = Card::new(1, &CREATURE_DEF); // armor 2
        c.deal_damage(5);
        assert_eq!(c.damage, 3); // 5 - 2 absorbed
    }

    #[test]
    fn test_armor_resets_each_turn() {
        let mut c = Card::new(1, &CREATURE_DEF); // armor 2
        c.deal_damage(2); // uses all armor
        assert_eq!(c.damage, 0);
        c.reset_turn();
        c.deal_damage(2); // armor restored
        assert_eq!(c.damage, 0);
    }

    #[test]
    fn test_armor_stacks() {
        let mut c = Card::new(1, &NO_ARMOR_DEF); // base armor 0
        c.armor_bonus = 3;
        assert_eq!(c.armor(), 3);
        c.deal_damage(1);
        assert_eq!(c.damage, 0); // absorbed by bonus armor
    }

    #[test]
    fn test_deal_damage() {
        let mut c = Card::new(1, &NO_ARMOR_DEF); // no armor
        c.deal_damage(2);
        assert_eq!(c.damage, 2);
    }

    #[test]
    fn test_is_destroyed() {
        let mut c = Card::new(1, &NO_ARMOR_DEF); // power 3
        c.damage = 2;
        assert!(!c.is_destroyed());
        c.damage = 3;
        assert!(c.is_destroyed());
    }

    #[test]
    fn test_heal() {
        let mut c = Card::new(1, &NO_ARMOR_DEF);
        c.damage = 5;
        c.heal(3);
        assert_eq!(c.damage, 2);
        c.heal(100);
        assert_eq!(c.damage, 0); // cannot go below 0
    }

    #[test]
    fn test_full_heal() {
        let mut c = Card::new(1, &NO_ARMOR_DEF);
        c.damage = 10;
        c.full_heal();
        assert_eq!(c.damage, 0);
    }

    #[test]
    fn test_has_keyword() {
        let c = Card::new(1, &KEYWORD_DEF);
        assert!(c.has_keyword(Keyword::Skirmish));
        assert!(!c.has_keyword(Keyword::Poison));
    }

    #[test]
    fn test_assault_value() {
        let c = Card::new(1, &KEYWORD_DEF);
        let value = c.def.keywords.iter().find_map(|kw| {
            if let Keyword::Assault(x) = kw { Some(*x) } else { None }
        });
        assert_eq!(value, Some(3));
    }

    #[test]
    fn test_belongs_to_house() {
        let c = Card::new(1, &CREATURE_DEF); // house Brobnar
        assert!(c.belongs_to_house(House::Brobnar));
        assert!(!c.belongs_to_house(House::Dis));
    }

    #[test]
    fn test_belongs_to_bonus_house() {
        let mut c = Card::new(1, &CREATURE_DEF); // house Brobnar
        c.extra_houses.push(House::Logos);
        assert!(c.belongs_to_house(House::Logos));
    }

    #[test]
    fn test_ward_prevents_damage() {
        let mut c = Card::new(1, &NO_ARMOR_DEF);
        c.ward = true;
        c.deal_damage(99);
        assert_eq!(c.damage, 0); // ward blocked
        assert!(!c.ward); // ward consumed
        c.deal_damage(1); // next hit lands
        assert_eq!(c.damage, 1);
    }

    #[test]
    fn test_stun_state() {
        let mut c = Card::new(1, &NO_ARMOR_DEF);
        assert!(!c.stun);
        c.stun = true;
        assert!(c.stun);
        c.stun = false;
        assert!(!c.stun);
    }

    #[test]
    fn test_elusive_resets_each_turn() {
        let mut c = Card::new(1, &NO_ARMOR_DEF);
        c.elusive_used_this_turn = true;
        c.reset_turn();
        assert!(!c.elusive_used_this_turn);
    }
}
