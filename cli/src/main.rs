use briscola_ai::mc::{MonteCarloConfig, choose_best_move};
use briscola_ai::rng::FastRng;
use briscola_core::bitset::add;
use briscola_core::card::{Card, INITIAL_TALON_SIZE, Rank, Suit};
use briscola_core::state::{Player, PublicGameState};

fn main() {
    let my_hand = vec![
        Card::new(Suit::Coins, Rank::Ace),
        Card::new(Suit::Clubs, Rank::Two),
        Card::new(Suit::Swords, Rank::King),
    ];
    let face_up = Card::new(Suit::Clubs, Rank::Four);

    let mut seen_cards = add(0, face_up);
    for card in &my_hand {
        seen_cards = add(seen_cards, *card);
    }

    let state = PublicGameState {
        my_hand,
        opp_played: None,
        briscola_suit: Suit::Clubs,
        talon_len: INITIAL_TALON_SIZE,
        last_face_up_trump: face_up,
        seen_cards,
        score_me: 0,
        score_opp: 0,
        leader: Player::Me,
    };

    let mut rng = FastRng::new(42);
    let result = match choose_best_move(&state, &mut rng, MonteCarloConfig { samples_per_move: 64 })
    {
        Ok(result) => result,
        Err(error) => {
            eprintln!("cannot compute best move: {error:?}");
            std::process::exit(1);
        }
    };

    println!("best move: {:?}", result.best_move);
    for stats in result.moves {
        println!(
            "candidate {:?}: p_win={:.3} score_delta={:.2} samples={}",
            stats.card, stats.p_win, stats.expected_score_delta, stats.n_samples
        );
    }
}
