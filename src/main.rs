use macroquad::prelude::*;

mod card;
mod cards;
mod deck;
mod game;
mod victory;
mod zones;

use card::{CardId, CardType, House};
use game::{attack, choose_house, end_turn, play_card, reap, step_forge_key, GameState};
use zones::Flank;

// ---------------------------------------------------------------------------
// Dynamic layout — recomputed every frame from screen dimensions
// ---------------------------------------------------------------------------

/// All layout values derived from the current window size.
struct L {
    // card / zone sizes
    cw: f32, ch: f32, art_h: f32, gap: f32, zone_w: f32,
    // horizontal split
    panel_x: f32,
    // vertical positions
    p1_hand_y: f32, p1_line_y: f32, p1_art_y: f32,
    divider_y: f32,
    p0_art_y: f32, p0_line_y: f32, p0_hand_y: f32,
    // side-zone columns
    deck_x: f32, arch_x: f32, disc_x: f32,
    // status bar
    status_y: f32,
}

impl L {
    fn new(sw: f32, sh: f32) -> Self {
        // Scale relative to 1280 × 720 base
        let s = (sw / 1280.0).min(sh / 720.0);

        let cw     = 85.0 * s;
        let ch     = 115.0 * s;
        let art_h  = 50.0 * s;
        let gap    = 6.0 * s;
        let zone_w = 68.0 * s;

        let panel_x = sw - 200.0_f32.max(sw * 0.156); // right panel ~156px @ 1280

        let status_h = 28.0 * s;
        let available = sh - status_h;

        // 6 content rows + flank-strip buffer, spread over 7 equal gaps
        let flank_buf = 32.0 * s;
        let content_h = 4.0 * ch + 2.0 * art_h + flank_buf;
        let g = ((available - content_h) / 7.0).max(3.0);

        let p1_hand_y = g;
        let p1_line_y = p1_hand_y + ch + g;
        let p1_art_y  = p1_line_y + ch + g;
        let divider_y = p1_art_y + art_h + g;
        let p0_art_y  = divider_y + g;
        let p0_line_y = p0_art_y + art_h + flank_buf + g;
        let p0_hand_y = p0_line_y + ch + g;
        let status_y  = sh - status_h;

        let zone_total = zone_w * 3.0 + gap * 2.0;
        let deck_x = panel_x - zone_total - 20.0;
        let arch_x = deck_x + zone_w + gap;
        let disc_x = arch_x + zone_w + gap;

        Self { cw, ch, art_h, gap, zone_w, panel_x,
               p1_hand_y, p1_line_y, p1_art_y, divider_y,
               p0_art_y, p0_line_y, p0_hand_y,
               deck_x, arch_x, disc_x, status_y }
    }

    /// X of hand card i.
    fn cx(&self, i: usize) -> f32 {
        20.0 + i as f32 * (self.cw + self.gap)
    }

    /// Centered X for battleline card i out of count.
    fn blx(&self, i: usize, count: usize) -> f32 {
        let total = count as f32 * self.cw + count.saturating_sub(1) as f32 * self.gap;
        let start = ((self.panel_x - total) / 2.0).max(20.0);
        start + i as f32 * (self.cw + self.gap)
    }

