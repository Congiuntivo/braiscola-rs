//! Greedy rollout policy for simulated playouts.
//!
//! The policy is deterministic and intentionally cheap: it attempts to preserve
//! high-value cards and avoid spending trump unless useful in context.

use briscola_core::card::Card;
use briscola_core::rules::{TrickWinner, trick_points, trick_winner};
use briscola_core::state::{DeterminizedState, Player};

/// Returns true when the reply card wins the trick against the lead card.
///
/// # Parameters
///
/// - `lead_card`: Card already led.
/// - `reply_card`: Candidate response card.
/// - `briscola`: Trump suit for the current match.
fn reply_wins(lead_card: Card, reply_card: Card, briscola: briscola_core::card::Suit) -> bool {
    trick_winner(lead_card, reply_card, briscola) == TrickWinner::Follower
}

/// Chooses a lead card with a conservative greedy heuristic.
///
/// # Parameters
///
/// - `state`: Determinized game state used during simulation.
/// - `player`: Player who is currently leading.
///
/// # Strategy
///
/// Priority order:
/// 1. Lowest-power non-trump card worth 0 points.
/// 2. Otherwise, lowest (points, power) non-trump card.
/// 3. Otherwise, lowest (points, power) card in hand.
pub fn choose_lead_card(state: &DeterminizedState, player: Player) -> Card {
    let hand = state.hand(player);

    if let Some(card) = hand
        .iter()
        .copied()
        .filter(|card| card.suit != state.briscola_suit && card.rank.points() == 0)
        .min_by_key(|card| card.rank.power())
    {
        return card;
    }

    if let Some(card) = hand
        .iter()
        .copied()
        .filter(|card| card.suit != state.briscola_suit)
        .min_by_key(|card| (card.rank.points(), card.rank.power()))
    {
        return card;
    }

    hand.iter()
        .copied()
        .min_by_key(|card| (card.rank.points(), card.rank.power()))
        .expect("leader has at least one card")
}

/// Chooses a reply card with a greedy win-when-cheap policy.
///
/// # Parameters
///
/// - `state`: Determinized game state used during simulation.
/// - `player`: Player replying to the current lead.
/// - `lead_card`: Card already led in this trick.
///
/// # Strategy
///
/// If a winning reply exists, play the weakest winning card, preferring
/// non-trump winners in low-value tricks while the talon is still deep.
/// Otherwise, discard the weakest card by (points, is_trump, power).
pub fn choose_reply_card(state: &DeterminizedState, player: Player, lead_card: Card) -> Card {
    let hand = state.hand(player);
    let points_in_trick = trick_points(lead_card, lead_card);

    let mut winning_cards: Vec<Card> = hand
        .iter()
        .copied()
        .filter(|card| reply_wins(lead_card, *card, state.briscola_suit))
        .collect();

    if !winning_cards.is_empty() {
        if points_in_trick <= 2 && state.talon.len() > 4 {
            let non_trump_winning: Vec<Card> = winning_cards
                .iter()
                .copied()
                .filter(|card| card.suit != state.briscola_suit)
                .collect();
            if !non_trump_winning.is_empty() {
                winning_cards = non_trump_winning;
            }
        }

        return winning_cards
            .iter()
            .copied()
            .min_by_key(|card| {
                (u8::from(card.suit == state.briscola_suit), card.rank.power(), card.rank.points())
            })
            .expect("winning card exists");
    }

    hand.iter()
        .copied()
        .min_by_key(|card| {
            (card.rank.points(), u8::from(card.suit == state.briscola_suit), card.rank.power())
        })
        .expect("follower has at least one card")
}
