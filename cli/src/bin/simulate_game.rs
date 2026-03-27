//! Headless simulation CLI entrypoint.
//!
//! Runs one full game simulation and prints trick-by-trick logs plus final score.

use cli::{MePolicy, SimulationOptions, simulate_game_with_options};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CliArgs {
    seed: u64,
    best_me: bool,
    samples_per_move: usize,
}

fn parse_args() -> Result<CliArgs, String> {
    let mut seed = 42_u64;
    let mut seed_set = false;
    let mut best_me = false;
    let mut samples_per_move = 128_usize;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--best-me" => {
                best_me = true;
            }
            "--samples" => {
                let Some(raw) = args.next() else {
                    return Err(String::from("missing value after --samples"));
                };
                samples_per_move = raw
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --samples value '{raw}': {error}"))?;
            }
            "-h" | "--help" => {
                return Err(String::from(
                    "usage: cargo run -p cli --bin simulate_game -- [SEED] [--best-me] [--samples N]",
                ));
            }
            _ => {
                if seed_set {
                    return Err(format!("unexpected argument '{arg}'"));
                }
                seed =
                    arg.parse::<u64>().map_err(|error| format!("invalid seed '{arg}': {error}"))?;
                seed_set = true;
            }
        }
    }

    Ok(CliArgs { seed, best_me, samples_per_move })
}

fn main() {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let options = SimulationOptions {
        me_policy: if args.best_me { MePolicy::BestMove } else { MePolicy::Heuristic },
        samples_per_move: args.samples_per_move,
    };

    let simulation = match simulate_game_with_options(args.seed, options) {
        Ok(log) => log,
        Err(error) => {
            eprintln!("simulation failed: {error:?}");
            std::process::exit(1);
        }
    };

    println!("seed: {}", simulation.seed);
    println!("me policy: {:?}", options.me_policy);
    println!("samples per move: {}", options.samples_per_move);
    println!("briscola: {:?}", simulation.briscola_suit);
    println!("face-up trump: {:?}", simulation.face_up_trump);
    println!("my initial hand: {:?}", simulation.initial_my_hand);
    println!("opponent initial hand: {:?}", simulation.initial_opp_hand);
    println!();

    for trick in &simulation.tricks {
        println!(
            "trick {:02}: leader={:?} lead={:?} reply={:?} winner={:?} points={} score=({}-{}) hands=({},{}) talon={}",
            trick.number,
            trick.leader,
            trick.lead_card,
            trick.reply_card,
            trick.winner,
            trick.trick_points,
            trick.score_me,
            trick.score_opp,
            trick.my_hand_len,
            trick.opp_hand_len,
            trick.talon_len
        );
    }

    println!();
    println!(
        "final score: {}-{} winner={:?}",
        simulation.final_score_me, simulation.final_score_opp, simulation.winner
    );
}
