//! Monte Carlo move evaluator.
//!
//! For each legal move, this module samples hidden worlds consistent with public
//! information, forces that move, rolls out to terminal state, and aggregates:
//! - win score in [0.0, 1.0]
//! - expected final score delta (me - opponent)

use std::cmp::Ordering;

use briscola_core::card::Card;
use briscola_core::state::{Player, PublicGameState, StateError};

use crate::determinize::{DeterminizeError, sample_world};
use crate::rng::FastRng;
use crate::rollout::{choose_lead_card, choose_reply_card};

/// Monte Carlo search configuration.
#[derive(Debug, Clone, Copy)]
pub struct MonteCarloConfig {
    /// Number of sampled worlds evaluated for each legal move.
    ///
    /// Values below 1 are clamped to 1 at runtime.
    pub samples_per_move: usize,
}

/// Aggregated statistics for a legal move.
#[derive(Debug, Clone, Copy)]
pub struct MoveStats {
    /// Candidate card this row refers to.
    pub card: Card,
    /// Estimated win probability for this move.
    ///
    /// 1.0 = always win, 0.5 = always draw, 0.0 = always lose.
    pub p_win: f64,
    /// Mean final score delta (me - opponent) across successful samples.
    pub expected_score_delta: f64,
    /// Number of successful simulations used in this estimate.
    pub n_samples: usize,
}

/// Ranked result for all legal moves.
#[derive(Debug, Clone)]
pub struct BestMoveResult {
    /// Top-ranked move according to sort policy.
    pub best_move: Card,
    /// All evaluated moves sorted by descending win probability,
    /// then descending expected score delta.
    pub moves: Vec<MoveStats>,
}

/// Failures that can occur while selecting a move by simulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonteCarloError {
    /// No legal move can be played from this state.
    NoLegalMoves,
    /// The caller asked for a move when it is not me to act.
    NotMyTurn,
    /// Hidden-world sampling failed due to inconsistent public information.
    DeterminizeFailed,
    /// State transitions failed while simulating a playout.
    SimulationFailed,
    /// Every attempted sample failed, so no estimate can be produced.
    NoSuccessfulSamples,
}

