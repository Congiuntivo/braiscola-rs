//! CLI-facing simulation APIs and shared helpers.
//!
//! This crate exposes reusable building blocks used by command binaries:
//! - full game simulation with configurable strategy for Me,
//! - advisor session and JSON suggestion helpers,
//! - interactive play-state orchestration,
//! - card rendering helpers for text and terminal UI.

pub mod advisor;
pub mod card_art;
pub mod play;

use briscola_ai::mc::{MonteCarloConfig, MonteCarloError, choose_best_move};
use briscola_ai::rng::FastRng;
use briscola_ai::rollout::{choose_lead_card, choose_reply_card};
use briscola_core::bitset::{CardMask, add};
use briscola_core::card::{Card, FULL_DECK_SIZE, HAND_SIZE, INITIAL_TALON_SIZE, Suit, full_deck};
use briscola_core::state::{DeterminizedState, Player, PublicGameState};

/// Policy used for Me when simulating games.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MePolicy {
    /// Fast deterministic heuristic policy.
    Heuristic,
    /// Monte Carlo best-move policy from briscola_ai.
    BestMove,
}

/// Simulation configuration knobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationOptions {
    /// Move-selection policy for Me.
    pub me_policy: MePolicy,
    /// Samples per move when [MePolicy::BestMove] is used.
    pub samples_per_move: usize,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self { me_policy: MePolicy::Heuristic, samples_per_move: 128 }
    }
}

/// Winner of a completed simulated game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameWinner {
    /// Me has strictly higher final score.
    Me,
    /// Opponent has strictly higher final score.
    Opponent,
    /// Final scores are equal.
    Draw,
}

/// Simulation failure causes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationError {
    /// Deck shape or derived card counts were invalid.
    InvalidDeck,
    /// A state transition failed unexpectedly.
    InvalidTransition,
    /// Monte Carlo strategy failed while choosing a move.
    StrategyFailed(MonteCarloError),
}

/// Snapshot log for one resolved trick during simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrickLog {
    /// 1-based trick number.
    pub number: usize,
    /// Player that led this trick.
    pub leader: Player,
    /// Card led by [TrickLog::leader].
    pub lead_card: Card,
    /// Player that replied to the lead.
    pub follower: Player,
    /// Reply card by [TrickLog::follower].
    pub reply_card: Card,
    /// Winner of this trick.
    pub winner: Player,
    /// Points collected in this trick.
    pub trick_points: u8,
    /// Score for Me immediately after trick resolution.
    pub score_me: u8,
    /// Score for Opponent immediately after trick resolution.
    pub score_opp: u8,
    /// My hand size after trick resolution.
    pub my_hand_len: usize,
    /// Opponent hand size after trick resolution.
    pub opp_hand_len: usize,
    /// Talon size after trick resolution.
    pub talon_len: usize,
}

/// Full simulation output, including trick-by-trick history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameLog {
    /// RNG seed used to initialize simulation state.
    pub seed: u64,
    /// Trump suit for the game.
    pub briscola_suit: Suit,
    /// Face-up trump card under the talon.
    pub face_up_trump: Card,
    /// My starting hand at trick 1.
    pub initial_my_hand: Vec<Card>,
    /// Opponent starting hand at trick 1.
    pub initial_opp_hand: Vec<Card>,
    /// Ordered trick history from first to last trick.
    pub tricks: Vec<TrickLog>,
    /// Final score for Me.
    pub final_score_me: u8,
    /// Final score for Opponent.
    pub final_score_opp: u8,
    /// Winner derived from final scores.
    pub winner: GameWinner,
}

/// Simulates a full game with default [SimulationOptions].
///
/// # Parameters
///
/// - `seed`: Seed controlling deck shuffle and deterministic behavior.
pub fn simulate_game(seed: u64) -> Result<GameLog, SimulationError> {
    simulate_game_with_options(seed, SimulationOptions::default())
}

/// Simulates a full game from seed and strategy options.
///
/// # Parameters
///
/// - `seed`: Seed controlling deck shuffle and RNG streams.
/// - `options`: Strategy and Monte Carlo sampling options.
///
/// # Returns
///
/// A [GameLog] containing complete game trace and final result.
///
/// # Errors
///
/// Returns [SimulationError] if initial state is inconsistent, a transition
/// fails, or strategy selection fails.
pub fn simulate_game_with_options(
    seed: u64,
    options: SimulationOptions,
) -> Result<GameLog, SimulationError> {
    let mut state = initial_state(seed)?;
    let mut best_move_rng = FastRng::new(seed ^ 0xA5A5_A5A5_5A5A_5A5A);
    let initial_my_hand = state.my_hand.clone();
    let initial_opp_hand = state.opp_hand.clone();
    let briscola_suit = state.briscola_suit;
    let face_up_trump = state.face_up_trump;
    let mut opp_revealed_cards = 0_u64;

    let mut tricks = Vec::with_capacity(20);

    while !state.is_terminal() {
        let number = tricks.len() + 1;
        let leader = state.leader;
        let lead_card = if leader == Player::Me {
            let seen_cards = visible_seen_mask(&state, opp_revealed_cards, None);
            select_me_card(&state, seen_cards, None, options, &mut best_move_rng)?
        } else {
            choose_lead_card(&state, leader)
        };

        if leader == Player::Opponent {
            opp_revealed_cards = add(opp_revealed_cards, lead_card);
        }

        if state.play_lead_card(leader, lead_card).is_err() {
            return Err(SimulationError::InvalidTransition);
        }

        let follower = leader.other();
        let reply_card = if follower == Player::Me {
            let seen_cards = visible_seen_mask(&state, opp_revealed_cards, Some(lead_card));
            select_me_card(&state, seen_cards, Some(lead_card), options, &mut best_move_rng)?
        } else {
            choose_reply_card(&state, follower, lead_card)
        };

        if follower == Player::Opponent {
            opp_revealed_cards = add(opp_revealed_cards, reply_card);
        }

        let Ok(outcome) = state.play_reply_card(follower, reply_card) else {
            return Err(SimulationError::InvalidTransition);
        };

        tricks.push(TrickLog {
            number,
            leader,
            lead_card,
            follower,
            reply_card,
            winner: outcome.winner,
            trick_points: outcome.trick_points,
            score_me: state.score_me,
            score_opp: state.score_opp,
            my_hand_len: state.my_hand.len(),
            opp_hand_len: state.opp_hand.len(),
            talon_len: state.talon.len(),
        });
    }

    let winner = match state.score_me.cmp(&state.score_opp) {
        std::cmp::Ordering::Greater => GameWinner::Me,
        std::cmp::Ordering::Less => GameWinner::Opponent,
        std::cmp::Ordering::Equal => GameWinner::Draw,
    };

    Ok(GameLog {
        seed,
        briscola_suit,
        face_up_trump,
        initial_my_hand,
        initial_opp_hand,
        tricks,
        final_score_me: state.score_me,
        final_score_opp: state.score_opp,
        winner,
    })
}

