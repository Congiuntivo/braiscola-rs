# Napoletane Card Assets

This folder contains card images used by advisor and TUI renderers.

## Naming Convention

Files use `<suit><rank>.webp` naming:

- Suits:
  - `denara` (coins)
  - `coppe` (cups)
  - `spade` (swords)
  - `bastoni` (clubs)
- Ranks:
  - `1` = Ace
  - `2..7` = numeric ranks
  - `8` = Jack (Fante)
  - `9` = Queen (Cavallo)
  - `10` = King (Re)

Examples:

- `denara1.webp` is Ace of Coins.
- `spade10.webp` is King of Swords.
- `bastoni8.webp` is Jack of Clubs.

## Usage in Code

- `cli/src/card_art.rs` maps game cards to these filenames.
- White or near-white pixels are treated as background in terminal renderers.
- Assets are rendered in two modes:
  - ASCII grayscale output for standard terminal printing.
  - Colored block rendering for the TUI.

## Notes

- Keep filenames stable to avoid breaking card lookup.
- Prefer maintaining transparent/white background style for consistent renderer behavior.
