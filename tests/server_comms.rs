/// Integration tests for server TCP communications.
///
/// Each test spins up a real game server (`run_session`) in a background
/// thread, connects two client streams, and exercises the full
/// request/response cycle over the wire.
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use keyforge::card::{BonusIcon, House};
use keyforge::protocol::{ClientGameView, ClientMessage, ServerMessage};
use keyforge::server::run_session;
use keyforge::zones::Flank;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Bind to an OS-assigned free port and return the listener + port number.
fn bind_free() -> (TcpListener, u16) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    (l, port)
}

fn tcp_connect(port: u16) -> TcpStream {
    TcpStream::connect(format!("127.0.0.1:{}", port)).unwrap()
}

fn send(stream: &mut TcpStream, msg: &ClientMessage) {
    let mut line = serde_json::to_string(msg).unwrap();
    line.push('\n');
    stream.write_all(line.as_bytes()).unwrap();
    stream.flush().unwrap();
}

fn recv(reader: &mut BufReader<TcpStream>) -> ServerMessage {
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    serde_json::from_str(line.trim()).expect("invalid server message")
}


fn expect_game_state(msg: ServerMessage) -> ClientGameView {
    match msg {
        ServerMessage::GameState(v) => v,
        other => panic!("expected GameState, got {:?}", other),
    }
}

/// Spawn `run_session` for one pair of clients in a background thread.
/// Returns a channel that receives `()` when the session ends.
fn spawn_session(listener: TcpListener) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let (s0, _) = listener.accept().unwrap();
        let (s1, _) = listener.accept().unwrap();
        run_session(s0, s1);
        let _ = tx.send(());
    });
    rx
}

/// Spawn a server that handles `n` sequential sessions.
fn spawn_n_sessions(listener: TcpListener, n: usize) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for _ in 0..n {
            let (s0, _) = listener.accept().unwrap();
            let (s1, _) = listener.accept().unwrap();
            run_session(s0, s1);
        }
        let _ = tx.send(());
    });
    rx
}

/// Connect two clients and consume welcome + initial game-state messages.
/// Returns `(stream0, reader0, view0, stream1, reader1, view1)`.
fn handshake(port: u16) -> (TcpStream, BufReader<TcpStream>, ClientGameView,
                             TcpStream, BufReader<TcpStream>, ClientGameView) {
    let s0 = tcp_connect(port);
    let s1 = tcp_connect(port);
    let mut r0 = BufReader::new(s0.try_clone().unwrap());
    let mut r1 = BufReader::new(s1.try_clone().unwrap());
    recv(&mut r0); // Welcome{0}
    recv(&mut r1); // Welcome{1}
    let v0 = expect_game_state(recv(&mut r0));
    let v1 = expect_game_state(recv(&mut r1));
    (s0, r0, v0, s1, r1, v1)
}

// ---------------------------------------------------------------------------
// Matchmaking
// ---------------------------------------------------------------------------

/// Server sends Welcome{player_index:0} to the first connector and
/// Welcome{player_index:1} to the second.
#[test]
fn test_matchmaking_correct_player_indices() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let s0 = tcp_connect(port);
    let s1 = tcp_connect(port);
    let mut r0 = BufReader::new(s0);
    let mut r1 = BufReader::new(s1);

    let w0 = recv(&mut r0);
    let w1 = recv(&mut r1);

    assert!(matches!(w0, ServerMessage::Welcome { player_index: 0 }));
    assert!(matches!(w1, ServerMessage::Welcome { player_index: 1 }));
}

/// After both players connect each receives an initial GameState.
#[test]
fn test_matchmaking_initial_state_sent_to_both() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let s0 = tcp_connect(port);
    let s1 = tcp_connect(port);
    let mut r0 = BufReader::new(s0);
    let mut r1 = BufReader::new(s1);

    recv(&mut r0); // Welcome
    recv(&mut r1);

    let v0 = expect_game_state(recv(&mut r0));
    let v1 = expect_game_state(recv(&mut r1));

    assert_eq!(v0.my_index, 0);
    assert_eq!(v1.my_index, 1);
    assert_eq!(v0.active_player, 0);
    assert_eq!(v0.turn, 1);
    // Setup draw: P0 draws 7, P1 draws 6.
    assert_eq!(v0.my_hand.len(), 7);
    assert_eq!(v1.my_hand.len(), 6);
    assert_eq!(v0.opp_hand_count, v1.my_hand.len());
    assert_eq!(v1.opp_hand_count, v0.my_hand.len());
}

