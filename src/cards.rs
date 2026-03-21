use crate::card::{BonusIcon, CardDef, CardType, House, Keyword, Rarity};

// --- Brobnar ---

pub static TROLL: CardDef = CardDef {
    name: "Troll",
    card_type: CardType::Creature,
    house: House::Brobnar,
    power: Some(5),
    armor: Some(1),
    keywords: &[Keyword::Taunt],
    bonus_icons: &[BonusIcon::Aember],
    traits: &["Giant"],
    rarity: Rarity::Common,
};

pub static BANNER_OF_BATTLE: CardDef = CardDef {
    name: "Banner of Battle",
    card_type: CardType::Artifact,
    house: House::Brobnar,
    power: None,
    armor: None,
    keywords: &[],
    bonus_icons: &[],
    traits: &["Item"],
    rarity: Rarity::Uncommon,
};

pub static SMAAASH: CardDef = CardDef {
    name: "Smaaash",
    card_type: CardType::Creature,
    house: House::Brobnar,
    power: Some(3),
    armor: None,
    keywords: &[Keyword::Assault(2)],
    bonus_icons: &[BonusIcon::Damage],
    traits: &["Goblin"],
    rarity: Rarity::Rare,
};

// --- Dis ---

pub static VEZYMA_THINKDRONE: CardDef = CardDef {
    name: "Vezyma Thinkdrone",
    card_type: CardType::Creature,
    house: House::Dis,
    power: Some(1),
    armor: None,
    keywords: &[Keyword::Poison],
    bonus_icons: &[BonusIcon::Aember],
    traits: &["Human", "Scientist"],
    rarity: Rarity::Common,
};

pub static PLAGUE: CardDef = CardDef {
    name: "Plague",
    card_type: CardType::Action,
    house: House::Dis,
    power: None,
    armor: None,
    keywords: &[],
    bonus_icons: &[BonusIcon::Aember],
    traits: &[],
    rarity: Rarity::Uncommon,
};

// --- Shadows ---

pub static SILVERTOOTH: CardDef = CardDef {
    name: "Silvertooth",
    card_type: CardType::Creature,
    house: House::Shadows,
    power: Some(2),
    armor: None,
    keywords: &[Keyword::Skirmish, Keyword::Steal],
    bonus_icons: &[],
    traits: &["Thief"],
    rarity: Rarity::Common,
};

pub static SHADOW_SELF: CardDef = CardDef {
    name: "Shadow Self",
    card_type: CardType::Upgrade,
    house: House::Shadows,
    power: None,
    armor: None,
    keywords: &[Keyword::Elusive],
    bonus_icons: &[],
    traits: &[],
    rarity: Rarity::Rare,
};
