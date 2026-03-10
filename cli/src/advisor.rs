use std::fs;
use std::path::Path;

use briscola_ai::mc::{BestMoveResult, MonteCarloConfig, choose_best_move};
use briscola_ai::rng::FastRng;
use briscola_core::bitset::{CardMask, add, contains};
use briscola_core::card::{Card, HAND_SIZE, INITIAL_TALON_SIZE, Rank, Suit, full_deck};
use briscola_core::rules::{TrickWinner, trick_points, trick_winner};
use briscola_core::state::{Player, PublicGameState};
use serde::Deserialize;

/// Initial public state required to bootstrap an interactive advisor session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractiveInit {
    pub briscola_suit: Suit,
    pub face_up_trump: Card,
    pub leader: Player,
    pub talon_len: usize,
    pub score_me: u8,
    pub score_opp: u8,
}

/// Score and winner information for a resolved turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TurnResult {
    pub winner: Player,
    pub points: u8,
    pub score_me: u8,
    pub score_opp: u8,
    pub talon_len: usize,
}

/// Interactive advisor session errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionError {
    InvalidSetup,
    InvalidMove,
    InvalidDraw,
    SuggestionFailed,
}

/// Stateful advisor session for turn-by-turn manual play tracking.
#[derive(Debug, Clone)]
pub struct InteractiveSession {
    briscola_suit: Suit,
    face_up_trump: Card,
    my_hand: Vec<Card>,
    seen_cards: CardMask,
    score_me: u8,
    score_opp: u8,
    talon_len: usize,
    leader: Player,
    trick_number: usize,
}

impl InteractiveSession {
    /// Creates a new session from public game setup and current hand.
    pub fn new(init: InteractiveInit, my_initial_hand: Vec<Card>) -> Result<Self, SessionError> {
        if init.face_up_trump.suit != init.briscola_suit {
            return Err(SessionError::InvalidSetup);
        }
        if my_initial_hand.is_empty()
            || my_initial_hand.len() > HAND_SIZE
            || init.talon_len > INITIAL_TALON_SIZE
        {
            return Err(SessionError::InvalidSetup);
        }

        let mut seen_cards = add(0, init.face_up_trump);
        for card in &my_initial_hand {
            seen_cards = add(seen_cards, *card);
        }

        Ok(Self {
            briscola_suit: init.briscola_suit,
            face_up_trump: init.face_up_trump,
            my_hand: my_initial_hand,
            seen_cards,
            score_me: init.score_me,
            score_opp: init.score_opp,
            talon_len: init.talon_len,
            leader: init.leader,
            trick_number: 1,
        })
    }

    pub fn my_hand(&self) -> &[Card] {
        &self.my_hand
    }

    pub fn leader(&self) -> Player {
        self.leader
    }

    pub fn trick_number(&self) -> usize {
        self.trick_number
    }

    pub fn score_me(&self) -> u8 {
        self.score_me
    }

    pub fn score_opp(&self) -> u8 {
        self.score_opp
    }

    pub fn talon_len(&self) -> usize {
        self.talon_len
    }

    pub fn game_over(&self) -> bool {
        self.my_hand.is_empty() && self.talon_len == 0
    }

    /// Computes move suggestions for the current turn.
    pub fn suggest_move(
        &self,
        opp_played: Option<Card>,
        rng: &mut FastRng,
        samples_per_move: usize,
    ) -> Result<BestMoveResult, SessionError> {
        let mut seen_cards = self.seen_cards;
        if let Some(card) = opp_played {
            seen_cards = add(seen_cards, card);
        }

        let public_state = PublicGameState {
            my_hand: self.my_hand.clone(),
            opp_played,
            briscola_suit: self.briscola_suit,
            talon_len: self.talon_len,
            last_face_up_trump: self.face_up_trump,
            seen_cards,
            score_me: self.score_me,
            score_opp: self.score_opp,
            leader: self.leader,
        };

        choose_best_move(
            &public_state,
            rng,
            MonteCarloConfig { samples_per_move: samples_per_move.max(1) },
        )
        .map_err(|_| SessionError::SuggestionFailed)
    }