/// Opponent's hand is hidden (count only) in each player's view.
#[test]
fn test_matchmaking_opponent_hand_hidden() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (_, _r0, v0, _, _r1, v1) = handshake(port);

    assert_eq!(v0.opp_hand_count, v1.my_hand.len());
    assert_eq!(v1.opp_hand_count, v0.my_hand.len());
    // my_hand is always a full CardView slice
    assert!(!v0.my_hand.is_empty());
    assert!(!v1.my_hand.is_empty());
}

// ---------------------------------------------------------------------------
// Game handling — actions
// ---------------------------------------------------------------------------

/// ChooseHouse broadcasts an updated GameState with the chosen house set.
#[test]
fn test_game_choose_house_broadcasts_update() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, _, mut r1, _) = handshake(port);

    send(&mut s0, &ClientMessage::ChooseHouse { house: House::Brobnar, pick_up_archives: false });

    let v0 = expect_game_state(recv(&mut r0));
    let v1 = expect_game_state(recv(&mut r1));

    assert_eq!(v0.active_house, Some(House::Brobnar));
    assert_eq!(v1.active_house, Some(House::Brobnar));
}

/// Playing a card removes it from hand and places it on the battleline.
#[test]
fn test_game_play_card_updates_battleline() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, _, mut r1, _) = handshake(port);

    send(&mut s0, &ClientMessage::ChooseHouse { house: House::Brobnar, pick_up_archives: false });
    let v0 = expect_game_state(recv(&mut r0));
    recv(&mut r1);

    let card = v0.my_hand.iter()
        .find(|c| c.house == House::Brobnar)
        .expect("no Brobnar card in hand");
    let id = card.id;

    send(&mut s0, &ClientMessage::PlayCard { card_id: id, flank: Flank::Left });
    let v0 = expect_game_state(recv(&mut r0));

    assert!(!v0.my_hand.iter().any(|c| c.id == id), "card still in hand after play");
    assert!(v0.my_battleline.iter().any(|c| c.id == id), "card not on battleline after play");
}

/// Reaping a creature exhausts it and grants aember (1 base + bonus icons).
#[test]
fn test_game_reap_grants_aember_and_exhausts() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, _, mut r1, _) = handshake(port);

    // Choose Brobnar.
    send(&mut s0, &ClientMessage::ChooseHouse { house: House::Brobnar, pick_up_archives: false });
    let v0 = expect_game_state(recv(&mut r0));
    recv(&mut r1);

    // Play a Brobnar creature with an aember bonus icon (Troll).
    let card = v0.my_hand.iter()
        .find(|c| c.house == House::Brobnar && c.bonus_icons.contains(&BonusIcon::Aember))
        .expect("no Brobnar card with aember bonus icon in hand");
    let id = card.id;
    send(&mut s0, &ClientMessage::PlayCard { card_id: id, flank: Flank::Left });
    recv(&mut r0);
    recv(&mut r1);

    // Reap with it.
    send(&mut s0, &ClientMessage::Reap { card_id: id });
    let v0 = expect_game_state(recv(&mut r0));

    let creature = v0.my_battleline.iter().find(|c| c.id == id).unwrap();
    assert!(creature.exhausted, "creature should be exhausted after reaping");
    // 1 base reap + 1 bonus icon (Troll has BonusIcon::Aember)
    assert!(v0.my_player.aember_pool >= 2, "expected at least 2 aember after reap");
}

/// EndTurn increments the turn counter and switches the active player in both views.
#[test]
fn test_game_end_turn_switches_player_and_increments_turn() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, _, mut r1, _) = handshake(port);

    send(&mut s0, &ClientMessage::EndTurn);

    let v0 = expect_game_state(recv(&mut r0));
    let v1 = expect_game_state(recv(&mut r1));

    assert_eq!(v0.active_player, 1);
    assert_eq!(v1.active_player, 1);
    assert_eq!(v0.turn, 2);
    assert_eq!(v1.turn, 2);
}

