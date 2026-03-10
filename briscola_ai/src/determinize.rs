use briscola_core::bitset::contains;
use briscola_core::card::full_deck;
use briscola_core::state::{DeterminizedState, Player, PublicGameState};

use crate::rng::FastRng;

/// Errors while building a determinized world from public information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeterminizeError {
    InvalidPublicState,
}

/// Samples a full hidden game state that is consistent with the public view.
pub fn sample_world(
    public: &PublicGameState,
    rng: &mut FastRng,
) -> Result<DeterminizedState, DeterminizeError> {
    let mut unknown = Vec::new();
    for card in full_deck() {
        if !contains(public.seen_cards, card) {
            unknown.push(card);
        }
    }

    let expected_opponent_hand_len = expected_opponent_hand_len(public);
    let required_unknown = expected_opponent_hand_len + public.talon_len;
    if unknown.len() < required_unknown {
        return Err(DeterminizeError::InvalidPublicState);
    }

    rng.shuffle(&mut unknown);
    let sampled = &unknown[..required_unknown];
    let opp_hand = sampled[..expected_opponent_hand_len].to_vec();
    let talon = sampled[expected_opponent_hand_len..].to_vec();

    Ok(DeterminizedState {
        my_hand: public.my_hand.clone(),
        opp_hand,
        talon,
        briscola_suit: public.briscola_suit,
        face_up_trump: public.last_face_up_trump,
        score_me: public.score_me,
        score_opp: public.score_opp,
        leader: public.leader,
        pending_lead: public.opp_played,
        pending_lead_by: public.opp_played.map(|_| Player::Opponent),
    })
}

fn expected_opponent_hand_len(public: &PublicGameState) -> usize {
    if public.opp_played.is_some() {
        public.my_hand.len().saturating_sub(1)
    } else {
        public.my_hand.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use briscola_core::bitset::{CardMask, FULL_MASK, add, contains};
    use briscola_core::card::{Card, Rank, Suit, full_deck};
    use briscola_core::state::{Player, PublicGameState};

    fn subtract(full: CardMask, cards: &[Card]) -> CardMask {
        let mut mask = full;
        for card in cards {
            mask &= !(1u64 << card.index());
        }
        mask
    }

    #[test]
    fn sampled_world_uses_only_unknown_cards() {
        let my_cards = vec![
            Card::new(Suit::Coins, Rank::Ace),
            Card::new(Suit::Cups, Rank::Three),
            Card::new(Suit::Swords, Rank::King),
        ];
        let opp_played = Card::new(Suit::Clubs, Rank::Two);
        let face_up = Card::new(Suit::Clubs, Rank::Ace);

        let mut fixed_seen = my_cards.clone();
        fixed_seen.push(opp_played);
        fixed_seen.push(face_up);

        let mut unknown_target = Vec::new();
        for card in full_deck() {
            if !fixed_seen.contains(&card) && unknown_target.len() < 12 {
                unknown_target.push(card);
            }
        }

        let seen = subtract(FULL_MASK, &unknown_target);

        let public = PublicGameState {
            my_hand: my_cards,
            opp_played: Some(opp_played),
            briscola_suit: Suit::Clubs,
            talon_len: 10,
            last_face_up_trump: face_up,
            seen_cards: seen,
            score_me: 20,
            score_opp: 11,
            leader: Player::Opponent,
        };

        let mut rng = FastRng::new(7);
        let sampled = sample_world(&public, &mut rng).expect("sample world");

        assert_eq!(sampled.opp_hand.len(), 2);
        assert_eq!(sampled.talon.len(), 10);

        for card in sampled.opp_hand.iter().chain(sampled.talon.iter()) {
            assert!(!contains(public.seen_cards, *card));
        }

        let mut union = 0u64;
        for card in sampled.opp_hand.iter().chain(sampled.talon.iter()) {
            union = add(union, *card);
        }

        for card in unknown_target {
            assert!(contains(union, card));
        }
    }
}
