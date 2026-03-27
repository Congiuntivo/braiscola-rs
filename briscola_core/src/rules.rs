//! Trick-resolution and scoring rules for Briscola.

use crate::card::{Card, Suit};

/// Winner role in a resolved two-card trick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrickWinner {
    /// Player who led the trick.
    Leader,
    /// Player who replied to the lead.
    Follower,
}

/// Resolves the winner of a two-card trick.
///
/// # Parameters
///
/// - `lead_card`: Card played by the leader.
/// - `reply_card`: Card played by the follower.
/// - `briscola_suit`: Trump suit.
///
/// # Returns
///
/// - [TrickWinner::Follower] if follower beats leader by same-suit higher power,
///   or by playing trump against non-trump.
/// - [TrickWinner::Leader] otherwise.
pub fn trick_winner(lead_card: Card, reply_card: Card, briscola_suit: Suit) -> TrickWinner {
    if lead_card.suit == reply_card.suit {
        if reply_card.rank.power() > lead_card.rank.power() {
            TrickWinner::Follower
        } else {
            TrickWinner::Leader
        }
    } else if reply_card.suit == briscola_suit && lead_card.suit != briscola_suit {
        TrickWinner::Follower
    } else {
        TrickWinner::Leader
    }
}

/// Returns total points contained in a completed trick.
///
/// # Parameters
///
/// - `lead_card`: Card played by the leader.
/// - `reply_card`: Card played by the follower.
pub fn trick_points(lead_card: Card, reply_card: Card) -> u8 {
    lead_card.rank.points() + reply_card.rank.points()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Rank, Suit};

    #[test]
    fn trick_winner_follows_briscola_rules() {
        let briscola = Suit::Clubs;
        let lead = Card::new(Suit::Coins, Rank::King);
        let reply = Card::new(Suit::Coins, Rank::Ace);
        assert_eq!(trick_winner(lead, reply, briscola), TrickWinner::Follower);

        let lead = Card::new(Suit::Swords, Rank::Ace);
        let reply = Card::new(Suit::Clubs, Rank::Two);
        assert_eq!(trick_winner(lead, reply, briscola), TrickWinner::Follower);

        let lead = Card::new(Suit::Clubs, Rank::Four);
        let reply = Card::new(Suit::Coins, Rank::Ace);
        assert_eq!(trick_winner(lead, reply, briscola), TrickWinner::Leader);

        let lead = Card::new(Suit::Cups, Rank::Seven);
        let reply = Card::new(Suit::Coins, Rank::Ace);
        assert_eq!(trick_winner(lead, reply, briscola), TrickWinner::Leader);
    }
}
