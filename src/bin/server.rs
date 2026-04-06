use std::net::TcpListener;

use keyforge::server::run_session;

fn main() {
    let addr = std::env::args().nth(1).unwrap_or_else(|| "127.0.0.1:9999".to_string());
    let listener = TcpListener::bind(&addr).expect("bind");
    println!("Keyforge server listening on {}", addr);

    loop {
        println!("Waiting for player 0...");
        let (stream0, peer0) = listener.accept().expect("accept p0");
        println!("Player 0 connected from {}", peer0);

        println!("Waiting for player 1...");
        let (stream1, peer1) = listener.accept().expect("accept p1");
        println!("Player 1 connected from {}", peer1);

        run_session(stream0, stream1);
        println!("Game finished. Waiting for next match...");
    }
}