    /// Applies a completed turn and advances session state.
    pub fn apply_turn(
        &mut self,
        opp_played: Option<Card>,
        my_played: Card,
        opp_reply_if_me_lead: Option<Card>,
        my_draw_card: Option<Card>,
    ) -> Result<TurnResult, SessionError> {
        let hand_index = self
            .my_hand
            .iter()
            .position(|card| *card == my_played)
            .ok_or(SessionError::InvalidMove)?;

        let (lead_card, reply_card) = if self.leader == Player::Me {
            let opp_reply = opp_reply_if_me_lead.ok_or(SessionError::InvalidMove)?;
            (my_played, opp_reply)
        } else {
            let opp_lead = opp_played.ok_or(SessionError::InvalidMove)?;
            (opp_lead, my_played)
        };

        let winner =
            if trick_winner(lead_card, reply_card, self.briscola_suit) == TrickWinner::Leader {
                self.leader
            } else {
                self.leader.other()
            };

        let points = trick_points(lead_card, reply_card);
        if winner == Player::Me {
            self.score_me = self.score_me.saturating_add(points);
        } else {
            self.score_opp = self.score_opp.saturating_add(points);
        }

        self.my_hand.remove(hand_index);
        self.seen_cards = add(self.seen_cards, my_played);
        self.seen_cards = add(self.seen_cards, lead_card);
        self.seen_cards = add(self.seen_cards, reply_card);

        if self.talon_len > 0 {
            if self.talon_len == 1 {
                if winner == Player::Me {
                    let drawn = my_draw_card.ok_or(SessionError::InvalidDraw)?;
                    if contains(self.seen_cards, drawn) {
                        return Err(SessionError::InvalidDraw);
                    }
                    self.my_hand.push(drawn);
                    self.seen_cards = add(self.seen_cards, drawn);
                } else {
                    self.my_hand.push(self.face_up_trump);
                }
                self.talon_len = 0;
            } else {
                let drawn = my_draw_card.ok_or(SessionError::InvalidDraw)?;
                if contains(self.seen_cards, drawn) {
                    return Err(SessionError::InvalidDraw);
                }
                self.my_hand.push(drawn);
                self.seen_cards = add(self.seen_cards, drawn);
                self.talon_len -= 2;
            }
        }

        self.leader = winner;
        self.trick_number += 1;

        Ok(TurnResult {
            winner,
            points,
            score_me: self.score_me,
            score_opp: self.score_opp,
            talon_len: self.talon_len,
        })
    }
}

/// JSON payload used by `advisor suggest`.
#[derive(Debug, Deserialize)]
pub struct JsonSuggestionInput {
    pub briscola_suit: String,
    pub face_up_trump: String,
    pub my_hand: Vec<String>,
    pub opp_played: Option<String>,
    pub talon_len: usize,
    pub score_me: u8,
    pub score_opp: u8,
    pub leader: String,
    #[serde(default)]
    pub history: Vec<JsonTrick>,
    #[serde(default)]
    pub seen_cards: Vec<String>,
    pub samples_per_move: Option<usize>,
    pub seed: Option<u64>,
}

/// One completed trick from historical play.
#[derive(Debug, Deserialize)]
pub struct JsonTrick {
    pub lead: String,
    pub reply: String,
}

/// Computes best move suggestion from JSON file input.
pub fn suggest_from_json_path(
    path: &Path,
    samples_override: Option<usize>,
    seed_override: Option<u64>,
) -> Result<BestMoveResult, String> {
    let content =
        fs::read_to_string(path).map_err(|error| format!("cannot read json file: {error}"))?;
    let input: JsonSuggestionInput =
        serde_json::from_str(&content).map_err(|error| format!("invalid json input: {error}"))?;
    suggest_from_json_input(&input, samples_override, seed_override)
}