fn visible_seen_mask(
    state: &DeterminizedState,
    opp_revealed_cards: CardMask,
    opp_played: Option<Card>,
) -> CardMask {
    let mut seen_cards = add(opp_revealed_cards, state.face_up_trump);
    for card in &state.my_hand {
        seen_cards = add(seen_cards, *card);
    }
    if let Some(card) = opp_played {
        seen_cards = add(seen_cards, card);
    }
    seen_cards
}

fn select_me_card(
    state: &DeterminizedState,
    seen_cards: CardMask,
    opp_played: Option<Card>,
    options: SimulationOptions,
    rng: &mut FastRng,
) -> Result<Card, SimulationError> {
    match options.me_policy {
        MePolicy::Heuristic => Ok(match opp_played {
            Some(opp_card) => choose_reply_card(state, Player::Me, opp_card),
            None => choose_lead_card(state, Player::Me),
        }),
        MePolicy::BestMove => {
            let public = PublicGameState {
                my_hand: state.my_hand.clone(),
                opp_played,
                briscola_suit: state.briscola_suit,
                talon_len: state.talon.len(),
                last_face_up_trump: state.face_up_trump,
                seen_cards,
                score_me: state.score_me,
                score_opp: state.score_opp,
                leader: state.leader,
            };

            let result = choose_best_move(
                &public,
                rng,
                MonteCarloConfig { samples_per_move: options.samples_per_move },
            )
            .map_err(SimulationError::StrategyFailed)?;
            Ok(result.best_move)
        }
    }
}

fn initial_state(seed: u64) -> Result<DeterminizedState, SimulationError> {
    let mut deck = full_deck();
    if deck.len() != FULL_DECK_SIZE {
        return Err(SimulationError::InvalidDeck);
    }

    let mut rng = FastRng::new(seed);
    rng.shuffle(&mut deck);

    let mut cursor = 0;
    let mut my_hand = Vec::with_capacity(HAND_SIZE);
    let mut opp_hand = Vec::with_capacity(HAND_SIZE);

    for _ in 0..HAND_SIZE {
        my_hand.push(deck[cursor]);
        cursor += 1;
        opp_hand.push(deck[cursor]);
        cursor += 1;
    }

    let face_up_trump = deck[cursor];
    cursor += 1;
    let talon = deck[cursor..].to_vec();

    if talon.len() != INITIAL_TALON_SIZE {
        return Err(SimulationError::InvalidDeck);
    }

    Ok(DeterminizedState {
        my_hand,
        opp_hand,
        talon,
        briscola_suit: face_up_trump.suit,
        face_up_trump,
        score_me: 0,
        score_opp: 0,
        leader: Player::Me,
        pending_lead: None,
        pending_lead_by: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GameWinner, MePolicy, SimulationOptions, simulate_game, simulate_game_with_options,
    };

    #[test]
    fn simulation_completes_with_20_tricks_and_120_points() {
        let result = simulate_game(42);
        assert!(result.is_ok());

        let Ok(log) = result else {
            return;
        };

        assert_eq!(log.tricks.len(), 20);
        assert_eq!(u16::from(log.final_score_me) + u16::from(log.final_score_opp), 120);
    }

    #[test]
    fn simulation_is_deterministic_for_same_seed() {
        let first = simulate_game(7);
        let second = simulate_game(7);
        assert!(first.is_ok());
        assert!(second.is_ok());

        let Ok(first_log) = first else {
            return;
        };
        let Ok(second_log) = second else {
            return;
        };

        assert_eq!(first_log, second_log);
    }

    #[test]
    fn winner_matches_final_score_relation() {
        let result = simulate_game(99);
        assert!(result.is_ok());

        let Ok(log) = result else {
            return;
        };

        match log.final_score_me.cmp(&log.final_score_opp) {
            std::cmp::Ordering::Greater => assert_eq!(log.winner, GameWinner::Me),
            std::cmp::Ordering::Less => assert_eq!(log.winner, GameWinner::Opponent),
            std::cmp::Ordering::Equal => assert_eq!(log.winner, GameWinner::Draw),
        }
    }

    #[test]
    fn best_move_policy_simulation_completes() {
        let result = simulate_game_with_options(
            42,
            SimulationOptions { me_policy: MePolicy::BestMove, samples_per_move: 128 },
        );
        assert!(result.is_ok());

        let Ok(log) = result else {
            return;
        };

        assert_eq!(log.tricks.len(), 20);
        assert_eq!(u16::from(log.final_score_me) + u16::from(log.final_score_opp), 120);
    }
}