/// Sending an invalid card id (not in hand) returns an Error; no GameState is sent.
#[test]
fn test_game_invalid_action_returns_error_not_game_state() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, _, _, _) = handshake(port);

    send(&mut s0, &ClientMessage::PlayCard { card_id: 999_999, flank: Flank::Left });

    assert!(matches!(recv(&mut r0), ServerMessage::Error(_)));
}

// ---------------------------------------------------------------------------
// Game handling — inactive player
// ---------------------------------------------------------------------------

/// Messages from the inactive player (other than Surrender) are ignored.
/// The active player's turn is unaffected.
#[test]
fn test_game_inactive_player_non_surrender_ignored() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, mut s1, mut r1, _) = handshake(port);

    // P1 sends EndTurn while P0 is still active — server ignores it.
    send(&mut s1, &ClientMessage::EndTurn);

    // P0 ends their turn legitimately.
    send(&mut s0, &ClientMessage::EndTurn);
    let v0 = expect_game_state(recv(&mut r0));
    let _v1 = expect_game_state(recv(&mut r1));

    // Only P0's EndTurn was processed: turn is 2, P1 is now active.
    assert_eq!(v0.turn, 2);
    assert_eq!(v0.active_player, 1);
}

// ---------------------------------------------------------------------------
// Game handling — win condition
// ---------------------------------------------------------------------------

/// EndTurn with no win condition → GameState, not GameOver.
#[test]
fn test_game_no_premature_game_over() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, _, _, _) = handshake(port);

    send(&mut s0, &ClientMessage::EndTurn);
    assert!(matches!(recv(&mut r0), ServerMessage::GameState(_)));
}

// ---------------------------------------------------------------------------
// Disconnect handling
// ---------------------------------------------------------------------------

/// Dropping P0's connection causes the session to exit cleanly (no panic).
#[test]
fn test_disconnect_p0_ends_session_cleanly() {
    let (listener, port) = bind_free();
    let done = spawn_session(listener);

    let (s0, r0, _, _, r1, _) = handshake(port);

    drop(s0); // disconnect P0
    drop(r0);

    done.recv_timeout(Duration::from_secs(3))
        .expect("server did not exit after P0 disconnected");
    let _ = r1;
}

/// Dropping P1's connection causes the session to exit cleanly.
#[test]
fn test_disconnect_p1_ends_session_cleanly() {
    let (listener, port) = bind_free();
    let done = spawn_session(listener);

    let (mut s0, mut r0, _, s1, mut r1, _) = handshake(port);

    // P0 ends turn so P1 becomes active — server will next read from P1.
    send(&mut s0, &ClientMessage::EndTurn);
    recv(&mut r0);
    recv(&mut r1);

    drop(s1); // disconnect P1 while it's their turn
    drop(r1);

    done.recv_timeout(Duration::from_secs(3))
        .expect("server did not exit after P1 disconnected");
    let _ = r0;
}

/// Surrender from the active player immediately ends the match: both players
/// receive GameOver with the surrendering player as the loser.
#[test]
fn test_surrender_ends_match_immediately() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, _, mut r1, _) = handshake(port);

    send(&mut s0, &ClientMessage::Surrender);

    let msg0 = recv(&mut r0);
    let msg1 = recv(&mut r1);

    assert!(matches!(msg0, ServerMessage::GameOver { winner: 1 }),
        "surrendering player should lose: {:?}", msg0);
    assert!(matches!(msg1, ServerMessage::GameOver { winner: 1 }),
        "opponent should be declared winner: {:?}", msg1);
}

/// Surrender from the inactive player is processed immediately — P0 does not
/// need to take any action first.
#[test]
fn test_surrender_inactive_player_immediate() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (_, mut r0, _, mut s1, mut r1, _) = handshake(port);

    // P1 surrenders while inactive — server handles it in real time.
    send(&mut s1, &ClientMessage::Surrender);

    let msg0 = recv(&mut r0);
    let msg1 = recv(&mut r1);

    assert!(matches!(msg0, ServerMessage::GameOver { winner: 0 }),
        "P0 should be declared winner: {:?}", msg0);
    assert!(matches!(msg1, ServerMessage::GameOver { winner: 0 }),
        "surrendering P1 should lose: {:?}", msg1);
}

