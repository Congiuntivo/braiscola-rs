use briscola_ai::mc::{MonteCarloConfig, MonteCarloError, choose_best_move};
use briscola_ai::rng::FastRng;
use briscola_ai::rollout::{choose_lead_card, choose_reply_card};
use briscola_core::bitset::{CardMask, add};
use briscola_core::card::{Card, Suit, full_deck};
use briscola_core::state::{DeterminizedState, Player, PublicGameState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MePolicy {
    Heuristic,
    BestMove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulationOptions {
    pub me_policy: MePolicy,
    pub samples_per_move: usize,
}

impl Default for SimulationOptions {
    fn default() -> Self {
        Self { me_policy: MePolicy::Heuristic, samples_per_move: 128 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameWinner {
    Me,
    Opponent,
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimulationError {
    InvalidDeck,
    InvalidTransition,
    StrategyFailed(MonteCarloError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrickLog {
    pub number: usize,
    pub leader: Player,
    pub lead_card: Card,
    pub follower: Player,
    pub reply_card: Card,
    pub winner: Player,
    pub trick_points: u8,
    pub score_me: u8,
    pub score_opp: u8,
    pub my_hand_len: usize,
    pub opp_hand_len: usize,
    pub talon_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameLog {
    pub seed: u64,
    pub briscola_suit: Suit,
    pub face_up_trump: Card,
    pub initial_my_hand: Vec<Card>,
    pub initial_opp_hand: Vec<Card>,
    pub tricks: Vec<TrickLog>,
    pub final_score_me: u8,
    pub final_score_opp: u8,
    pub winner: GameWinner,
}

pub fn simulate_game(seed: u64) -> Result<GameLog, SimulationError> {
    simulate_game_with_options(seed, SimulationOptions::default())
}

pub fn simulate_game_with_options(
    seed: u64,
    options: SimulationOptions,
) -> Result<GameLog, SimulationError> {
    let mut state = initial_state(seed)?;
    let mut best_move_rng = FastRng::new(seed ^ 0xA5A5_A5A5_5A5A_5A5A);
    let initial_my_hand = state.my_hand.clone();
    let initial_opp_hand = state.opp_hand.clone();
    let briscola_suit = state.briscola_suit;
    let face_up_trump = state.face_up_trump;
    let mut opp_revealed_cards = 0_u64;

    let mut tricks = Vec::with_capacity(20);

    while !state.is_terminal() {
        let number = tricks.len() + 1;
        let leader = state.leader;
        let lead_card = if leader == Player::Me {
            let seen_cards = visible_seen_mask(&state, opp_revealed_cards, None);
            select_me_card(&state, seen_cards, None, options, &mut best_move_rng)?
        } else {
            choose_lead_card(&state, leader)
        };

        if leader == Player::Opponent {
            opp_revealed_cards = add(opp_revealed_cards, lead_card);
        }

        if state.play_lead_card(leader, lead_card).is_err() {
            return Err(SimulationError::InvalidTransition);
        }

        let follower = leader.other();
        let reply_card = if follower == Player::Me {
            let seen_cards = visible_seen_mask(&state, opp_revealed_cards, Some(lead_card));
            select_me_card(&state, seen_cards, Some(lead_card), options, &mut best_move_rng)?
        } else {
            choose_reply_card(&state, follower, lead_card)
        };

        if follower == Player::Opponent {
            opp_revealed_cards = add(opp_revealed_cards, reply_card);
        }

        let Ok(outcome) = state.play_reply_card(follower, reply_card) else {
            return Err(SimulationError::InvalidTransition);
        };

        tricks.push(TrickLog {
            number,
            leader,
            lead_card,
            follower,
            reply_card,
            winner: outcome.winner,
            trick_points: outcome.trick_points,
            score_me: state.score_me,
            score_opp: state.score_opp,
            my_hand_len: state.my_hand.len(),
            opp_hand_len: state.opp_hand.len(),
            talon_len: state.talon.len(),
        });
    }

    let winner = match state.score_me.cmp(&state.score_opp) {
        std::cmp::Ordering::Greater => GameWinner::Me,
        std::cmp::Ordering::Less => GameWinner::Opponent,
        std::cmp::Ordering::Equal => GameWinner::Draw,
    };

    Ok(GameLog {
        seed,
        briscola_suit,
        face_up_trump,
        initial_my_hand,
        initial_opp_hand,
        tricks,
        final_score_me: state.score_me,
        final_score_opp: state.score_opp,
        winner,
    })
}

fn visible_seen_mask(
    state: &DeterminizedState,
    opp_revealed_cards: CardMask,
    opp_played: Option<Card>,
) -> CardMask {
    let mut seen_cards = add(opp_revealed_cards, state.face_up_trump);
    for card in &state.my_hand {
        seen_cards = add(seen_cards, *card);
    }
    if let Some(card) = opp_played {
        seen_cards = add(seen_cards, card);
    }
    seen_cards
}

fn select_me_card(
    state: &DeterminizedState,
    seen_cards: CardMask,
    opp_played: Option<Card>,
    options: SimulationOptions,
    rng: &mut FastRng,
) -> Result<Card, SimulationError> {
    match options.me_policy {
        MePolicy::Heuristic => {
            if let Some(opp_card) = opp_played {
                Ok(choose_reply_card(state, Player::Me, opp_card))
            } else {
                Ok(choose_lead_card(state, Player::Me))
            }
        }
        MePolicy::BestMove => {
            let public = PublicGameState {
                my_hand: state.my_hand.clone(),
                opp_played,
                briscola_suit: state.briscola_suit,
                talon_len: state.talon.len(),
                last_face_up_trump: state.face_up_trump,
                seen_cards,
                score_me: state.score_me,
                score_opp: state.score_opp,
                leader: state.leader,
            };

            let result = choose_best_move(
                &public,
                rng,
                MonteCarloConfig { samples_per_move: options.samples_per_move },
            )
            .map_err(SimulationError::StrategyFailed)?;
            Ok(result.best_move)
        }
    }
}

fn initial_state(seed: u64) -> Result<DeterminizedState, SimulationError> {
    let mut deck = full_deck();
    if deck.len() != 40 {
        return Err(SimulationError::InvalidDeck);
    }

    let mut rng = FastRng::new(seed);
    rng.shuffle(&mut deck);

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
        return Err(SimulationError::InvalidDeck);
    }

    Ok(DeterminizedState {
        my_hand,
        opp_hand,
        talon,
        briscola_suit: face_up_trump.suit,
        face_up_trump,
        score_me: 0,
        score_opp: 0,
        leader: Player::Me,
        pending_lead: None,
        pending_lead_by: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GameWinner, MePolicy, SimulationOptions, simulate_game, simulate_game_with_options,
    };

    #[test]
    fn simulation_completes_with_20_tricks_and_120_points() {
        let result = simulate_game(42);
        assert!(result.is_ok());

        let Ok(log) = result else {
            return;
        };

        assert_eq!(log.tricks.len(), 20);
        assert_eq!(u16::from(log.final_score_me) + u16::from(log.final_score_opp), 120);
    }

    #[test]
    fn simulation_is_deterministic_for_same_seed() {
        let first = simulate_game(7);
        let second = simulate_game(7);
        assert!(first.is_ok());
        assert!(second.is_ok());

        let Ok(first_log) = first else {
            return;
        };
        let Ok(second_log) = second else {
            return;
        };

        assert_eq!(first_log, second_log);
    }

    #[test]
    fn winner_matches_final_score_relation() {
        let result = simulate_game(99);
        assert!(result.is_ok());

        let Ok(log) = result else {
            return;
        };

        match log.final_score_me.cmp(&log.final_score_opp) {
            std::cmp::Ordering::Greater => assert_eq!(log.winner, GameWinner::Me),
            std::cmp::Ordering::Less => assert_eq!(log.winner, GameWinner::Opponent),
            std::cmp::Ordering::Equal => assert_eq!(log.winner, GameWinner::Draw),
        }
    }

    #[test]
    fn best_move_policy_simulation_completes() {
        let result = simulate_game_with_options(
            42,
            SimulationOptions { me_policy: MePolicy::BestMove, samples_per_move: 128 },
        );
        assert!(result.is_ok());

        let Ok(log) = result else {
            return;
        };

        assert_eq!(log.tricks.len(), 20);
        assert_eq!(u16::from(log.final_score_me) + u16::from(log.final_score_opp), 120);
    }
}
