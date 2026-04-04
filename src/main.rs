use std::io::{BufRead, BufReader, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::mpsc;
use std::time::Duration;

use macroquad::prelude::*;

use keyforge::card::{CardId, CardType, House};
use keyforge::deck_store::SavedDeck;
use keyforge::protocol::{CardView, ClientGameView, ClientMessage, ServerMessage};
use keyforge::zones::Flank;
use keyforge::{deck_store, vault};

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
    view: Option<ClientGameView>,
    rx: mpsc::Receiver<ServerMessage>,
    tx_stream: TcpStream,
    selected_hand: Option<CardId>,
    selected_creature: Option<CardId>,
    drag_card: Option<CardId>,
    msg: String,
    game_over: Option<usize>,
}

impl App {
    fn from_stream(stream: TcpStream) -> Self {
        let reader_stream = stream.try_clone().expect("clone");
        let tx_stream = stream;

        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(reader_stream);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if let Ok(msg) = serde_json::from_str::<ServerMessage>(line.trim()) {
                            if tx.send(msg).is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        });

        Self {
            view: None,
            rx,
            tx_stream,
            selected_hand: None,
            selected_creature: None,
            drag_card: None,
            msg: "Connecting...".into(),
            game_over: None,
        }
    }

    fn send(&self, msg: &ClientMessage) {
        let mut line = serde_json::to_string(msg).expect("serialize");
        line.push('\n');
        let _ = (&self.tx_stream).write_all(line.as_bytes());
        let _ = (&self.tx_stream).flush();
    }

    fn poll(&mut self) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                ServerMessage::Welcome { player_index } => {
                    self.msg = format!("Connected as player {}. Waiting for game state...", player_index);
                }
                ServerMessage::GameState(view) => {
                    self.msg = if view.active_player == view.my_index {
                        "Your turn. Choose a house.".into()
                    } else {
                        "Opponent's turn.".into()
                    };
                    self.view = Some(view);
                }
                ServerMessage::Error(e) => {
                    self.msg = format!("Error: {}", e);
                }
                ServerMessage::GameOver { winner } => {
                    self.game_over = Some(winner);
                }
            }
        }
    }

    fn deselect(&mut self) {
        self.selected_hand = None;
        self.selected_creature = None;
        self.drag_card = None;
    }

    fn is_my_turn(&self) -> bool {
        self.view.as_ref().is_some_and(|v| v.active_player == v.my_index)
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

fn draw_artifact_row(l: &L, artifacts: &[CardView], is_mine: bool, y: f32) {
    let count = artifacts.len();
    let bg_base = if is_mine {
        Color::from_rgba(70, 30, 110, 255)
    } else {
        Color::from_rgba(50, 20, 80, 255)
    };
    for (i, card) in artifacts.iter().enumerate() {
        let x = l.blx(i, count);
        draw_rectangle(x, y, l.cw, l.art_h, bg_base);
        draw_rectangle_lines(x, y, l.cw, l.art_h, 2.0, DARKGRAY);
        let name = &card.name;
        let n = if name.len() > 11 { &name[..11] } else { name };
        draw_text(n, x + 4.0, y + l.art_h * 0.38, l.art_h * 0.28, WHITE);
        draw_text("Artifact", x + 4.0, y + l.art_h * 0.88, l.art_h * 0.22, LIGHTGRAY);
    }
}

fn wrap_text(text: &str, font_size: f32, max_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{} {}", current, word)
        };
        if measure_text(&candidate, None, font_size as u16, 1.0).width > max_width && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn draw_card(l: &L, x: f32, y: f32, name: &str, text: &str, sub: &str, bg: Color, border: Color) {
    draw_rectangle(x, y, l.cw, l.ch, bg);
    draw_rectangle_lines(x, y, l.cw, l.ch, 2.0, border);
    let n = if name.len() > 11 { &name[..11] } else { name };
    draw_text(n, x + 4.0, y + l.ch * 0.17, l.ch * 0.11, WHITE);

    let text_fs = l.ch * 0.09;
    let line_h  = l.ch * 0.115;
    let text_start_y = y + l.ch * 0.30;
    for (i, line) in wrap_text(text, text_fs, l.cw - 8.0).iter().enumerate() {
        draw_text(line, x + 4.0, text_start_y + i as f32 * line_h, text_fs, WHITE);
    }

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

fn card_sub_view(c: &CardView) -> String {
    match c.card_type {
        CardType::Creature => {
            let base = c.power.unwrap_or(0) as i32;
            let effective = (base + c.power_counters).max(0) as u32;
            format!("PWR:{} DMG:{}", effective, c.damage)
        }
        CardType::Artifact => "Artifact".into(),
        CardType::Action   => "Action".into(),
        CardType::Upgrade  => "Upgrade".into(),
    }
}

fn can_use_card(active_house: Option<House>, card: &CardView) -> bool {
    match active_house {
        Some(h) => card.house == h,
        None => false,
    }
}

// ---------------------------------------------------------------------------
// Screens
// ---------------------------------------------------------------------------

enum Screen {
    Menu { status: String },
    ImportDeck {
        url: String,
        status: String,
        loading: Option<mpsc::Receiver<Result<SavedDeck, String>>>,
    },
    DecksList { decks: Vec<SavedDeck>, selected: Option<usize> },
    InGame(App),
}

fn try_connect(addr: &str) -> Result<App, String> {
    let sa = addr
        .to_socket_addrs()
        .map_err(|e| e.to_string())?
        .next()
        .ok_or_else(|| "Could not resolve address".to_string())?;
    TcpStream::connect_timeout(&sa, Duration::from_secs(3))
        .map(App::from_stream)
        .map_err(|e| e.to_string())
}

fn draw_menu(status: &str, mx: f32, my: f32, click: bool) -> Option<&'static str> {
    let sw = screen_width();
    let sh = screen_height();
    clear_background(Color::from_rgba(10, 30, 10, 255));

    let title_fs = (sw * 0.06).clamp(32.0, 72.0);
    let title_w  = measure_text("Keyforge", None, title_fs as u16, 1.0).width;
    draw_text("Keyforge", (sw - title_w) / 2.0, sh * 0.25, title_fs, GOLD);

    let bw = (sw * 0.22).clamp(160.0, 260.0);
    let bh = (sh * 0.07).clamp(36.0, 52.0);
    let bx = (sw - bw) / 2.0;
    let fs = bh * 0.44;
    let gap = bh * 0.35;

    let buttons: &[(&str, &str, Color, Color)] = &[
        ("Find Match",   "find",   Color::from_rgba(30, 100, 30,  255), Color::from_rgba(50, 160, 50,  255)),
        ("Decks",        "decks",  Color::from_rgba(30,  60, 120, 255), Color::from_rgba(50, 100, 180, 255)),
        ("Import Deck",  "import", Color::from_rgba(30,  60, 120, 255), Color::from_rgba(50, 100, 180, 255)),
        ("Exit",         "exit",   Color::from_rgba(100, 30,  30, 255), Color::from_rgba(160, 50,  50, 255)),
    ];

    let total_h = buttons.len() as f32 * bh + (buttons.len() - 1) as f32 * gap;
    let start_y = (sh - total_h) / 2.0 + sh * 0.05;

    let mut action = None;
    for (i, (label, id, bg_normal, bg_hover)) in buttons.iter().enumerate() {
        let by  = start_y + i as f32 * (bh + gap);
        let hov = in_box(mx, my, bx, by, bw, bh);
        let bg  = if hov { *bg_hover } else { *bg_normal };
        draw_rectangle(bx, by, bw, bh, bg);
        draw_rectangle_lines(bx, by, bw, bh, 2.0, LIGHTGRAY);
        let lw = measure_text(label, None, fs as u16, 1.0).width;
        draw_text(label, bx + (bw - lw) / 2.0, by + bh * 0.68, fs, WHITE);
        if click && hov { action = Some(*id); }
    }

    if !status.is_empty() {
        let tw = measure_text(status, None, 14, 1.0).width;
        draw_text(status, (sw - tw) / 2.0, start_y + total_h + 28.0, 14.0, YELLOW);
    }

    action
}

fn handle_text_input(text: &mut String) {
    while let Some(c) = get_char_pressed() {
        if !c.is_control() {
            text.push(c);
        }
    }
    if is_key_pressed(KeyCode::Backspace) && !text.is_empty() {
        text.pop();
    }
    let ctrl = is_key_down(KeyCode::LeftControl) || is_key_down(KeyCode::RightControl);
    if ctrl && is_key_pressed(KeyCode::V) {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            if let Ok(s) = cb.get_text() {
                text.push_str(&s);
            }
        }
    }
}