/// Chooses the best move among legal options using sampled hidden information.
///
/// # Parameters
///
/// - `public`: Public game view used as rollout root.
/// - `rng`: Random generator used for hidden-world sampling.
/// - `config`: Monte Carlo sampling configuration.
///
/// # Returns
///
/// A ranked list of per-move estimates and the top move.
/// Ranking order is descending `p_win`, then descending
/// `expected_score_delta`.
///
/// # Errors
///
/// Returns [MonteCarloError] when no legal move exists, the caller is not to
/// act, determinization fails, simulation fails, or no sample succeeds.
pub fn choose_best_move(
    public: &PublicGameState,
    rng: &mut FastRng,
    config: MonteCarloConfig,
) -> Result<BestMoveResult, MonteCarloError> {
    let legal_moves = public.legal_moves();
    if legal_moves.is_empty() {
        return Err(MonteCarloError::NoLegalMoves);
    }

    if public.opp_played.is_none() && public.leader != Player::Me {
        return Err(MonteCarloError::NotMyTurn);
    }

    let samples = config.samples_per_move.max(1);
    let mut moves = Vec::with_capacity(legal_moves.len());

    for card in legal_moves {
        let mut wins = 0.0_f64;
        let mut deltas = 0.0_f64;
        let mut successful_samples = 0_usize;

        for _ in 0..samples {
            let mut state = sample_world(public, rng)
                .map_err(|_: DeterminizeError| MonteCarloError::DeterminizeFailed)?;
            let Ok((win_score, delta)) = simulate_with_forced_move(public, &mut state, card) else {
                continue;
            };
            wins += win_score;
            deltas += f64::from(delta);
            successful_samples += 1;
        }

        if successful_samples == 0 {
            return Err(MonteCarloError::NoSuccessfulSamples);
        }

        let successful_u32 =
            u32::try_from(successful_samples).map_err(|_| MonteCarloError::SimulationFailed)?;
        let n = f64::from(successful_u32);

        moves.push(MoveStats {
            card,
            p_win: wins / n,
            expected_score_delta: deltas / n,
            n_samples: successful_samples,
        });
    }

    moves.sort_by(|a, b| {
        b.p_win.partial_cmp(&a.p_win).unwrap_or(std::cmp::Ordering::Equal).then_with(|| {
            b.expected_score_delta
                .partial_cmp(&a.expected_score_delta)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });

    Ok(BestMoveResult { best_move: moves[0].card, moves })
}

/// Simulates one playout after forcing my root move.
///
/// # Parameters
///
/// - `public`: Root public state, used only to detect whether I am replying or leading.
/// - `state`: Determinized mutable state that will be advanced to terminal.
/// - `forced_move`: Move to force for me at the root.
///
/// # Returns
///
/// Tuple `(win_score, delta)` where:
/// - `win_score` is 1.0 for win, 0.5 for draw, 0.0 for loss.
/// - `delta` is final score difference `(me - opponent)`.
///
/// # Errors
///
/// Returns [MonteCarloError::SimulationFailed] if any transition is invalid.
fn simulate_with_forced_move(
    public: &PublicGameState,
    state: &mut briscola_core::state::DeterminizedState,
    forced_move: Card,
) -> Result<(f64, i16), MonteCarloError> {
    if public.opp_played.is_some() {
        state
            .play_reply_card(Player::Me, forced_move)
            .map_err(|_: StateError| MonteCarloError::SimulationFailed)?;
    } else {
        state
            .play_lead_card(Player::Me, forced_move)
            .map_err(|_: StateError| MonteCarloError::SimulationFailed)?;
        let opp_reply = choose_reply_card(state, Player::Opponent, forced_move);
        state
            .play_reply_card(Player::Opponent, opp_reply)
            .map_err(|_: StateError| MonteCarloError::SimulationFailed)?;
    }

    while !state.is_terminal() {
        if let Some(lead_card) = state.pending_lead {
            let lead_by = state.pending_lead_by.ok_or(MonteCarloError::SimulationFailed)?;
            let follower = lead_by.other();
            let reply = choose_reply_card(state, follower, lead_card);
            state
                .play_reply_card(follower, reply)
                .map_err(|_: StateError| MonteCarloError::SimulationFailed)?;
        } else {
            let leader = state.leader;
            let lead = choose_lead_card(state, leader);
            state
                .play_lead_card(leader, lead)
                .map_err(|_: StateError| MonteCarloError::SimulationFailed)?;
            let follower = leader.other();
            let reply = choose_reply_card(state, follower, lead);
            state
                .play_reply_card(follower, reply)
                .map_err(|_: StateError| MonteCarloError::SimulationFailed)?;
        }
    }

    let delta = i16::from(state.score_me) - i16::from(state.score_opp);
    let win_score = match delta.cmp(&0) {
        Ordering::Greater => 1.0,
        Ordering::Equal => 0.5,
        Ordering::Less => 0.0,
    };

    Ok((win_score, delta))
}

#[cfg(test)]
mod tests {
    use briscola_core::bitset::{FULL_MASK, card_mask};
    use briscola_core::card::{Card, Rank, Suit};
    use briscola_core::state::{Player, PublicGameState};

    use super::*;

    #[test]
    fn follower_prefers_winning_card_in_deterministic_end_state() {
        let winning = Card::new(Suit::Coins, Rank::Ace);
        let losing = Card::new(Suit::Swords, Rank::Two);
        let hidden = Card::new(Suit::Clubs, Rank::King);
        let opp_played = Card::new(Suit::Coins, Rank::King);
        let trump = Card::new(Suit::Clubs, Rank::Four);

        let public = PublicGameState {
            my_hand: vec![winning, losing],
            opp_played: Some(opp_played),
            briscola_suit: Suit::Clubs,
            talon_len: 0,
            last_face_up_trump: trump,
            seen_cards: FULL_MASK & !card_mask(hidden),
            score_me: 50,
            score_opp: 48,
            leader: Player::Opponent,
        };

        let mut rng = FastRng::new(13);
        let result = choose_best_move(&public, &mut rng, MonteCarloConfig { samples_per_move: 8 })
            .expect("choose move");

        assert_eq!(result.best_move, winning);
    }
}