/// Computes best move suggestion from parsed JSON input.
pub fn suggest_from_json_input(
    input: &JsonSuggestionInput,
    samples_override: Option<usize>,
    seed_override: Option<u64>,
) -> Result<BestMoveResult, String> {
    let briscola_suit = parse_suit(&input.briscola_suit)?;
    let face_up_trump = parse_card(&input.face_up_trump)?;
    if face_up_trump.suit != briscola_suit {
        return Err(String::from("face_up_trump suit must match briscola_suit"));
    }

    let my_hand = parse_card_list(&input.my_hand)?;
    if my_hand.is_empty() || my_hand.len() > HAND_SIZE {
        return Err(format!("my_hand must contain 1 to {HAND_SIZE} cards"));
    }

    let opp_played = match &input.opp_played {
        Some(card) => Some(parse_card(card)?),
        None => None,
    };

    let leader = parse_player(&input.leader)?;
    let mut seen_cards = add(0, face_up_trump);

    for trick in &input.history {
        seen_cards = add(seen_cards, parse_card(&trick.lead)?);
        seen_cards = add(seen_cards, parse_card(&trick.reply)?);
    }

    for card in &input.seen_cards {
        seen_cards = add(seen_cards, parse_card(card)?);
    }

    for card in &my_hand {
        seen_cards = add(seen_cards, *card);
    }

    if let Some(card) = opp_played {
        seen_cards = add(seen_cards, card);
    }

    let public_state = PublicGameState {
        my_hand,
        opp_played,
        briscola_suit,
        talon_len: input.talon_len,
        last_face_up_trump: face_up_trump,
        seen_cards,
        score_me: input.score_me,
        score_opp: input.score_opp,
        leader,
    };

    let samples_per_move = samples_override.or(input.samples_per_move).unwrap_or(128);
    let mut rng = FastRng::new(seed_override.or(input.seed).unwrap_or(42));

    choose_best_move(
        &public_state,
        &mut rng,
        MonteCarloConfig { samples_per_move: samples_per_move.max(1) },
    )
    .map_err(|error| format!("suggestion failed: {error:?}"))
}

/// Parses a list of card tokens.
pub fn parse_card_list(items: &[String]) -> Result<Vec<Card>, String> {
    let mut cards = Vec::with_capacity(items.len());
    for item in items {
        cards.push(parse_card(item)?);
    }
    Ok(cards)
}

/// Parses a card token in compact (`🪙A`, `oA`) or explicit (`coins:A`) format.
pub fn parse_card(input: &str) -> Result<Card, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(String::from("empty card string"));
    }

    let (suit_part, rank_part) = if let Some((left, right)) = trimmed.split_once(':') {
        (left.trim(), right.trim())
    } else {
        split_compact_card(trimmed).ok_or_else(|| format!("invalid card notation '{trimmed}'"))?
    };

    let suit = parse_suit(suit_part)?;
    let rank = parse_rank(rank_part)?;
    Ok(Card::new(suit, rank))
}

/// Formats a card using compact emoji suit notation.
pub fn format_card(card: Card) -> String {
    let suit_token = match card.suit {
        Suit::Coins => "🪙",
        Suit::Cups => "🏆",
        Suit::Swords => "⚔️",
        Suit::Clubs => "🪄",
    };

    let rank_token = match card.rank {
        Rank::Ace => "A",
        Rank::Two => "2",
        Rank::Three => "3",
        Rank::Four => "4",
        Rank::Five => "5",
        Rank::Six => "6",
        Rank::Seven => "7",
        Rank::Jack => "J",
        Rank::Queen => "Q",
        Rank::King => "K",
    };

    format!("{suit_token}{rank_token}")
}

fn split_compact_card(input: &str) -> Option<(&str, &str)> {
    for prefix in ["⚔️", "🪙", "🏆", "🪄", "o", "u", "s", "c", "d", "b"] {
        if let Some(rest) = input.strip_prefix(prefix)
            && !rest.trim().is_empty()
        {
            return Some((prefix, rest.trim()));
        }
    }
    None
}

