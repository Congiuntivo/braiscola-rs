use std::io::{self, Write};
use std::path::PathBuf;

use briscola_ai::mc::MoveStats;
use briscola_ai::rng::FastRng;
use briscola_core::card::{Card, HAND_SIZE, INITIAL_TALON_SIZE, Suit};
use briscola_core::rules::{TrickWinner, trick_winner};
use briscola_core::state::Player;
use cli::advisor::{
    InteractiveInit, InteractiveSession, format_card, parse_card, parse_card_list, parse_player,
    parse_suit, suggest_from_json_path,
};
use cli::card_art::AsciiCardRenderer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Interactive,
    Suggest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliConfig {
    mode: Mode,
    json_path: Option<PathBuf>,
    samples_per_move: usize,
    seed: u64,
}

const USAGE: &str = "usage:\n  advisor interactive [--samples N] [--seed N]\n  advisor suggest --json PATH [--samples N] [--seed N]";

fn parse_cli() -> Result<CliConfig, String> {
    let mut args = std::env::args().skip(1);
    let mode = match args.next().as_deref() {
        Some("interactive") => Mode::Interactive,
        Some("suggest") => Mode::Suggest,
        _ => return Err(String::from(USAGE)),
    };

    let mut json_path = None;
    let mut samples_per_move = 128_usize;
    let mut seed = 42_u64;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --json"));
                };
                json_path = Some(PathBuf::from(value));
            }
            "--samples" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --samples"));
                };
                samples_per_move = value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --samples value '{value}': {error}"))?;
            }
            "--seed" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --seed"));
                };
                seed = value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid --seed value '{value}': {error}"))?;
            }
            "-h" | "--help" => {
                return Err(String::from(USAGE));
            }
            _ => {
                return Err(format!("unexpected argument '{arg}'"));
            }
        }
    }

    if mode == Mode::Suggest && json_path.is_none() {
        return Err(String::from("suggest mode requires --json PATH"));
    }

    Ok(CliConfig { mode, json_path, samples_per_move, seed })
}

fn prompt_line(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout().flush().map_err(|error| format!("stdout flush failed: {error}"))?;

    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(|error| format!("stdin read failed: {error}"))?;
    Ok(line.trim().to_string())
}

