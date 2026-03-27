//! Public and determinized Briscola game states plus transition logic.
//!
//! Two state views are modeled:
//! - [PublicGameState]: information available to a decision-making player.
//! - [DeterminizedState]: fully specified world used for simulation.

use crate::bitset::CardMask;
use crate::card::{Card, Suit};
use crate::rules::{TrickWinner, trick_points, trick_winner};

/// Player identity from the perspective of the local agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Player {
    /// The local player perspective.
    Me,
    /// The opposing player.
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
    /// Cards currently in my hand.
    pub my_hand: Vec<Card>,
    /// Opponent lead card when I must reply, otherwise none.
    pub opp_played: Option<Card>,
    /// Trump suit for this match.
    pub briscola_suit: Suit,
    /// Cards still in talon excluding the exposed face-up trump.
    pub talon_len: usize,
    /// Face-up trump card under the talon.
    pub last_face_up_trump: Card,
    /// Set of cards already visible in history or in known hands.
    pub seen_cards: CardMask,
    /// My current score.
    pub score_me: u8,
    /// Opponent current score.
    pub score_opp: u8,
    /// Player who leads when there is no pending lead card.
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
    /// My current hand.
    pub my_hand: Vec<Card>,
    /// Opponent hand in this sampled world.
    pub opp_hand: Vec<Card>,
    /// Remaining draw pile order.
    pub talon: Vec<Card>,
    /// Trump suit.
    pub briscola_suit: Suit,
    /// Face-up trump card awarded when talon runs out after winner draw.
    pub face_up_trump: Card,
    /// My score.
    pub score_me: u8,
    /// Opponent score.
    pub score_opp: u8,
    /// Current leader when no pending trick exists.
    pub leader: Player,
    /// Current lead card waiting for reply.
    pub pending_lead: Option<Card>,
    /// Player who played pending lead.
    pub pending_lead_by: Option<Player>,
}

/// State transition errors for determinized gameplay operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateError {
    /// Action does not match turn/trick phase constraints.
    InvalidTurn,
    /// Attempted to play a card not present in actor hand.
    CardNotInHand,
    /// Reply attempted without a pending lead card.
    MissingLead,
}

/// Outcome for a completed trick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrickOutcome {
    /// Player that wins the trick.
    pub winner: Player,
    /// Points captured by winner from the two played cards.
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
    ///
    /// # Parameters
    ///
    /// - `player`: Player trying to lead.
    /// - `card`: Card to play.
    ///
    /// # Errors
    ///
    /// Returns [StateError::InvalidTurn] if a trick is already pending or
    /// `player` is not the leader. Returns [StateError::CardNotInHand] when
    /// `card` is not present in player hand.
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
    ///
    /// # Parameters
    ///
    /// - `player`: Player replying to the pending lead.
    /// - `card`: Reply card to play.
    ///
    /// # Returns
    ///
    /// [TrickOutcome] containing winner and points earned in this trick.
    ///
    /// # Errors
    ///
    /// Returns [StateError::MissingLead] when no pending lead exists,
    /// [StateError::InvalidTurn] when leader tries to reply, or
    /// [StateError::CardNotInHand] when `card` is absent from hand.
    ///
    /// # Algorithm
    ///
    /// 1. Validate pending lead and actor role.
    /// 2. Remove reply card from actor hand.
    /// 3. Resolve winner and add trick points.
    /// 4. Perform draw phase (winner first, loser second).
    /// 5. Reset pending trick and assign new leader.
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

    /// Returns mutable access to player hand.
    fn hand_mut(&mut self, player: Player) -> &mut Vec<Card> {
        match player {
            Player::Me => &mut self.my_hand,
            Player::Opponent => &mut self.opp_hand,
        }
    }

    /// Removes `card` from player hand.
    fn remove_card_from_hand(&mut self, player: Player, card: Card) -> Result<(), StateError> {
        let hand = self.hand_mut(player);
        let position =
            hand.iter().position(|existing| *existing == card).ok_or(StateError::CardNotInHand)?;
        hand.remove(position);
        Ok(())
    }

    /// Appends `card` to player hand.
    fn add_card_to_hand(&mut self, player: Player, card: Card) {
        self.hand_mut(player).push(card);
    }

    /// Adds trick points to player score, saturating at u8 bounds.
    fn add_score(&mut self, player: Player, points: u8) {
        match player {
            Player::Me => self.score_me = self.score_me.saturating_add(points),
            Player::Opponent => self.score_opp = self.score_opp.saturating_add(points),
        }
    }

    /// Resolves post-trick draw order.
    ///
    /// Winner draws first from talon. Loser draws second from talon, or takes
    /// the face-up trump when talon becomes empty after winner draw.
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