fn draw_import_screen(url: &mut String, status: &str, loading: bool,
                      mx: f32, my: f32, click: bool) -> Option<&'static str> {
    let sw = screen_width();
    let sh = screen_height();
    clear_background(Color::from_rgba(10, 20, 30, 255));

    let title_fs = (sw * 0.04).clamp(24.0, 48.0);
    draw_text("Import Deck", 30.0, 50.0, title_fs, GOLD);
    draw_text("Paste a KeyForge Vault URL:", 30.0, 90.0, 16.0, LIGHTGRAY);

    // URL input box
    let ix = 30.0;
    let iy = 108.0;
    let iw = sw - 60.0;
    let ih = 36.0;
    draw_rectangle(ix, iy, iw, ih, Color::from_rgba(20, 20, 40, 255));
    draw_rectangle_lines(ix, iy, iw, ih, 2.0, LIGHTGRAY);

    if !loading {
        handle_text_input(url);
    }

    // Clip displayed text to box width
    let url_fs = 15.0;
    let display: String = {
        let mut s = url.as_str();
        while !s.is_empty() && measure_text(s, None, url_fs as u16, 1.0).width > iw - 12.0 {
            s = &s[s.char_indices().nth(1).map(|(i, _)| i).unwrap_or(s.len())..];
        }
        s.to_string()
    };
    draw_text(&display, ix + 6.0, iy + ih * 0.68, url_fs, WHITE);

    let mut action = None;
    let bw = 120.0;
    let bh = 36.0;
    let by = iy + ih + 20.0;

    // Import button
    let imp_x = ix;
    let imp_col = if loading { Color::from_rgba(40, 40, 40, 255) } else { Color::from_rgba(30, 100, 30, 255) };
    let imp_hov = !loading && in_box(mx, my, imp_x, by, bw, bh);
    let imp_bg  = if imp_hov { Color::from_rgba(50, 160, 50, 255) } else { imp_col };
    draw_rectangle(imp_x, by, bw, bh, imp_bg);
    draw_rectangle_lines(imp_x, by, bw, bh, 2.0, LIGHTGRAY);
    draw_text("Import", imp_x + 28.0, by + bh * 0.68, 16.0, WHITE);
    if click && imp_hov { action = Some("import"); }

    // Back button
    let back_x = imp_x + bw + 16.0;
    let back_hov = in_box(mx, my, back_x, by, bw, bh);
    let back_bg  = if back_hov { Color::from_rgba(100, 50, 50, 255) } else { Color::from_rgba(60, 30, 30, 255) };
    draw_rectangle(back_x, by, bw, bh, back_bg);
    draw_rectangle_lines(back_x, by, bw, bh, 2.0, LIGHTGRAY);
    draw_text("Back", back_x + 36.0, by + bh * 0.68, 16.0, WHITE);
    if click && back_hov { action = Some("back"); }

    if !status.is_empty() {
        let col = if status.contains("success") || status.contains("imported") { GREEN } else { YELLOW };
        draw_text(status, ix, by + bh + 20.0, 15.0, col);
    }

    action
}

