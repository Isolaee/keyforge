use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use keyforge::card::CardDef;
use keyforge::cards;
use keyforge::deck;
use keyforge::game::{
    attack, choose_house, end_turn, play_card, play_card_deployed, reap, step_forge_key, unstun,
    GameState,
};
use keyforge::protocol::{ClientMessage, ServerMessage};
use keyforge::view::to_client_view;

fn send(stream: &mut TcpStream, msg: &ServerMessage) {
    let mut line = serde_json::to_string(msg).expect("serialize");
    line.push('\n');
    stream.write_all(line.as_bytes()).expect("write");
    stream.flush().expect("flush");
}

fn recv(reader: &mut BufReader<TcpStream>) -> Option<ClientMessage> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => None,
        Ok(_) => serde_json::from_str(line.trim()).ok(),
        Err(_) => None,
    }
}

fn build_game() -> GameState {
    let p0: &[&'static CardDef] = &[
        &cards::TROLL,
        &cards::SMAAASH,
        &cards::SILVERTOOTH,
        &cards::VEZYMA_THINKDRONE,
        &cards::PLAGUE,
        &cards::BANNER_OF_BATTLE,
        &cards::TROLL,
        &cards::SMAAASH,
    ];
    let p1: &[&'static CardDef] = &[
        &cards::TROLL,
        &cards::SILVERTOOTH,
        &cards::SMAAASH,
        &cards::VEZYMA_THINKDRONE,
        &cards::PLAGUE,
        &cards::BANNER_OF_BATTLE,
        &cards::SILVERTOOTH,
        &cards::TROLL,
    ];
    let (mut all, ids0) = deck::build_deck(p0);
    let (cards1, ids1) = deck::build_deck(p1);
    all.extend(cards1);
    GameState::new(ids0, ids1, all)
}

fn send_views(game: &GameState, streams: &mut [TcpStream; 2]) {
    for i in 0..2 {
        let view = to_client_view(game, i);
        send(&mut streams[i], &ServerMessage::GameState(view));
    }
}

fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:9999".to_string());
    let listener = TcpListener::bind(&addr).expect("bind");
    println!("Keyforge server listening on {}", addr);

    // Accept exactly 2 connections.
    println!("Waiting for player 0...");
    let (stream0, peer0) = listener.accept().expect("accept p0");
    println!("Player 0 connected from {}", peer0);

    println!("Waiting for player 1...");
    let (stream1, peer1) = listener.accept().expect("accept p1");
    println!("Player 1 connected from {}", peer1);

    let mut streams = [
        stream0.try_clone().expect("clone"),
        stream1.try_clone().expect("clone"),
    ];
    let mut readers = [
        BufReader::new(stream0),
        BufReader::new(stream1),
    ];

    // Send welcome messages.
    send(&mut streams[0], &ServerMessage::Welcome { player_index: 0 });
    send(&mut streams[1], &ServerMessage::Welcome { player_index: 1 });

    let mut game = build_game();

    // Send initial game state.
    send_views(&game, &mut streams);

    // Game loop.
    loop {
        let ap = game.active_player;

        let msg = match recv(&mut readers[ap]) {
            Some(m) => m,
            None => {
                println!("Player {} disconnected.", ap);
                break;
            }
        };

        println!("P{}: {:?}", ap, msg);

        let result: Result<(), String> = match msg {
            ClientMessage::ChooseHouse { house, pick_up_archives } => {
                choose_house(&mut game, house, pick_up_archives);
                Ok(())
            }
            ClientMessage::PlayCard { card_id, flank } => {
                if !game.players[ap].zones.hand.contains(&card_id) {
                    Err("Card not in hand".into())
                } else {
                    play_card(&mut game, card_id, flank);
                    Ok(())
                }
            }
            ClientMessage::PlayCardDeployed { card_id, index } => {
                if !game.players[ap].zones.hand.contains(&card_id) {
                    Err("Card not in hand".into())
                } else {
                    play_card_deployed(&mut game, card_id, index);
                    Ok(())
                }
            }
            ClientMessage::Reap { card_id } => {
                let on_line = game.players[ap].zones.battleline.creature_ids().contains(&card_id);
                if !on_line {
                    Err("Creature not on your battleline".into())
                } else {
                    reap(&mut game, card_id);
                    Ok(())
                }
            }
            ClientMessage::Attack { attacker_id, defender_id } => {
                let own = game.players[ap].zones.battleline.creature_ids();
                let opp = game.players[1 - ap].zones.battleline.creature_ids();
                if !own.contains(&attacker_id) {
                    Err("Attacker not on your battleline".into())
                } else if !opp.contains(&defender_id) {
                    Err("Defender not on opponent battleline".into())
                } else {
                    attack(&mut game, attacker_id, defender_id);
                    Ok(())
                }
            }
            ClientMessage::Unstun { card_id } => {
                let on_line = game.players[ap].zones.battleline.creature_ids().contains(&card_id);
                if !on_line {
                    Err("Creature not on your battleline".into())
                } else {
                    unstun(&mut game, card_id);
                    Ok(())
                }
            }
            ClientMessage::DiscardFromHand { card_id } => {
                if !game.players[ap].zones.hand.contains(&card_id) {
                    Err("Card not in hand".into())
                } else {
                    game.players[ap].zones.discard_from_hand(card_id);
                    Ok(())
                }
            }
            ClientMessage::EndTurn => {
                end_turn(&mut game);
                let new_ap = game.active_player;
                step_forge_key(&mut game.players[new_ap].player);
                Ok(())
            }
        };

        if let Err(e) = result {
            send(&mut streams[ap], &ServerMessage::Error(e));
            continue;
        }

        // Check win condition.
        for i in 0..2 {
            if game.players[i].player.keys.has_won() {
                let winner_msg = ServerMessage::GameOver { winner: i };
                send(&mut streams[0], &winner_msg);
                send(&mut streams[1], &winner_msg);
                println!("Player {} wins!", i);
                return;
            }
        }

        send_views(&game, &mut streams);
    }
}
