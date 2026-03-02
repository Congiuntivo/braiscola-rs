use crate::card::{Card, Suit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrickWinner {
    Leader,
    Follower,
}

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
