use briscola_ai::mc::{BestMoveResult, MonteCarloConfig, choose_best_move};
use briscola_ai::rng::FastRng;
use briscola_ai::rollout::{choose_lead_card, choose_reply_card};
use briscola_core::bitset::{CardMask, add};
use briscola_core::card::{Card, full_deck};
use briscola_core::rules::TrickWinner;
use briscola_core::state::{DeterminizedState, Player, PublicGameState, StateError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayConfig {
    pub seed: u64,
    pub hint_samples: usize,
    pub opponent_samples: usize,
}

impl Default for PlayConfig {
    fn default() -> Self {
        Self { seed: 42, hint_samples: 128, opponent_samples: 96 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerTurnOutcome {
    pub lead_card: Card,
    pub reply_card: Card,
    pub winner: Player,
    pub trick_points: u8,
    pub opponent_card: Card,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayError {
    InvalidState,
    InvalidMove,
    NotPlayerTurn,
    OpponentMoveFailed,
    HintFailed,
}

pub struct PlayableGame {
    state: DeterminizedState,
    seen_by_me: CardMask,
    seen_by_opp: CardMask,
    completed_tricks: usize,
    opponent_rng: FastRng,
    hint_rng: FastRng,
    opponent_samples: usize,
    hint_samples: usize,
}

impl PlayableGame {
    pub fn new(config: PlayConfig) -> Result<Self, PlayError> {
        let mut deck = full_deck();
        let mut deck_rng = FastRng::new(config.seed);
        deck_rng.shuffle(&mut deck);

        if deck.len() != 40 {
            return Err(PlayError::InvalidState);
        }

        let mut cursor = 0;
        let mut my_hand = Vec::with_capacity(3);
        let mut opp_hand = Vec::with_capacity(3);

        for _ in 0..3 {
            my_hand.push(deck[cursor]);
            cursor += 1;
            opp_hand.push(deck[cursor]);
            cursor += 1;
        }

        let face_up_trump = deck[cursor];
        cursor += 1;
        let talon = deck[cursor..].to_vec();
        if talon.len() != 33 {
            return Err(PlayError::InvalidState);
        }
        let mut leader_rng = FastRng::new(config.seed ^ 0xC3C3_C3C3_7A7A_7A7A);
        let initial_leader =
            if leader_rng.gen_below(2) == 0 { Player::Me } else { Player::Opponent };

        let state = DeterminizedState {
            my_hand: my_hand.clone(),
            opp_hand: opp_hand.clone(),
            talon,
            briscola_suit: face_up_trump.suit,
            face_up_trump,
            score_me: 0,
            score_opp: 0,
            leader: initial_leader,
            pending_lead: None,
            pending_lead_by: None,
        };

        let mut seen_by_me = add(0, face_up_trump);
        let mut seen_by_opp = add(0, face_up_trump);
        for card in &my_hand {
            seen_by_me = add(seen_by_me, *card);
        }
        for card in &opp_hand {
            seen_by_opp = add(seen_by_opp, *card);
        }

        Ok(Self {
            state,
            seen_by_me,
            seen_by_opp,
            completed_tricks: 0,
            opponent_rng: FastRng::new(config.seed ^ 0xA1A1_A1A1_5E5E_5E5E),
            hint_rng: FastRng::new(config.seed ^ 0xB2B2_B2B2_6D6D_6D6D),
            opponent_samples: config.opponent_samples.max(1),
            hint_samples: config.hint_samples.max(1),
        })
    }

    pub fn set_hint_samples(&mut self, samples: usize) {
        self.hint_samples = samples.max(1);
    }

    pub fn set_opponent_samples(&mut self, samples: usize) {
        self.opponent_samples = samples.max(1);
    }

    pub fn completed_tricks(&self) -> usize {
        self.completed_tricks
    }

    pub fn is_game_over(&self) -> bool {
        self.state.is_terminal()
    }

    pub fn score_me(&self) -> u8 {
        self.state.score_me
    }

    pub fn score_opp(&self) -> u8 {
        self.state.score_opp
    }

    pub fn talon_len(&self) -> usize {
        self.state.talon.len()
    }

    pub fn opponent_cards_remaining(&self) -> usize {
        self.state.opp_hand.len()
    }

    pub fn leader(&self) -> Player {
        self.state.leader
    }

    pub fn briscola_card(&self) -> Card {
        self.state.face_up_trump
    }

    pub fn briscola_suit(&self) -> briscola_core::card::Suit {
        self.state.briscola_suit
    }

    pub fn my_hand(&self) -> &[Card] {
        &self.state.my_hand
    }

    pub fn current_opponent_lead(&self) -> Option<Card> {
        if self.state.pending_lead_by == Some(Player::Opponent) {
            self.state.pending_lead
        } else {
            None
        }
    }

    pub fn is_player_turn(&self) -> bool {
        if self.is_game_over() {
            return false;
        }
        self.current_opponent_lead().is_some()
            || (self.state.leader == Player::Me && self.state.pending_lead.is_none())
    }

    pub fn maybe_play_opponent_lead(&mut self) -> Result<Option<Card>, PlayError> {
        if self.is_game_over() {
            return Ok(None);
        }
        if self.state.leader != Player::Opponent || self.state.pending_lead.is_some() {
            return Ok(None);
        }

        let lead = self.choose_opponent_lead()?;
        self.state
            .play_lead_card(Player::Opponent, lead)
            .map_err(|_: StateError| PlayError::OpponentMoveFailed)?;
        self.seen_by_me = add(self.seen_by_me, lead);
        self.seen_by_opp = add(self.seen_by_opp, lead);
        Ok(Some(lead))
    }

    pub fn hint_best_move(&mut self) -> Result<BestMoveResult, PlayError> {
        if !self.is_player_turn() {
            return Err(PlayError::NotPlayerTurn);
        }

        let public_state = PublicGameState {
            my_hand: self.state.my_hand.clone(),
            opp_played: self.current_opponent_lead(),
            briscola_suit: self.state.briscola_suit,
            talon_len: self.state.talon.len(),
            last_face_up_trump: self.state.face_up_trump,
            seen_cards: self.seen_by_me,
            score_me: self.state.score_me,
            score_opp: self.state.score_opp,
            leader: self.state.leader,
        };

        choose_best_move(
            &public_state,
            &mut self.hint_rng,
            MonteCarloConfig { samples_per_move: self.hint_samples },
        )
        .map_err(|_| PlayError::HintFailed)
    }

    pub fn play_player_card(&mut self, card: Card) -> Result<PlayerTurnOutcome, PlayError> {
        if !self.is_player_turn() {
            return Err(PlayError::NotPlayerTurn);
        }

        if !self.state.my_hand.contains(&card) {
            return Err(PlayError::InvalidMove);
        }

        let old_my_hand = self.state.my_hand.clone();
        let old_opp_hand = self.state.opp_hand.clone();

        let (lead_card, reply_card, opponent_card, outcome) =
            if let Some(opp_lead) = self.current_opponent_lead() {
                self.seen_by_me = add(self.seen_by_me, card);
                self.seen_by_opp = add(self.seen_by_opp, card);

                let outcome = self
                    .state
                    .play_reply_card(Player::Me, card)
                    .map_err(|_: StateError| PlayError::InvalidMove)?;
                (opp_lead, card, opp_lead, outcome)
            } else {
                self.state
                    .play_lead_card(Player::Me, card)
                    .map_err(|_: StateError| PlayError::InvalidMove)?;
                self.seen_by_me = add(self.seen_by_me, card);
                self.seen_by_opp = add(self.seen_by_opp, card);

                let opp_reply = self.choose_opponent_reply(card)?;
                self.seen_by_me = add(self.seen_by_me, opp_reply);
                self.seen_by_opp = add(self.seen_by_opp, opp_reply);
                let outcome = self
                    .state
                    .play_reply_card(Player::Opponent, opp_reply)
                    .map_err(|_: StateError| PlayError::OpponentMoveFailed)?;
                (card, opp_reply, opp_reply, outcome)
            };

        if let Some(my_draw) = drawn_card(&old_my_hand, &self.state.my_hand) {
            self.seen_by_me = add(self.seen_by_me, my_draw);
        }

        if let Some(opp_draw) = drawn_card(&old_opp_hand, &self.state.opp_hand) {
            self.seen_by_opp = add(self.seen_by_opp, opp_draw);
        }

        self.completed_tricks += 1;

        Ok(PlayerTurnOutcome {
            lead_card,
            reply_card,
            winner: outcome.winner,
            trick_points: outcome.trick_points,
            opponent_card,
        })
    }

    fn choose_opponent_lead(&mut self) -> Result<Card, PlayError> {
        if self.opponent_samples == 0 {
            return Ok(choose_lead_card(&self.state, Player::Opponent));
        }

        let public = PublicGameState {
            my_hand: self.state.opp_hand.clone(),
            opp_played: None,
            briscola_suit: self.state.briscola_suit,
            talon_len: self.state.talon.len(),
            last_face_up_trump: self.state.face_up_trump,
            seen_cards: self.seen_by_opp,
            score_me: self.state.score_opp,
            score_opp: self.state.score_me,
            leader: Player::Me,
        };

        choose_best_move(
            &public,
            &mut self.opponent_rng,
            MonteCarloConfig { samples_per_move: self.opponent_samples },
        )
        .map(|result| result.best_move)
        .map_err(|_| PlayError::OpponentMoveFailed)
    }

    fn choose_opponent_reply(&mut self, my_lead: Card) -> Result<Card, PlayError> {
        if self.opponent_samples == 0 {
            return Ok(choose_reply_card(&self.state, Player::Opponent, my_lead));
        }

        let mut seen = self.seen_by_opp;
        seen = add(seen, my_lead);

        let public = PublicGameState {
            my_hand: self.state.opp_hand.clone(),
            opp_played: Some(my_lead),
            briscola_suit: self.state.briscola_suit,
            talon_len: self.state.talon.len(),
            last_face_up_trump: self.state.face_up_trump,
            seen_cards: seen,
            score_me: self.state.score_opp,
            score_opp: self.state.score_me,
            leader: Player::Opponent,
        };

        choose_best_move(
            &public,
            &mut self.opponent_rng,
            MonteCarloConfig { samples_per_move: self.opponent_samples },
        )
        .map(|result| result.best_move)
        .map_err(|_| PlayError::OpponentMoveFailed)
    }
}

fn drawn_card(old_hand: &[Card], new_hand: &[Card]) -> Option<Card> {
    for card in new_hand {
        if !old_hand.contains(card) {
            return Some(*card);
        }
    }
    None
}

pub fn winner_from_scores(score_me: u8, score_opp: u8) -> Option<Player> {
    match score_me.cmp(&score_opp) {
        std::cmp::Ordering::Greater => Some(Player::Me),
        std::cmp::Ordering::Less => Some(Player::Opponent),
        std::cmp::Ordering::Equal => None,
    }
}

pub fn trick_winner_for_display(
    leader: Player,
    lead: Card,
    reply: Card,
    briscola: briscola_core::card::Suit,
) -> Player {
    if briscola_core::rules::trick_winner(lead, reply, briscola) == TrickWinner::Leader {
        leader
    } else {
        leader.other()
    }
}

#[cfg(test)]
mod tests {
    use super::{PlayConfig, PlayableGame};
    use briscola_core::state::Player;

    #[test]
    fn new_game_has_expected_initial_sizes() {
        let game = PlayableGame::new(PlayConfig::default());
        assert!(game.is_ok());

        let Ok(game) = game else {
            return;
        };

        assert_eq!(game.my_hand().len(), 3);
        assert_eq!(game.opponent_cards_remaining(), 3);
        assert_eq!(game.talon_len(), 33);
    }

    #[test]
    fn game_progresses_after_one_player_move() {
        let Ok(mut game) = PlayableGame::new(PlayConfig::default()) else {
            return;
        };

        if game.maybe_play_opponent_lead().is_err() {
            return;
        }

        let Some(card) = game.my_hand().first().copied() else {
            return;
        };

        let outcome = game.play_player_card(card);
        assert!(outcome.is_ok());
        assert_eq!(game.completed_tricks(), 1);
    }

    #[test]
    fn hint_returns_legal_card() {
        let Ok(mut game) = PlayableGame::new(PlayConfig::default()) else {
            return;
        };

        if game.maybe_play_opponent_lead().is_err() {
            return;
        }

        let hint = game.hint_best_move();
        assert!(hint.is_ok());

        let Ok(hint) = hint else {
            return;
        };

        assert!(game.my_hand().contains(&hint.best_move));
    }

    #[test]
    fn initial_leader_is_seeded_and_can_be_either_player() {
        let same_seed_a = PlayableGame::new(PlayConfig { seed: 777, ..PlayConfig::default() })
            .expect("game should initialize");
        let same_seed_b = PlayableGame::new(PlayConfig { seed: 777, ..PlayConfig::default() })
            .expect("game should initialize");
        assert_eq!(same_seed_a.leader(), same_seed_b.leader());

        let mut saw_me = false;
        let mut saw_opponent = false;
        for seed in 1..=64_u64 {
            let game = PlayableGame::new(PlayConfig { seed, ..PlayConfig::default() })
                .expect("game should initialize");
            match game.leader() {
                Player::Me => saw_me = true,
                Player::Opponent => saw_opponent = true,
            }
        }

        assert!(saw_me && saw_opponent);
    }
}
