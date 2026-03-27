//! Bitset utilities for representing sets of cards.
//!
//! Each card maps to one bit in a 64-bit mask using canonical deck index.
//! Only the lowest 40 bits are used for Briscola cards.

use crate::card::{Card, FULL_DECK_SIZE};

/// Bitmask type used to represent card sets.
pub type CardMask = u64;

/// Mask with all valid card bits set.
pub const FULL_MASK: CardMask = (1u64 << FULL_DECK_SIZE) - 1;

/// Returns a single-card mask with the bit of `card` set.
pub fn card_mask(card: Card) -> CardMask {
    1u64 << card.index()
}

/// Returns `mask` with `card` inserted.
pub fn add(mask: CardMask, card: Card) -> CardMask {
    mask | card_mask(card)
}

/// Returns true when `card` is present in `mask`.
pub fn contains(mask: CardMask, card: Card) -> bool {
    (mask & card_mask(card)) != 0
}
