# Advisor Example Scenarios

This folder contains ready-to-run JSON snapshots for `advisor suggest`.

## Files

- `start_turn.json`: opening-like state where Me leads and talon is still deep.
- `lead_turn_n.json`: midgame state where Me leads (`leader = me`).
- `respond_turn_n.json`: turn where Opponent already led (`opp_played` is set).
- `endgame_turn_n.json`: low-information endgame snapshot with no talon draws.

## JSON Schema

Required fields:

- `briscola_suit`: trump suit.
  - Accepted values include English (`coins`, `cups`, `swords`, `clubs`),
    short aliases (`o`, `u`, `s`, `c`, plus `d` for denari and `b` for bastoni),
    and suit emoji tokens.
- `face_up_trump`: exposed trump card token.
- `my_hand`: current hand for Me (1 to 3 cards).
- `talon_len`: cards left in talon (excluding face-up trump).
- `score_me`: current score for Me.
- `score_opp`: current score for Opponent.
- `leader`: `me` or `opponent`.

Optional fields:

- `opp_played`: opponent lead in current trick when Me is replying.
- `history`: completed tricks list with `{ "lead": "...", "reply": "..." }`.
- `seen_cards`: extra known cards to include as visible information.
- `samples_per_move`: per-move Monte Carlo sample count (default 128).
- `seed`: RNG seed for reproducibility (default 42).

## Card Tokens

Accepted card notation:

- Compact: `<suit><rank>` (for example `oA`, `s3`, `cK`, `u7`, or emoji suit forms).
- Explicit: `<suit>:<rank>` (for example `coins:A`, `clubs:K`).

Rank tokens:

- `A,2,3,4,5,6,7,J,Q,K`
- Name aliases are also accepted (`ace`, `king`, `fante`, `re`, etc.).

## Run Examples

```bash
cargo run -p cli --bin advisor -- suggest --json examples/advisor/start_turn.json --samples 256 --seed 42
cargo run -p cli --bin advisor -- suggest --json examples/advisor/lead_turn_n.json --samples 192 --seed 42
cargo run -p cli --bin advisor -- suggest --json examples/advisor/respond_turn_n.json --samples 128 --seed 13
cargo run -p cli --bin advisor -- suggest --json examples/advisor/endgame_turn_n.json --samples 256 --seed 7
```

## Authoring New Scenarios

1. Start from one existing JSON file.
2. Keep `my_hand` consistent with visible cards and `history`.
3. Set `leader` and `opp_played` coherently:
   - if `leader = me`, omit `opp_played`;
   - if `leader = opponent` and Opponent has already acted, include `opp_played`.
4. Keep `talon_len` consistent with game phase.
5. Use a fixed `seed` while iterating so comparisons are reproducible.
