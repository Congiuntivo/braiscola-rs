use crate::card::{Card, FULL_DECK_SIZE};

pub type CardMask = u64;

pub const FULL_MASK: CardMask = (1u64 << FULL_DECK_SIZE) - 1;

pub fn card_mask(card: Card) -> CardMask {
    1u64 << card.index()
}

pub fn add(mask: CardMask, card: Card) -> CardMask {
    mask | card_mask(card)
}

pub fn contains(mask: CardMask, card: Card) -> bool {
    (mask & card_mask(card)) != 0
}
