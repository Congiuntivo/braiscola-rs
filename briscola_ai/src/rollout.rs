use briscola_core::card::Card;
use briscola_core::rules::{TrickWinner, trick_points, trick_winner};
use briscola_core::state::{DeterminizedState, Player};

fn reply_wins(lead_card: Card, reply_card: Card, briscola: briscola_core::card::Suit) -> bool {
    trick_winner(lead_card, reply_card, briscola) == TrickWinner::Follower
}

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
