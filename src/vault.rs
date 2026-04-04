use serde::Deserialize;

use crate::deck_store::SavedDeck;

#[derive(Deserialize)]
struct DeckLinks {
    houses: Vec<String>,
}

#[derive(Deserialize)]
struct DeckData {
    id: String,
    name: String,
    #[serde(rename = "_links")]
    links: DeckLinks,
}

#[derive(Deserialize)]
struct CardData {
    card_title: String,
}

#[derive(Deserialize)]
struct LinkedData {
    cards: Vec<CardData>,
}

#[derive(Deserialize)]
struct ApiResponse {
    data: DeckData,
    #[serde(rename = "_linked")]
    linked: LinkedData,
}

/// Extract the UUID segment from a KeyForge Vault URL.
pub fn extract_uuid(url: &str) -> Option<String> {
    url.trim()
        .split('/')
        .find(|seg| seg.len() == 36 && seg.chars().filter(|&c| c == '-').count() == 4)
        .map(|s| s.to_string())
}

/// Fetch a deck from the KeyForge Vault API. Blocking — run in a thread.
pub fn fetch_deck(url: &str) -> Result<SavedDeck, String> {
    let uuid = extract_uuid(url).ok_or("No valid UUID found in URL")?;
    let api_url = format!("https://www.keyforgegame.com/api/decks/{}/?links=cards", uuid);

    let body = ureq::get(&api_url)
        .call()
        .map_err(|e| format!("Request failed: {e}"))?
        .into_string()
        .map_err(|e| format!("Read error: {e}"))?;

    let resp: ApiResponse =
        serde_json::from_str(&body).map_err(|e| format!("Parse error: {e}"))?;

    Ok(SavedDeck {
        id: resp.data.id,
        name: resp.data.name,
        houses: resp.data.links.houses,
        cards: resp.linked.cards.into_iter().map(|c| c.card_title).collect(),
    })
}