fn prompt_card(prompt: &str) -> Result<Card, String> {
    loop {
        let line = prompt_line(prompt)?;
        match parse_card(&line) {
            Ok(card) => return Ok(card),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_hand(prompt: &str) -> Result<Vec<Card>, String> {
    loop {
        let line = prompt_line(prompt)?;
        let pieces: Vec<String> = line
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        match parse_card_list(&pieces) {
            Ok(cards) if !cards.is_empty() && cards.len() <= HAND_SIZE => return Ok(cards),
            Ok(_) => println!("hand must have 1 to {HAND_SIZE} cards"),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_player(prompt: &str) -> Result<Player, String> {
    loop {
        let line = prompt_line(prompt)?;
        match parse_player(&line) {
            Ok(player) => return Ok(player),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_u8(prompt: &str) -> Result<u8, String> {
    loop {
        let line = prompt_line(prompt)?;
        match line.parse::<u8>() {
            Ok(value) => return Ok(value),
            Err(error) => println!("invalid number: {error}"),
        }
    }
}

fn prompt_usize(prompt: &str) -> Result<usize, String> {
    loop {
        let line = prompt_line(prompt)?;
        match line.parse::<usize>() {
            Ok(value) => return Ok(value),
            Err(error) => println!("invalid number: {error}"),
        }
    }
}

fn print_card_syntax_legend() {
    println!("Card syntax:");
    println!("  compact: <suit><rank>  (example: 🪙A, ⚔️3, 🪄K)");
    println!("  explicit: <suit>:<rank> (example: clubs:K, coins:A)");
    println!("Suits:");
    println!("  🪙 = denari, 🏆 = coppe, ⚔️ = spade, 🪄 = bastoni");
    println!("  aliases accepted: o/u/s/c, d/b, english and italian names");
    println!("Ranks:");
    println!("  A = Ace, 2..7 = numeric cards, J = Jack/Fante, Q = Queen/Cavallo, K = King/Re");
}

fn print_cards_ascii(
    title: &str,
    cards: &[Card],
    renderer: &mut AsciiCardRenderer,
) -> Result<(), String> {
    if cards.is_empty() {
        return Ok(());
    }

    println!("{title}");
    let mut rendered_cards = Vec::with_capacity(cards.len());
    for card in cards {
        rendered_cards.push(renderer.render_card(*card)?);
    }

    let Some(line_count) = rendered_cards.first().map(Vec::len) else {
        return Ok(());
    };

    for line_index in 0..line_count {
        for (card_index, rendered) in rendered_cards.iter().enumerate() {
            if card_index > 0 {
                print!("  ");
            }
            print!("{}", rendered[line_index]);
        }
        println!();
    }

    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            print!("  ");
        }
        print!("{:width$}", format_card(*card), width = renderer.max_width());
    }
    println!();
    Ok(())
}

fn prompt_briscola_suit() -> Result<Suit, String> {
    loop {
        let suit_raw = prompt_line("briscola suit (coins/cups/swords/clubs or 🪙/🏆/⚔️/🪄): ")?;
        match parse_suit(&suit_raw) {
            Ok(suit) => return Ok(suit),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_initial_state() -> Result<(InteractiveInit, Vec<Card>, Suit), String> {
    let briscola_suit = prompt_briscola_suit()?;
    let face_up_trump = prompt_card("face-up trump card: ")?;
    let my_initial_hand = prompt_hand("your hand (comma-separated, e.g. 🪙A,⚔️3,🪄K): ")?;
    let leader = prompt_player("first leader (me/opponent): ")?;
    let talon_len = prompt_usize(&format!("talon length ({INITIAL_TALON_SIZE} at start): "))?;
    let score_me = prompt_u8("your score: ")?;
    let score_opp = prompt_u8("opponent score: ")?;

    let init =
        InteractiveInit { briscola_suit, face_up_trump, leader, talon_len, score_me, score_opp };
    Ok((init, my_initial_hand, briscola_suit))
}

fn print_turn_state(
    session: &InteractiveSession,
    renderer: &mut AsciiCardRenderer,
) -> Result<(), String> {
    println!();
    println!(
        "turn {} | score {}-{} | talon {} | leader {:?}",
        session.trick_number(),
        session.score_me(),
        session.score_opp(),
        session.talon_len(),
        session.leader()
    );
    print_cards_ascii("your hand:", session.my_hand(), renderer)
}

fn prompt_my_played_card(session: &InteractiveSession, default_move: Card) -> Result<Card, String> {
    let default_card = format_card(default_move);
    loop {
        let line = prompt_line(&format!("your played card [{default_card}]: "))?;
        let chosen = if line.is_empty() {
            default_move
        } else {
            match parse_card(&line) {
                Ok(card) => card,
                Err(error) => {
                    println!("{error}");
                    continue;
                }
            }
        };

        if session.my_hand().contains(&chosen) {
            return Ok(chosen);
        }
        println!("card {} is not in your hand", format_card(chosen));
    }
}

fn resolve_trick_cards(
    leader: Player,
    my_played: Card,
    opp_played: Option<Card>,
    opp_reply_if_me_lead: Option<Card>,
) -> Result<(Card, Card), String> {
    if leader == Player::Me {
        let Some(opp_reply) = opp_reply_if_me_lead else {
            return Err(String::from("missing opponent reply"));
        };
        return Ok((my_played, opp_reply));
    }

    let Some(opp_lead) = opp_played else {
        return Err(String::from("missing opponent lead"));
    };
    Ok((opp_lead, my_played))
}

fn prompt_my_draw_card(
    session: &InteractiveSession,
    winner: Player,
) -> Result<Option<Card>, String> {
    let needs_draw = session.talon_len() > 1 || (session.talon_len() == 1 && winner == Player::Me);
    if needs_draw {
        return prompt_card("your drawn card from talon: ").map(Some);
    }
    Ok(None)
}

fn print_suggestions(stats: &[MoveStats]) {
    println!("suggested moves:");
    for stats in stats {
        println!(
            "  {} -> p_win={:.3}, ev_delta={:.2}, samples={}",
            format_card(stats.card),
            stats.p_win,
            stats.expected_score_delta,
            stats.n_samples
        );
    }
}

fn run_interactive(samples_per_move: usize, seed: u64) -> Result<(), String> {
    println!("Briscola move advisor (interactive)");
    print_card_syntax_legend();
    let mut renderer = AsciiCardRenderer::new(20);

    let (init, my_initial_hand, briscola_suit) = prompt_initial_state()?;

    let mut session = InteractiveSession::new(init, my_initial_hand)
        .map_err(|error| format!("cannot create session: {error:?}"))?;
    let mut rng = FastRng::new(seed);

    while !session.game_over() {
        print_turn_state(&session, &mut renderer)?;

        let opp_played = if session.leader() == Player::Opponent {
            Some(prompt_card("opponent played: ")?)
        } else {
            None
        };
        if let Some(card) = opp_played {
            print_cards_ascii("opponent played:", &[card], &mut renderer)?;
        }

        let suggestion = session
            .suggest_move(opp_played, &mut rng, samples_per_move)
            .map_err(|error| format!("cannot compute suggestion: {error:?}"))?;

        print_suggestions(&suggestion.moves);
        let suggested_cards: Vec<Card> = suggestion.moves.iter().map(|stats| stats.card).collect();
        print_cards_ascii("suggested card art:", &suggested_cards, &mut renderer)?;

        let my_played = prompt_my_played_card(&session, suggestion.best_move)?;

        let opp_reply_if_me_lead = if session.leader() == Player::Me {
            Some(prompt_card("opponent reply card: ")?)
        } else {
            None
        };

        let (lead, reply) =
            resolve_trick_cards(session.leader(), my_played, opp_played, opp_reply_if_me_lead)?;

        let winner = if trick_winner(lead, reply, briscola_suit) == TrickWinner::Leader {
            session.leader()
        } else {
            session.leader().other()
        };

        let my_draw_card = prompt_my_draw_card(&session, winner)?;

        let result = session
            .apply_turn(opp_played, my_played, opp_reply_if_me_lead, my_draw_card)
            .map_err(|error| format!("cannot apply turn: {error:?}"))?;

        println!(
            "winner {:?}, trick points {}, score {}-{}",
            result.winner, result.points, result.score_me, result.score_opp
        );
    }

    println!();
    println!("game over | final score {}-{}", session.score_me(), session.score_opp());
    Ok(())
}

fn run_suggest(config: &CliConfig) -> Result<(), String> {
    let Some(path) = &config.json_path else {
        return Err(String::from("missing json path"));
    };

    print_card_syntax_legend();
    let mut renderer = AsciiCardRenderer::new(20);
    let result = suggest_from_json_path(path, Some(config.samples_per_move), Some(config.seed))?;
    println!("best move: {}", format_card(result.best_move));
    print_cards_ascii("best move card art:", &[result.best_move], &mut renderer)?;
    for stats in result.moves {
        println!(
            "  {} -> p_win={:.3}, ev_delta={:.2}, samples={}",
            format_card(stats.card),
            stats.p_win,
            stats.expected_score_delta,
            stats.n_samples
        );
    }
    Ok(())
}

fn main() {
    let config = match parse_cli() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let outcome = match config.mode {
        Mode::Interactive => run_interactive(config.samples_per_move, config.seed),
        Mode::Suggest => run_suggest(&config),
    };

    if let Err(error) = outcome {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
