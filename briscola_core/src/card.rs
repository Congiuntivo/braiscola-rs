/// Briscola suit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Suit {
    Coins,
    Cups,
    Swords,
    Clubs,
}

impl Suit {
    /// All suits in canonical deck order.
    pub const ALL: [Suit; 4] = [Suit::Coins, Suit::Cups, Suit::Swords, Suit::Clubs];

    /// Zero-based suit index in canonical deck order.
    pub fn index(self) -> usize {
        match self {
            Suit::Coins => 0,
            Suit::Cups => 1,
            Suit::Swords => 2,
            Suit::Clubs => 3,
        }
    }
}

/// Briscola rank.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Rank {
    Ace,
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Jack,
    Queen,
    King,
}

impl Rank {
    /// All ranks in canonical deck order.
    pub const ALL: [Rank; 10] = [
        Rank::Ace,
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
    ];

    /// Zero-based rank index in canonical deck order.
    pub fn deck_index(self) -> usize {
        match self {
            Rank::Ace => 0,
            Rank::Two => 1,
            Rank::Three => 2,
            Rank::Four => 3,
            Rank::Five => 4,
            Rank::Six => 5,
            Rank::Seven => 6,
            Rank::Jack => 7,
            Rank::Queen => 8,
            Rank::King => 9,
        }
    }

    /// Trick points for this rank in classic Briscola scoring.
    pub fn points(self) -> u8 {
        match self {
            Rank::Ace => 11,
            Rank::Three => 10,
            Rank::King => 4,
            Rank::Queen => 3,
            Rank::Jack => 2,
            Rank::Two | Rank::Four | Rank::Five | Rank::Six | Rank::Seven => 0,
        }
    }

    /// Relative rank strength used to compare cards of the same suit.
    pub fn power(self) -> u8 {
        match self {
            Rank::Two => 0,
            Rank::Four => 1,
            Rank::Five => 2,
            Rank::Six => 3,
            Rank::Seven => 4,
            Rank::Jack => 5,
            Rank::Queen => 6,
            Rank::King => 7,
            Rank::Three => 8,
            Rank::Ace => 9,
        }
    }
}

/// A single card in a Briscola deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Card {
    pub suit: Suit,
    pub rank: Rank,
}

impl Card {
    /// Creates a card with the given suit and rank.
    pub fn new(suit: Suit, rank: Rank) -> Self {
        Self { suit, rank }
    }

    /// Zero-based index in canonical deck order.
    pub fn index(self) -> usize {
        self.suit.index() * 10 + self.rank.deck_index()
    }
}

/// Number of cards in a full Briscola deck.
pub const FULL_DECK_SIZE: usize = 40;
/// Number of cards dealt to each player.
pub const HAND_SIZE: usize = 3;
/// Cards remaining in the talon after dealing and revealing the trump.
pub const INITIAL_TALON_SIZE: usize = FULL_DECK_SIZE - (HAND_SIZE * 2 + 1);

/// Converts a canonical deck index into a card.
pub fn card_from_index(index: usize) -> Option<Card> {
    if index >= FULL_DECK_SIZE {
        return None;
    }
    let suit = Suit::ALL[index / 10];
    let rank = Rank::ALL[index % 10];
    Some(Card::new(suit, rank))
}

/// Returns a full 40-card Briscola deck in canonical order.
pub fn full_deck() -> Vec<Card> {
    let mut deck = Vec::with_capacity(FULL_DECK_SIZE);
    for suit in Suit::ALL {
        for rank in Rank::ALL {
            deck.push(Card::new(suit, rank));
        }
    }
    deck
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_points_and_power_follow_briscola_rules() {
        assert_eq!(Rank::Ace.points(), 11);
        assert_eq!(Rank::Three.points(), 10);
        assert_eq!(Rank::King.points(), 4);
        assert_eq!(Rank::Queen.points(), 3);
        assert_eq!(Rank::Jack.points(), 2);
        assert_eq!(Rank::Seven.points(), 0);

        assert!(Rank::Ace.power() > Rank::Three.power());
        assert!(Rank::Three.power() > Rank::King.power());
        assert!(Rank::King.power() > Rank::Queen.power());
        assert!(Rank::Queen.power() > Rank::Jack.power());
        assert!(Rank::Jack.power() > Rank::Seven.power());
        assert!(Rank::Seven.power() > Rank::Six.power());
        assert!(Rank::Six.power() > Rank::Five.power());
        assert!(Rank::Five.power() > Rank::Four.power());
        assert!(Rank::Four.power() > Rank::Two.power());
    }

    #[test]
    fn card_index_round_trip_and_full_deck_size() {
        let deck = full_deck();
        assert_eq!(deck.len(), FULL_DECK_SIZE);

        for card in deck {
            let reconstructed = card_from_index(card.index()).expect("valid index");
            assert_eq!(card, reconstructed);
        }
    }
}