/// Parses player token (`me`/`opponent` aliases accepted).
pub fn parse_player(input: &str) -> Result<Player, String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "me" | "m" | "player" => Ok(Player::Me),
        "opponent" | "opp" | "o" => Ok(Player::Opponent),
        _ => Err(format!("invalid player '{input}' (use me/opponent)")),
    }
}

/// Parses suit token in English, Italian, legacy short, or emoji form.
pub fn parse_suit(input: &str) -> Result<Suit, String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "coins" | "coin" | "o" | "oro" | "denari" | "denaro" | "d" | "🪙" => Ok(Suit::Coins),
        "cups" | "cup" | "u" | "coppe" | "coppa" | "🏆" => Ok(Suit::Cups),
        "swords" | "sword" | "s" | "spade" | "⚔️" | "⚔" => Ok(Suit::Swords),
        "clubs" | "club" | "c" | "bastoni" | "b" | "🪄" => Ok(Suit::Clubs),
        _ => Err(format!("invalid suit '{input}'")),
    }
}

fn parse_rank(input: &str) -> Result<Rank, String> {
    match input.trim().to_ascii_lowercase().as_str() {
        "a" | "ace" | "asso" => Ok(Rank::Ace),
        "2" => Ok(Rank::Two),
        "3" => Ok(Rank::Three),
        "4" => Ok(Rank::Four),
        "5" => Ok(Rank::Five),
        "6" => Ok(Rank::Six),
        "7" => Ok(Rank::Seven),
        "j" | "jack" | "fante" => Ok(Rank::Jack),
        "q" | "queen" | "cavallo" => Ok(Rank::Queen),
        "k" | "king" | "re" => Ok(Rank::King),
        _ => Err(format!("invalid rank '{input}'")),
    }
}

/// Returns a full deck excluding the given cards.
pub fn all_cards_except(excluded: &[Card]) -> Vec<Card> {
    let mut cards = Vec::new();
    for card in full_deck() {
        if !excluded.contains(&card) {
            cards.push(card);
        }
    }
    cards
}

#[cfg(test)]
mod tests {
    use super::{
        JsonSuggestionInput, all_cards_except, format_card, parse_card, suggest_from_json_input,
    };
    use briscola_core::card::{Card, Rank, Suit};

    #[test]
    fn parse_card_supports_compact_and_colon_formats() {
        assert_eq!(parse_card("oA").ok(), Some(Card::new(Suit::Coins, Rank::Ace)));
        assert_eq!(parse_card("🪙A").ok(), Some(Card::new(Suit::Coins, Rank::Ace)));
        assert_eq!(parse_card("⚔️7").ok(), Some(Card::new(Suit::Swords, Rank::Seven)));
        assert_eq!(parse_card("clubs:K").ok(), Some(Card::new(Suit::Clubs, Rank::King)));
        assert!(parse_card("bad").is_err());
    }

    #[test]
    fn suggest_from_json_returns_legal_card() {
        let winning = Card::new(Suit::Coins, Rank::Ace);
        let losing = Card::new(Suit::Swords, Rank::Two);
        let opp_played = Card::new(Suit::Coins, Rank::King);
        let face_up = Card::new(Suit::Clubs, Rank::Four);
        let hidden = Card::new(Suit::Clubs, Rank::Ace);

        let mut seen_cards = Vec::new();
        for card in all_cards_except(&[winning, losing, hidden]) {
            seen_cards.push(format_card(card));
        }

        let input = JsonSuggestionInput {
            briscola_suit: String::from("clubs"),
            face_up_trump: format_card(face_up),
            my_hand: vec![format_card(winning), format_card(losing)],
            opp_played: Some(format_card(opp_played)),
            talon_len: 0,
            score_me: 50,
            score_opp: 48,
            leader: String::from("opponent"),
            history: Vec::new(),
            seen_cards,
            samples_per_move: Some(32),
            seed: Some(13),
        };

        let result = suggest_from_json_input(&input, None, None);
        assert!(result.is_ok());

        let Ok(best) = result else {
            return;
        };

        assert_eq!(best.best_move, winning);
    }
}
