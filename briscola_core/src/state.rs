use crate::bitset::CardMask;
use crate::card::{Card, Suit};
use crate::rules::{TrickWinner, trick_points, trick_winner};

/// Player identity from the perspective of the local agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    Me,
    Opponent,
}

impl Player {
    /// Returns the opposing player.
    pub fn other(self) -> Player {
        match self {
            Player::Me => Player::Opponent,
            Player::Opponent => Player::Me,
        }
    }
}

/// Public information available at decision time.
#[derive(Debug, Clone)]
pub struct PublicGameState {
    pub my_hand: Vec<Card>,
    pub opp_played: Option<Card>,
    pub briscola_suit: Suit,
    pub talon_len: usize,
    pub last_face_up_trump: Card,
    pub seen_cards: CardMask,
    pub score_me: u8,
    pub score_opp: u8,
    pub leader: Player,
}

impl PublicGameState {
    /// Returns legal moves for the active player.
    pub fn legal_moves(&self) -> Vec<Card> {
        self.my_hand.clone()
    }
}

/// Fully specified game state used by simulation/rollout.
#[derive(Debug, Clone)]
pub struct DeterminizedState {
    pub my_hand: Vec<Card>,
    pub opp_hand: Vec<Card>,
    pub talon: Vec<Card>,
    pub briscola_suit: Suit,
    pub face_up_trump: Card,
    pub score_me: u8,
    pub score_opp: u8,
    pub leader: Player,
    pub pending_lead: Option<Card>,
    pub pending_lead_by: Option<Player>,
}

/// State transition errors for determinized gameplay operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    InvalidTurn,
    CardNotInHand,
    MissingLead,
}

/// Outcome for a completed trick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrickOutcome {
    pub winner: Player,
    pub trick_points: u8,
}

impl DeterminizedState {
    /// Returns a player's current hand.
    pub fn hand(&self, player: Player) -> &[Card] {
        match player {
            Player::Me => &self.my_hand,
            Player::Opponent => &self.opp_hand,
        }
    }

    /// Returns legal moves for a player.
    pub fn legal_moves(&self, player: Player) -> Vec<Card> {
        self.hand(player).to_vec()
    }

    /// True when there are no cards left in hands or talon.
    pub fn is_terminal(&self) -> bool {
        self.my_hand.is_empty() && self.opp_hand.is_empty() && self.talon.is_empty()
    }

    /// Plays a lead card for the current trick leader.
    pub fn play_lead_card(&mut self, player: Player, card: Card) -> Result<(), StateError> {
        if self.pending_lead.is_some() || self.leader != player {
            return Err(StateError::InvalidTurn);
        }

        self.remove_card_from_hand(player, card)?;
        self.pending_lead = Some(card);
        self.pending_lead_by = Some(player);
        Ok(())
    }

    /// Plays a reply card and resolves the trick.
    pub fn play_reply_card(
        &mut self,
        player: Player,
        card: Card,
    ) -> Result<TrickOutcome, StateError> {
        let lead_card = self.pending_lead.ok_or(StateError::MissingLead)?;
        let lead_by = self.pending_lead_by.ok_or(StateError::MissingLead)?;
        if lead_by == player {
            return Err(StateError::InvalidTurn);
        }

        self.remove_card_from_hand(player, card)?;

        let winner = match trick_winner(lead_card, card, self.briscola_suit) {
            TrickWinner::Leader => lead_by,
            TrickWinner::Follower => player,
        };

        let points = trick_points(lead_card, card);
        self.add_score(winner, points);
        self.draw_phase(winner);

        self.leader = winner;
        self.pending_lead = None;
        self.pending_lead_by = None;

        Ok(TrickOutcome { winner, trick_points: points })
    }

    fn hand_mut(&mut self, player: Player) -> &mut Vec<Card> {
        match player {
            Player::Me => &mut self.my_hand,
            Player::Opponent => &mut self.opp_hand,
        }
    }

    fn remove_card_from_hand(&mut self, player: Player, card: Card) -> Result<(), StateError> {
        let hand = self.hand_mut(player);
        let position =
            hand.iter().position(|existing| *existing == card).ok_or(StateError::CardNotInHand)?;
        hand.remove(position);
        Ok(())
    }

    fn add_card_to_hand(&mut self, player: Player, card: Card) {
        self.hand_mut(player).push(card);
    }

    fn add_score(&mut self, player: Player, points: u8) {
        match player {
            Player::Me => self.score_me = self.score_me.saturating_add(points),
            Player::Opponent => self.score_opp = self.score_opp.saturating_add(points),
        }
    }

    fn draw_phase(&mut self, winner: Player) {
        if self.talon.is_empty() {
            return;
        }

        let loser = winner.other();
        let winner_draw = self.talon.remove(0);
        self.add_card_to_hand(winner, winner_draw);

        if self.talon.is_empty() {
            self.add_card_to_hand(loser, self.face_up_trump);
        } else {
            let loser_draw = self.talon.remove(0);
            self.add_card_to_hand(loser, loser_draw);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Rank, Suit};

    #[test]
    fn legal_moves_allow_any_card() {
        let state = PublicGameState {
            my_hand: vec![
                Card::new(Suit::Coins, Rank::Ace),
                Card::new(Suit::Clubs, Rank::Two),
                Card::new(Suit::Swords, Rank::King),
            ],
            opp_played: None,
            briscola_suit: Suit::Clubs,
            talon_len: 20,
            last_face_up_trump: Card::new(Suit::Clubs, Rank::Four),
            seen_cards: 0,
            score_me: 0,
            score_opp: 0,
            leader: Player::Me,
        };

        let legal = state.legal_moves();
        assert_eq!(legal.len(), 3);
        assert!(legal.contains(&Card::new(Suit::Coins, Rank::Ace)));
        assert!(legal.contains(&Card::new(Suit::Clubs, Rank::Two)));
        assert!(legal.contains(&Card::new(Suit::Swords, Rank::King)));
    }

    #[test]
    fn draws_face_up_trump_as_last_card() {
        let mut state = DeterminizedState {
            my_hand: vec![Card::new(Suit::Coins, Rank::Ace)],
            opp_hand: vec![Card::new(Suit::Swords, Rank::Two)],
            talon: vec![Card::new(Suit::Cups, Rank::Four)],
            briscola_suit: Suit::Clubs,
            face_up_trump: Card::new(Suit::Clubs, Rank::King),
            score_me: 0,
            score_opp: 0,
            leader: Player::Me,
            pending_lead: None,
            pending_lead_by: None,
        };

        state
            .play_lead_card(Player::Me, Card::new(Suit::Coins, Rank::Ace))
            .expect("lead should work");

        state
            .play_reply_card(Player::Opponent, Card::new(Suit::Swords, Rank::Two))
            .expect("reply should resolve");

        assert_eq!(state.talon.len(), 0);
        assert!(state.my_hand.contains(&Card::new(Suit::Cups, Rank::Four)));
        assert!(state.opp_hand.contains(&Card::new(Suit::Clubs, Rank::King)));
    }
}