/// When P0 (active player) disconnects, the surviving player P1 receives
/// GameOver declaring P1 the winner.
#[test]
fn test_disconnect_active_player_sends_gameover_to_survivor() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (s0, r0, _, _, mut r1, _) = handshake(port);

    drop(s0);
    drop(r0);

    let msg = recv(&mut r1);
    assert!(
        matches!(msg, ServerMessage::GameOver { winner: 1 }),
        "expected GameOver{{winner:1}}, got {:?}", msg
    );
}

/// When the inactive player P1 disconnects (detected during state broadcast),
/// P0 receives GameOver declaring P0 the winner.
#[test]
fn test_disconnect_inactive_player_sends_gameover_to_survivor() {
    let (listener, port) = bind_free();
    spawn_session(listener);

    let (mut s0, mut r0, _, s1, r1, _) = handshake(port);

    // Drop P1 while P0 is still active.
    drop(s1);
    drop(r1);

    // P0 acts — server broadcasts state, detects P1 is gone, sends GameOver to P0.
    send(&mut s0, &ClientMessage::EndTurn);

    let msg = recv(&mut r0);
    assert!(
        matches!(msg, ServerMessage::GameOver { winner: 0 }),
        "expected GameOver{{winner:0}}, got {:?}", msg
    );
}

// ---------------------------------------------------------------------------
// Persistence — server stays up after a game ends
// ---------------------------------------------------------------------------

/// After one game session ends (both clients disconnect), the server accepts
/// a second pair of clients and matches them into a new game.
#[test]
fn test_server_accepts_second_match_after_first_ends() {
    let (listener, port) = bind_free();
    let done = spawn_n_sessions(listener, 2);

    // Game 1: connect and immediately disconnect.
    {
        let s0 = tcp_connect(port);
        let s1 = tcp_connect(port);
        let mut r0 = BufReader::new(s0);
        let mut r1 = BufReader::new(s1);
        recv(&mut r0); // Welcome
        recv(&mut r1);
        recv(&mut r0); // GameState
        recv(&mut r1);
        // Streams drop here → session 1 ends.
    }

    // Game 2: fresh pair connects and gets welcomed.
    let s0 = tcp_connect(port);
    let s1 = tcp_connect(port);
    let mut r0 = BufReader::new(s0);
    let mut r1 = BufReader::new(s1);

    let w0 = recv(&mut r0);
    let w1 = recv(&mut r1);

    assert!(matches!(w0, ServerMessage::Welcome { player_index: 0 }));
    assert!(matches!(w1, ServerMessage::Welcome { player_index: 1 }));

    // Both get an initial GameState for game 2.
    assert!(matches!(recv(&mut r0), ServerMessage::GameState(_)));
    assert!(matches!(recv(&mut r1), ServerMessage::GameState(_)));

    drop(r0);
    drop(r1);
    // Let the server thread exit so test harness doesn't leak threads.
    let _ = done.recv_timeout(Duration::from_secs(3));
}

/// Three sequential games all get correct initial state, confirming the server
/// resets cleanly between sessions.
#[test]
fn test_server_resets_game_state_between_sessions() {
    let (listener, port) = bind_free();
    spawn_n_sessions(listener, 3);

    for game_no in 0..3u32 {
        let s0 = tcp_connect(port);
        let s1 = tcp_connect(port);
        let mut r0 = BufReader::new(s0);
        let mut r1 = BufReader::new(s1);

        recv(&mut r0); // Welcome
        recv(&mut r1);
        let v0 = expect_game_state(recv(&mut r0));
        let v1 = expect_game_state(recv(&mut r1));

        assert_eq!(v0.turn, 1, "game {} should start at turn 1", game_no + 1);
        assert_eq!(v0.active_player, 0, "game {} P0 should be active first", game_no + 1);
        assert_eq!(v0.my_player.aember_pool, 0);
        assert_eq!(v1.my_player.aember_pool, 0);
        // Streams drop → session ends, server loops.
    }
}