fn draw_decks_screen(decks: &[SavedDeck], selected: &mut Option<usize>,
                     mx: f32, my: f32, click: bool) -> Option<&'static str> {
    let sw = screen_width();
    let sh = screen_height();
    clear_background(Color::from_rgba(10, 20, 30, 255));

    let title_fs = (sw * 0.04).clamp(24.0, 48.0);
    draw_text("Your Decks", 30.0, 50.0, title_fs, GOLD);

    let row_h = 52.0;
    let row_x = 30.0;
    let row_w = sw - 60.0;
    let start_y = 80.0;

    if decks.is_empty() {
        draw_text("No decks imported yet.", row_x, start_y + 30.0, 16.0, GRAY);
    }

    for (i, deck) in decks.iter().enumerate() {
        let ry = start_y + i as f32 * (row_h + 6.0);
        let is_sel = *selected == Some(i);
        let hov = in_box(mx, my, row_x, ry, row_w, row_h);
        let bg = if is_sel  { Color::from_rgba(40, 80, 140, 255) }
                 else if hov { Color::from_rgba(25, 50,  90, 255) }
                 else        { Color::from_rgba(15, 30,  55, 255) };
        draw_rectangle(row_x, ry, row_w, row_h, bg);
        draw_rectangle_lines(row_x, ry, row_w, row_h, 1.5,
            if is_sel { GOLD } else { Color::from_rgba(60, 80, 120, 255) });

        draw_text(&deck.name, row_x + 10.0, ry + row_h * 0.42, 17.0, WHITE);
        let houses = deck.houses.join(" · ");
        draw_text(&houses, row_x + 10.0, ry + row_h * 0.78, 13.0, LIGHTGRAY);
        let card_count = format!("{} cards", deck.cards.len());
        let cw = measure_text(&card_count, None, 13, 1.0).width;
        draw_text(&card_count, row_x + row_w - cw - 10.0, ry + row_h * 0.60, 13.0, GRAY);

        if click && hov {
            *selected = Some(i);
        }
    }

    // Back button
    let bw = 120.0;
    let bh = 36.0;
    let bx = row_x;
    let by = sh - bh - 20.0;
    let back_hov = in_box(mx, my, bx, by, bw, bh);
    let back_bg  = if back_hov { Color::from_rgba(100, 50, 50, 255) } else { Color::from_rgba(60, 30, 30, 255) };
    draw_rectangle(bx, by, bw, bh, back_bg);
    draw_rectangle_lines(bx, by, bw, bh, 2.0, LIGHTGRAY);
    draw_text("Back", bx + 36.0, by + bh * 0.68, 16.0, WHITE);
    if click && back_hov { return Some("back"); }

    None
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
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:9999".to_string());
    let mut screen = Screen::Menu { status: String::new() };

    loop {
        let (mx, my) = mouse_position();
        let click    = is_mouse_button_pressed(MouseButton::Left);

        // ---- Menu screen ------------------------------------------------
        if let Screen::Menu { ref status } = screen {
            let status_clone = status.clone();
            let action = draw_menu(&status_clone, mx, my, click);
            match action {
                Some("exit")   => std::process::exit(0),
                Some("find")   => match try_connect(&addr) {
                    Ok(app) => screen = Screen::InGame(app),
                    Err(e)  => screen = Screen::Menu { status: format!("Could not connect: {e}") },
                },
                Some("import") => screen = Screen::ImportDeck {
                    url: String::new(), status: String::new(), loading: None,
                },
                Some("decks")  => screen = Screen::DecksList {
                    decks: deck_store::load(), selected: None,
                },
                _ => {}
            }
            next_frame().await;
            continue;
        }

        // ---- Import Deck screen -----------------------------------------
        if matches!(screen, Screen::ImportDeck { .. }) {
            // Poll background import thread.
            if let Screen::ImportDeck { ref mut status, ref mut loading, .. } = screen {
                if let Some(ref rx) = *loading {
                    match rx.try_recv() {
                        Ok(Ok(deck)) => {
                            deck_store::save_deck(deck);
                            *status = "Deck imported successfully!".into();
                            *loading = None;
                        }
                        Ok(Err(e)) => { *status = e; *loading = None; }
                        _ => {}
                    }
                }
            }
            let action = if let Screen::ImportDeck { ref mut url, ref status, ref loading } = screen {
                draw_import_screen(url, status, loading.is_some(), mx, my, click)
            } else { None };

            match action {
                Some("back") => screen = Screen::Menu { status: String::new() },
                Some("import") => {
                    if let Screen::ImportDeck { ref url, ref mut status, ref mut loading } = screen {
                        let url_clone = url.clone();
                        let (tx, rx) = mpsc::channel();
                        std::thread::spawn(move || { let _ = tx.send(vault::fetch_deck(&url_clone)); });
                        *loading = Some(rx);
                        *status = "Importing...".into();
                    }
                }
                _ => {}
            }
            next_frame().await;
            continue;
        }

        // ---- Decks List screen ------------------------------------------
        if matches!(screen, Screen::DecksList { .. }) {
            let action = if let Screen::DecksList { ref decks, ref mut selected } = screen {
                draw_decks_screen(decks, selected, mx, my, click)
            } else { None };
            if action == Some("back") {
                screen = Screen::Menu { status: String::new() };
            }
            next_frame().await;
            continue;
        }

        let app = match screen { Screen::InGame(ref mut a) => a, _ => unreachable!() };

        app.poll();
        clear_background(Color::from_rgba(20, 60, 20, 255));

        let l = L::new(screen_width(), screen_height());
        let lfs = l.ch * 0.11;

        let released = is_mouse_button_released(MouseButton::Left);
        let rclick   = is_mouse_button_pressed(MouseButton::Right);

        if rclick { app.deselect(); }

        let view = match &app.view {
            Some(v) => v,
            None => {
                draw_text(&app.msg, 20.0, 40.0, 20.0, WHITE);
                next_frame().await;
                continue;
            }
        };

        let my_turn = app.is_my_turn();

        // ---- zone labels -----------------------------------------------
        draw_text("OPPONENT  hand",       20.0, l.p1_hand_y - 4.0, lfs, GRAY);
        draw_text("OPPONENT  battleline", 20.0, l.p1_line_y - 4.0, lfs, GRAY);
        draw_text("OPPONENT  artifacts",  20.0, l.p1_art_y  - 4.0, lfs, GRAY);
        draw_line(0.0, l.divider_y, l.panel_x, l.divider_y, 2.0, DARKGRAY);
        draw_text("YOUR  artifacts",      20.0, l.p0_art_y  - 4.0, lfs, GRAY);
        draw_text("YOUR  battleline",     20.0, l.p0_line_y - 4.0, lfs, GRAY);
        draw_text("YOUR  hand",           20.0, l.p0_hand_y - 4.0, lfs, GRAY);

        // ---- Opponent hand (face-down) ----------------------------------
        for i in 0..view.opp_hand_count {
            draw_card(&l, l.cx(i), l.p1_hand_y,
                "?", "", "", Color::from_rgba(25, 25, 80, 255), GRAY);
        }
        draw_zone(&l, l.deck_x, l.p1_hand_y, "Deck",    view.opp_deck_count,      false);
        draw_zone(&l, l.arch_x, l.p1_hand_y, "Archive", view.opp_archives_count,   false);
        draw_zone(&l, l.disc_x, l.p1_hand_y, "Discard", view.opp_discard.len(),    false);

        // ---- Opponent battleline ----------------------------------------
        let opp_count = view.opp_battleline.len();
        for (i, card) in view.opp_battleline.iter().enumerate() {
            let x = l.blx(i, opp_count);
            let selected = app.selected_creature == Some(card.id);
            let bg = if card.exhausted { Color::from_rgba(70, 15, 15, 255) }
                     else              { Color::from_rgba(160, 30, 30, 255) };
            let border = if selected { YELLOW } else { DARKGRAY };
            draw_card(&l, x, l.p1_line_y, &card.name,
                &card.text, &card_sub_view(card), bg, border);
            if i == 0 {
                draw_flank_badge(&l, x, l.p1_line_y, "◄L", Color::from_rgba(255, 160, 160, 255));
            }
            if i == opp_count - 1 {
                draw_flank_badge(&l, x + l.cw - l.cw * 0.26, l.p1_line_y,
                    "R►", Color::from_rgba(255, 160, 160, 255));
            }
            if click && l.hit(mx, my, x, l.p1_line_y) {
                if let Some(att) = app.selected_creature {
                    let own_ids: Vec<CardId> = view.my_battleline.iter().map(|c| c.id).collect();
                    if own_ids.contains(&att) && my_turn {
                        app.send(&ClientMessage::Attack {
                            attacker_id: att,
                            defender_id: card.id,
                        });
                        app.selected_creature = None;
                        app.msg = "Attack sent.".into();
                    }
                }
            }
        }

        // ---- Opponent artifacts -----------------------------------------
        draw_artifact_row(&l, &view.opp_artifacts, false, l.p1_art_y);

        // ---- Own battleline ---------------------------------------------
        let my_count = view.my_battleline.len();
        for (i, card) in view.my_battleline.iter().enumerate() {
            let x = l.blx(i, my_count);
            let selected = app.selected_creature == Some(card.id);
            let bg = if card.exhausted { Color::from_rgba(15, 60, 15, 255) }
                     else              { Color::from_rgba(25, 130, 25, 255) };
            let border = if selected { YELLOW } else { DARKGRAY };
            draw_card(&l, x, l.p0_line_y, &card.name,
                &card.text, &card_sub_view(card), bg, border);
            if i == 0 {
                draw_flank_badge(&l, x, l.p0_line_y, "◄L", Color::from_rgba(160, 255, 160, 255));
            }
            if i == my_count - 1 {
                draw_flank_badge(&l, x + l.cw - l.cw * 0.26, l.p0_line_y,
                    "R►", Color::from_rgba(160, 255, 160, 255));
            }
            if click && l.hit(mx, my, x, l.p0_line_y) && my_turn {
                if view.active_house.is_none() {
                    app.msg = "Choose a house first.".into();
                } else if !can_use_card(view.active_house, card) {
                    app.msg = format!(
                        "{} is not a {:?} card — choose its house to use it.",
                        card.name, card.house);
                } else if selected {
                    if !card.exhausted {
                        app.send(&ClientMessage::Reap { card_id: card.id });
                        app.selected_creature = None;
                        app.msg = "Reap sent.".into();
                    }
                } else {
                    app.selected_creature = Some(card.id);
                    app.selected_hand = None;
                    app.msg = "Creature selected — click again to reap, click enemy to attack.".into();
                }
            }
        }

        // ---- Own artifacts ----------------------------------------------
        draw_artifact_row(&l, &view.my_artifacts, true, l.p0_art_y);

        // ---- play drop-zones --------------------------------------------
        let active_card_id = app.selected_hand.or(app.drag_card);
        if active_card_id.is_some() {
            let find_card = active_card_id.and_then(|id| view.my_hand.iter().find(|c| c.id == id));
            let is_artifact = find_card.is_some_and(|c| c.card_type == CardType::Artifact);

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
                       else if on_art   { Some(Flank::Right) }
                       else             { None };
                if let (Some(flank), Some(id)) = (flank, app.selected_hand) {
                    app.send(&ClientMessage::PlayCard { card_id: id, flank });
                    app.selected_hand = None;
                    app.msg = "Play sent.".into();
                }
            }
        }

        // ---- Own hand ---------------------------------------------------
        for (i, card) in view.my_hand.iter().enumerate() {
            let selected = app.selected_hand == Some(card.id);
            let playable = can_use_card(view.active_house, card);
            let bg = if selected        { Color::from_rgba(180, 140, 0, 255) }
                     else if playable   { Color::from_rgba(30, 60, 160, 255) }
                     else               { Color::from_rgba(20, 20, 50, 255) };
            let border = if selected { WHITE } else if playable { GRAY } else { DARKGRAY };
            draw_card(&l, l.cx(i), l.p0_hand_y,
                &card.name, &card.text, &card_sub_view(card), bg, border);
            if click && l.hit(mx, my, l.cx(i), l.p0_hand_y) && my_turn {
                app.drag_card = Some(card.id);
                app.selected_hand = None;
                app.selected_creature = None;
            }
        }

        // ---- Own side zones ---------------------------------------------
        let dragging_mine = app.drag_card.is_some() && my_turn;
        draw_zone(&l, l.deck_x, l.p0_hand_y, "Deck",    view.my_deck_count,         false);
        draw_zone(&l, l.arch_x, l.p0_hand_y, "Archive", view.my_archives.len(),     false);
        draw_zone(&l, l.disc_x, l.p0_hand_y, "Discard", view.my_discard.len(),      dragging_mine);

        // ================================================================
        // Right panel
        // ================================================================
        let sw = screen_width();
        let sh = screen_height();
        draw_rectangle(l.panel_x, 0.0, sw - l.panel_x, sh, Color::from_rgba(12, 12, 28, 255));

        let px = l.panel_x + 8.0;

        draw_text(&format!("Turn {}", view.turn), px, 28.0, 20.0, WHITE);
        let active_lbl = if my_turn { "You" } else { "Opponent" };
        draw_text(&format!("Active: {}", active_lbl), px, 50.0, 16.0, LIGHTGRAY);

        let ah_lbl = match view.active_house {
            Some(House::Brobnar) => "Brobnar",
            Some(House::Dis)     => "Dis",
            Some(House::Shadows) => "Shadows",
            _                    => "—",
        };
        draw_text(&format!("House:  {}", ah_lbl), px, 70.0, 16.0, LIGHTGRAY);
        draw_line(l.panel_x, 80.0, sw, 80.0, 1.0, DARKGRAY);

        let my_keys = view.my_player.keys.iter().filter(|k| k.forged).count();
        let opp_keys = view.opp_player.keys.iter().filter(|k| k.forged).count();

        draw_text("You", px, 100.0, 15.0, GREEN);
        draw_text(&format!("Aember: {}", view.my_player.aember_pool), px, 118.0, 14.0, GOLD);
        draw_text(&format!("Keys:   {}/3", my_keys), px, 134.0, 14.0, GOLD);

        draw_text("Opponent", px, 155.0, 15.0, RED);
        draw_text(&format!("Aember: {}", view.opp_player.aember_pool), px, 173.0, 14.0, ORANGE);
        draw_text(&format!("Keys:   {}/3", opp_keys), px, 189.0, 14.0, ORANGE);

        draw_line(l.panel_x, 202.0, sw, 202.0, 1.0, DARKGRAY);

        draw_text("Choose house:", px, 220.0, 14.0, LIGHTGRAY);
        let houses = [(House::Brobnar, "Brobnar"), (House::Dis, "Dis"), (House::Shadows, "Shadows")];
        for (i, (h, label)) in houses.iter().enumerate() {
            let by = 228.0 + i as f32 * 42.0;
            let active = view.active_house == Some(*h);
            if btn(px, by, 128.0, 32.0, label, active, mx, my, click) && my_turn {
                app.send(&ClientMessage::ChooseHouse { house: *h, pick_up_archives: false });
                app.msg = format!("House {} chosen.", label);
            }
        }

        draw_line(l.panel_x, 360.0, sw, 360.0, 1.0, DARKGRAY);

        if btn(px, 370.0, 128.0, 36.0, "End Turn", false, mx, my, click) && my_turn {
            app.send(&ClientMessage::EndTurn);
            app.selected_hand = None;
            app.selected_creature = None;
            app.drag_card = None;
            app.msg = "End turn sent.".into();
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
        if let Some(winner) = app.game_over {
            let ow = l.panel_x * 0.7;
            let ox = (l.panel_x - ow) / 2.0;
            let oy = sh / 2.0 - 65.0;
            draw_rectangle(ox, oy, ow, 130.0, Color::from_rgba(0, 0, 0, 210));
            let is_me = view.my_index == winner;
            let (txt, col) = if is_me { ("YOU WIN!", GOLD) } else { ("YOU LOSE!", RED) };
            draw_text(txt, ox + 20.0, oy + 80.0, l.ch * 0.62, col);
        }

        // ---- drag ghost -------------------------------------------------
        if let Some(drag_id) = app.drag_card {
            if is_mouse_button_down(MouseButton::Left) {
                if let Some(card) = view.my_hand.iter().find(|c| c.id == drag_id) {
                    draw_card(&l, mx - l.cw / 2.0, my - l.ch / 2.0,
                        &card.name, &card.text, &card_sub_view(card),
                        Color::from_rgba(200, 180, 60, 210), WHITE);
                }
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

                let on_discard    = in_box(mx, my, l.disc_x, l.p0_hand_y, l.zone_w, l.ch) && my_turn;
                let on_artifact   = in_box(mx, my, 20.0, l.p0_art_y, art_zone_w, l.art_h);
                let on_left_flank = in_box(mx, my, lx, flank_zy, half, flank_zh);
                let on_right_flank= in_box(mx, my, rx, flank_zy, half, flank_zh);

                if on_discard {
                    app.send(&ClientMessage::DiscardFromHand { card_id: drag_id });
                    app.msg = "Discard sent.".into();
                } else if on_artifact || on_left_flank || on_right_flank {
                    let flank = if on_left_flank { Flank::Left } else { Flank::Right };
                    app.send(&ClientMessage::PlayCard { card_id: drag_id, flank });
                    app.msg = "Play sent.".into();
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