    fn hit(&self, mx: f32, my: f32, x: f32, y: f32) -> bool {
        mx >= x && mx < x + self.cw && my >= y && my < y + self.ch
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct App {
    game: GameState,
    selected_hand: Option<CardId>,
    selected_creature: Option<CardId>,
    drag_card: Option<CardId>,
    msg: String,
}

impl App {
    fn new() -> Self {
        use cards::*;
        let p0: &[&'static card::CardDef] = &[
            &TROLL, &SMAAASH, &SILVERTOOTH, &VEZYMA_THINKDRONE,
            &PLAGUE, &BANNER_OF_BATTLE, &TROLL, &SMAAASH,
        ];
        let p1: &[&'static card::CardDef] = &[
            &TROLL, &SILVERTOOTH, &SMAAASH, &VEZYMA_THINKDRONE,
            &PLAGUE, &BANNER_OF_BATTLE, &SILVERTOOTH, &TROLL,
        ];
        let (mut all, ids0) = deck::build_deck(p0);
        let (cards1, ids1) = deck::build_deck(p1);
        all.extend(cards1);
        let mut g = GameState::new(ids0, ids1, all);
        for _ in 0..6 { g.players[0].zones.draw(); }
        for _ in 0..6 { g.players[1].zones.draw(); }
        Self {
            game: g,
            selected_hand: None,
            selected_creature: None,
            drag_card: None,
            msg: "Your turn. Choose a house to begin.".into(),
        }
    }

    fn deselect(&mut self) {
        self.selected_hand = None;
        self.selected_creature = None;
        self.drag_card = None;
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn in_box(mx: f32, my: f32, x: f32, y: f32, w: f32, h: f32) -> bool {
    mx >= x && mx < x + w && my >= y && my < y + h
}

fn draw_flank_badge(l: &L, x: f32, y: f32, label: &str, col: Color) {
    let bw = l.cw * 0.26;
    let bh = l.ch * 0.12;
    draw_rectangle(x, y, bw, bh, Color::from_rgba(0, 0, 0, 180));
    draw_text(label, x + 2.0, y + bh * 0.85, l.ch * 0.10, col);
}

fn draw_zone(l: &L, x: f32, y: f32, label: &str, count: usize, highlight: bool) {
    let border = if highlight { YELLOW } else { Color::from_rgba(70, 70, 100, 255) };
    draw_rectangle(x, y, l.zone_w, l.ch, Color::from_rgba(25, 25, 45, 255));
    draw_rectangle_lines(x, y, l.zone_w, l.ch, 2.0, border);
    draw_text(label, x + 4.0, y + l.ch * 0.14, l.ch * 0.11, GRAY);
    let s = count.to_string();
    let fs = l.ch * 0.22;
    draw_text(&s, x + l.zone_w / 2.0 - s.len() as f32 * fs * 0.5, y + l.ch * 0.6, fs, WHITE);
}

fn draw_artifact_row(l: &L, game: &GameState, player: usize, y: f32) {
    let ids = game.players[player].zones.artifacts.clone();
    let count = ids.len();
    let bg_base = if player == 0 {
        Color::from_rgba(70, 30, 110, 255)
    } else {
        Color::from_rgba(50, 20, 80, 255)
    };
    for (i, &id) in ids.iter().enumerate() {
        let x = l.blx(i, count);
        draw_rectangle(x, y, l.cw, l.art_h, bg_base);
        draw_rectangle_lines(x, y, l.cw, l.art_h, 2.0, DARKGRAY);
        let name = game.cards[&id].def.name;
        let n = if name.len() > 11 { &name[..11] } else { name };
        draw_text(n, x + 4.0, y + l.art_h * 0.38, l.art_h * 0.28, WHITE);
        draw_text("Artifact", x + 4.0, y + l.art_h * 0.88, l.art_h * 0.22, LIGHTGRAY);
    }
}

fn draw_card(l: &L, x: f32, y: f32, name: &str, sub: &str, bg: Color, border: Color) {
    draw_rectangle(x, y, l.cw, l.ch, bg);
    draw_rectangle_lines(x, y, l.cw, l.ch, 2.0, border);
    let n = if name.len() > 11 { &name[..11] } else { name };
    draw_text(n, x + 4.0, y + l.ch * 0.17, l.ch * 0.11, WHITE);
    draw_text(sub, x + 4.0, y + l.ch * 0.93, l.ch * 0.10, LIGHTGRAY);
}

fn btn(x: f32, y: f32, w: f32, h: f32, label: &str, active: bool,
       mx: f32, my: f32, click: bool) -> bool {
    let bg = if active {
        Color::from_rgba(40, 130, 40, 255)
    } else {
        Color::from_rgba(50, 50, 80, 255)
    };
    draw_rectangle(x, y, w, h, bg);
    draw_rectangle_lines(x, y, w, h, 2.0, LIGHTGRAY);
    draw_text(label, x + 6.0, y + h * 0.68, 15.0, WHITE);
    click && in_box(mx, my, x, y, w, h)
}

fn card_sub(game: &GameState, id: CardId) -> String {
    let c = &game.cards[&id];
    match c.def.card_type {
        CardType::Creature => format!("PWR:{} DMG:{}", c.power(), c.damage),
        CardType::Artifact => "Artifact".into(),
        CardType::Action   => "Action".into(),
        CardType::Upgrade  => "Upgrade".into(),
    }
}

/// True if the active house is set and this card belongs to it.
fn can_use(game: &GameState, id: CardId) -> bool {
    match game.active_house {
        Some(h) => game.cards[&id].belongs_to_house(h),
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Window config
// ---------------------------------------------------------------------------

fn window_conf() -> Conf {
    Conf {
        window_title: "Keyforge".to_owned(),
        window_width: 1280,
        window_height: 720,
        window_resizable: true,
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

#[macroquad::main(window_conf)]
async fn main() {
    let mut app = App::new();

    loop {
        clear_background(Color::from_rgba(20, 60, 20, 255));

        let l = L::new(screen_width(), screen_height());

        let (mx, my) = mouse_position();
        let click    = is_mouse_button_pressed(MouseButton::Left);
        let released = is_mouse_button_released(MouseButton::Left);
        let rclick   = is_mouse_button_pressed(MouseButton::Right);

        if rclick { app.deselect(); }

        let ap = app.game.active_player;
        let lfs = l.ch * 0.11; // label font size

        // ---- zone labels -----------------------------------------------
        draw_text("OPPONENT  hand",       20.0, l.p1_hand_y - 4.0, lfs, GRAY);
        draw_text("OPPONENT  battleline", 20.0, l.p1_line_y - 4.0, lfs, GRAY);
        draw_text("OPPONENT  artifacts",  20.0, l.p1_art_y  - 4.0, lfs, GRAY);
        draw_line(0.0, l.divider_y, l.panel_x, l.divider_y, 2.0, DARKGRAY);
        draw_text("YOUR  artifacts",      20.0, l.p0_art_y  - 4.0, lfs, GRAY);
        draw_text("YOUR  battleline",     20.0, l.p0_line_y - 4.0, lfs, GRAY);
        draw_text("YOUR  hand",           20.0, l.p0_hand_y - 4.0, lfs, GRAY);

        // ---- P1 hand (face-down) ----------------------------------------
        for i in 0..app.game.players[1].zones.hand.len() {
            draw_card(&l, l.cx(i), l.p1_hand_y,
                "?", "", Color::from_rgba(25, 25, 80, 255), GRAY);
        }
        draw_zone(&l, l.deck_x, l.p1_hand_y, "Deck",    app.game.players[1].zones.deck.len(),     false);
        draw_zone(&l, l.arch_x, l.p1_hand_y, "Archive", app.game.players[1].zones.archives.len(), false);
        draw_zone(&l, l.disc_x, l.p1_hand_y, "Discard", app.game.players[1].zones.discard.len(),  false);

        // ---- P1 battleline ----------------------------------------------
        let p1_creatures = app.game.players[1].zones.battleline.creature_ids();
        let p1_count = p1_creatures.len();
        for (i, &id) in p1_creatures.iter().enumerate() {
            let x = l.blx(i, p1_count);
            let exhausted = app.game.cards[&id].exhausted;
            let selected  = app.selected_creature == Some(id);
            let bg = if exhausted { Color::from_rgba(70, 15, 15, 255) }
                     else         { Color::from_rgba(160, 30, 30, 255) };
            let border = if selected { YELLOW } else { DARKGRAY };
            draw_card(&l, x, l.p1_line_y, app.game.cards[&id].def.name,
                &card_sub(&app.game, id), bg, border);
            if i == 0 {
                draw_flank_badge(&l, x, l.p1_line_y, "◄L", Color::from_rgba(255, 160, 160, 255));
            }
            if i == p1_count - 1 {
                draw_flank_badge(&l, x + l.cw - l.cw * 0.26, l.p1_line_y,
                    "R►", Color::from_rgba(255, 160, 160, 255));
            }
            if click && l.hit(mx, my, x, l.p1_line_y) {
                if let Some(att) = app.selected_creature {
                    let own = app.game.players[0].zones.battleline.creature_ids();
                    if own.contains(&att) && ap == 0 {
                        attack(&mut app.game, att, id);
                        app.selected_creature = None;
                        app.msg = "Attacked!".into();
                    }
                }
            }
        }

        // ---- P1 artifacts -----------------------------------------------
        draw_artifact_row(&l, &app.game, 1, l.p1_art_y);

        // ---- P0 battleline ----------------------------------------------
        let p0_creatures = app.game.players[0].zones.battleline.creature_ids();
        let p0_count = p0_creatures.len();
        for (i, &id) in p0_creatures.iter().enumerate() {
            let x = l.blx(i, p0_count);
            let exhausted = app.game.cards[&id].exhausted;
            let selected  = app.selected_creature == Some(id);
            let bg = if exhausted { Color::from_rgba(15, 60, 15, 255) }
                     else         { Color::from_rgba(25, 130, 25, 255) };
            let border = if selected { YELLOW } else { DARKGRAY };
            draw_card(&l, x, l.p0_line_y, app.game.cards[&id].def.name,
                &card_sub(&app.game, id), bg, border);
            if i == 0 {
                draw_flank_badge(&l, x, l.p0_line_y, "◄L", Color::from_rgba(160, 255, 160, 255));
            }
            if i == p0_count - 1 {
                draw_flank_badge(&l, x + l.cw - l.cw * 0.26, l.p0_line_y,
                    "R►", Color::from_rgba(160, 255, 160, 255));
            }
            if click && l.hit(mx, my, x, l.p0_line_y) && ap == 0 {
                if app.game.active_house.is_none() {
                    app.msg = "Choose a house first.".into();
                } else if !can_use(&app.game, id) {
                    app.msg = format!(
                        "{} is not a {:?} card — choose its house to use it.",
                        app.game.cards[&id].def.name, app.game.cards[&id].def.house);
                } else if selected {
                    if !app.game.cards[&id].exhausted {
                        reap(&mut app.game, id);
                        app.selected_creature = None;
                        app.msg = "Reaped! +1 Aember.".into();
                    }
                } else {
                    app.selected_creature = Some(id);
                    app.selected_hand = None;
                    app.msg = "Creature selected — click again to reap, click enemy to attack.".into();
                }
            }
        }

        // ---- P0 artifacts -----------------------------------------------
        draw_artifact_row(&l, &app.game, 0, l.p0_art_y);

        // ---- play drop-zones (shown when dragging or a hand card is selected) ---
        let active_card = app.selected_hand.or(app.drag_card);
        if active_card.is_some() {
            let is_artifact = active_card
                .map(|id| app.game.cards[&id].def.card_type == CardType::Artifact)
                .unwrap_or(false);

            let art_zone_w = l.panel_x - 40.0;
            let flank_zy   = l.p0_line_y - l.ch * 0.26;
            let flank_zh   = l.ch * 0.22;
            let half       = (l.panel_x - 50.0) / 2.0;
            let lx         = 20.0;
            let rx         = lx + half + 10.0;

            if is_artifact {
                draw_rectangle(20.0, l.p0_art_y, art_zone_w, l.art_h,
                    Color::from_rgba(180, 80, 255, 60));
                draw_rectangle_lines(20.0, l.p0_art_y, art_zone_w, l.art_h,
                    2.0, Color::from_rgba(220, 120, 255, 255));
                draw_text("▼ Drop here to play artifact", 30.0,
                    l.p0_art_y + l.art_h * 0.65, l.art_h * 0.28,
                    Color::from_rgba(220, 120, 255, 255));
            } else {
                draw_rectangle(lx, flank_zy, half, flank_zh, Color::from_rgba(200, 200, 0, 50));
                draw_rectangle_lines(lx, flank_zy, half, flank_zh, 2.0, YELLOW);
                draw_text("◄ Left flank", lx + 8.0, flank_zy + flank_zh * 0.72, lfs, YELLOW);

                draw_rectangle(rx, flank_zy, half, flank_zh, Color::from_rgba(200, 200, 0, 50));
                draw_rectangle_lines(rx, flank_zy, half, flank_zh, 2.0, YELLOW);
                draw_text("Right flank ►", rx + 8.0, flank_zy + flank_zh * 0.72, lfs, YELLOW);
            }

            if click {
                let on_art   = in_box(mx, my, 20.0, l.p0_art_y, art_zone_w, l.art_h);
                let on_left  = in_box(mx, my, lx, flank_zy, half, flank_zh);
                let on_right = in_box(mx, my, rx, flank_zy, half, flank_zh);
                let flank = if on_left  { Some(Flank::Left) }
                       else if on_right { Some(Flank::Right) }
                       else if on_art   { Some(Flank::Right) } // flank ignored for artifacts
                       else             { None };
                if let (Some(flank), Some(id)) = (flank, app.selected_hand) {
                    if app.game.active_house.is_none() {
                        app.msg = "Choose a house first.".into();
                    } else if !can_use(&app.game, id) {
                        app.msg = format!(
                            "{} is not a {:?} card — choose its house to play it.",
                            app.game.cards[&id].def.name, app.game.cards[&id].def.house);
                    } else {
                        let ct = app.game.cards[&id].def.card_type;
                        play_card(&mut app.game, id, flank);
                        app.selected_hand = None;
                        app.msg = match ct {
                            CardType::Creature => "Creature played.".into(),
                            CardType::Artifact => "Artifact played.".into(),
                            CardType::Action   => "Action played — card goes to discard.".into(),
                            CardType::Upgrade  => "Upgrade played.".into(),
                        };
                    }
                }
            }
        }

        // ---- P0 hand ----------------------------------------------------
        let p0_hand: Vec<CardId> = app.game.players[0].zones.hand.clone();
        for (i, &id) in p0_hand.iter().enumerate() {
            let selected = app.selected_hand == Some(id);
            let playable = can_use(&app.game, id);
            let bg = if selected        { Color::from_rgba(180, 140, 0, 255) }
                     else if playable   { Color::from_rgba(30, 60, 160, 255) }
                     else               { Color::from_rgba(20, 20, 50, 255) };
            let border = if selected { WHITE } else if playable { GRAY } else { DARKGRAY };
            draw_card(&l, l.cx(i), l.p0_hand_y,
                app.game.cards[&id].def.name, &card_sub(&app.game, id), bg, border);
            if click && l.hit(mx, my, l.cx(i), l.p0_hand_y) && ap == 0 {
                app.drag_card = Some(id);
                app.selected_hand = None;
                app.selected_creature = None;
            }
        }

        // ---- P0 side zones ----------------------------------------------
        let dragging_p0 = app.drag_card.is_some() && ap == 0;
        draw_zone(&l, l.deck_x, l.p0_hand_y, "Deck",    app.game.players[0].zones.deck.len(),     false);
        draw_zone(&l, l.arch_x, l.p0_hand_y, "Archive", app.game.players[0].zones.archives.len(), false);
        draw_zone(&l, l.disc_x, l.p0_hand_y, "Discard", app.game.players[0].zones.discard.len(),  dragging_p0);

        // ================================================================
        // Right panel
        // ================================================================
        let sw = screen_width();
        let sh = screen_height();
        draw_rectangle(l.panel_x, 0.0, sw - l.panel_x, sh, Color::from_rgba(12, 12, 28, 255));

        let px = l.panel_x + 8.0;

        draw_text(&format!("Turn {}", app.game.turn), px, 28.0, 20.0, WHITE);
        draw_text(&format!("Active: P{}", ap),        px, 50.0, 16.0, LIGHTGRAY);

        let ah_lbl = match app.game.active_house {
            Some(House::Brobnar) => "Brobnar",
            Some(House::Dis)     => "Dis",
            Some(House::Shadows) => "Shadows",
            _                    => "—",
        };
        draw_text(&format!("House:  {}", ah_lbl), px, 70.0, 16.0, LIGHTGRAY);
        draw_line(l.panel_x, 80.0, sw, 80.0, 1.0, DARKGRAY);

        draw_text("P0 (You)", px, 100.0, 15.0, GREEN);
        draw_text(&format!("Aember: {}", app.game.players[0].player.aember_pool), px, 118.0, 14.0, GOLD);
        draw_text(&format!("Keys:   {}/3", app.game.players[0].player.keys.forged_count()), px, 134.0, 14.0, GOLD);

        draw_text("P1 (Opp)", px, 155.0, 15.0, RED);
        draw_text(&format!("Aember: {}", app.game.players[1].player.aember_pool), px, 173.0, 14.0, ORANGE);
        draw_text(&format!("Keys:   {}/3", app.game.players[1].player.keys.forged_count()), px, 189.0, 14.0, ORANGE);

        draw_line(l.panel_x, 202.0, sw, 202.0, 1.0, DARKGRAY);

        draw_text("Choose house:", px, 220.0, 14.0, LIGHTGRAY);
        let houses = [(House::Brobnar, "Brobnar"), (House::Dis, "Dis"), (House::Shadows, "Shadows")];
        for (i, (h, label)) in houses.iter().enumerate() {
            let by = 228.0 + i as f32 * 42.0;
            let active = app.game.active_house == Some(*h);
            if btn(px, by, 128.0, 32.0, label, active, mx, my, click) && ap == 0 {
                choose_house(&mut app.game, *h, false);
                app.msg = format!("House {} chosen.", label);
            }
        }

        draw_line(l.panel_x, 360.0, sw, 360.0, 1.0, DARKGRAY);

        if btn(px, 370.0, 128.0, 36.0, "End Turn", false, mx, my, click) {
            end_turn(&mut app.game);
            let new_ap = app.game.active_player;
            step_forge_key(&mut app.game.players[new_ap].player);
            app.deselect();
            app.msg = format!("P{} to play. Choose a house.", new_ap);
        }

        draw_line(l.panel_x, 416.0, sw, 416.0, 1.0, DARKGRAY);

        draw_text("Controls:",                px, 496.0, 13.0, GRAY);
        draw_text("drag card → zone  play",   px, 512.0, 12.0, DARKGRAY);
        draw_text("click card        select", px, 526.0, 12.0, DARKGRAY);
        draw_text("click zone        play",   px, 540.0, 12.0, DARKGRAY);
        draw_text("2× own creature   reap",   px, 554.0, 12.0, DARKGRAY);
        draw_text("select+enemy      attack", px, 568.0, 12.0, DARKGRAY);
        draw_text("R-click           clear",  px, 582.0, 12.0, DARKGRAY);

        // ---- win overlay ------------------------------------------------
        let p0_won = app.game.players[0].player.keys.has_won();
        let p1_won = app.game.players[1].player.keys.has_won();
        if p0_won || p1_won {
            let ow = l.panel_x * 0.7;
            let ox = (l.panel_x - ow) / 2.0;
            let oy = sh / 2.0 - 65.0;
            draw_rectangle(ox, oy, ow, 130.0, Color::from_rgba(0, 0, 0, 210));
            let (txt, col) = if p0_won { ("PLAYER 0 WINS!", GOLD) } else { ("PLAYER 1 WINS!", RED) };
            draw_text(txt, ox + 20.0, oy + 80.0, l.ch * 0.62, col);
        }

        // ---- drag ghost -------------------------------------------------
        if let Some(drag_id) = app.drag_card {
            if is_mouse_button_down(MouseButton::Left) {
                let name = app.game.cards[&drag_id].def.name;
                let sub  = card_sub(&app.game, drag_id);
                draw_card(&l, mx - l.cw / 2.0, my - l.ch / 2.0, name, &sub,
                    Color::from_rgba(200, 180, 60, 210), WHITE);
            }
        }

        // ---- drag release -----------------------------------------------
        if released {
            if let Some(drag_id) = app.drag_card {
                let art_zone_w = l.panel_x - 40.0;
                let flank_zy   = l.p0_line_y - l.ch * 0.26;
                let flank_zh   = l.ch * 0.22;
                let half       = (l.panel_x - 50.0) / 2.0;
                let lx         = 20.0;
                let rx         = lx + half + 10.0;

                let on_discard    = in_box(mx, my, l.disc_x, l.p0_hand_y, l.zone_w, l.ch) && ap == 0;
                let on_artifact   = in_box(mx, my, 20.0, l.p0_art_y, art_zone_w, l.art_h);
                let on_left_flank = in_box(mx, my, lx, flank_zy, half, flank_zh);
                let on_right_flank= in_box(mx, my, rx, flank_zy, half, flank_zh);
                let drag_type     = app.game.cards[&drag_id].def.card_type;

                if on_discard {
                    app.game.players[ap].zones.discard_from_hand(drag_id);
                    app.msg = "Card discarded.".into();
                } else if on_artifact && drag_type == CardType::Artifact {
                    if app.game.active_house.is_none() {
                        app.msg = "Choose a house first.".into();
                    } else if !can_use(&app.game, drag_id) {
                        app.msg = format!(
                            "{} is not a {:?} card — choose its house to play it.",
                            app.game.cards[&drag_id].def.name, app.game.cards[&drag_id].def.house);
                    } else {
                        play_card(&mut app.game, drag_id, Flank::Right);
                        app.msg = "Artifact played.".into();
                    }
                } else if on_left_flank || on_right_flank {
                    let flank = if on_left_flank { Flank::Left } else { Flank::Right };
                    if app.game.active_house.is_none() {
                        app.msg = "Choose a house first.".into();
                    } else if !can_use(&app.game, drag_id) {
                        app.msg = format!(
                            "{} is not a {:?} card — choose its house to play it.",
                            app.game.cards[&drag_id].def.name, app.game.cards[&drag_id].def.house);
                    } else {
                        play_card(&mut app.game, drag_id, flank);
                        app.msg = match drag_type {
                            CardType::Creature => "Creature played.".into(),
                            CardType::Artifact => "Artifact played.".into(),
                            CardType::Action   => "Action played — card goes to discard.".into(),
                            CardType::Upgrade  => "Upgrade played.".into(),
                        };
                    }
                } else {
                    app.selected_hand = Some(drag_id);
                    app.msg = "Card selected — drop in a zone to play or discard.".into();
                }
                app.drag_card = None;
            }
        }

        // ---- status bar -------------------------------------------------
        draw_rectangle(0.0, l.status_y, l.panel_x, sh - l.status_y,
            Color::from_rgba(0, 0, 0, 170));
        draw_text(&app.msg, 8.0, l.status_y + (sh - l.status_y) * 0.72, lfs, YELLOW);

        next_frame().await
    }
}
