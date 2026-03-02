Below is a “game-as-algorithm” spec for **Briscola** (2-player, standard 40-card deck), written in a way you can turn into a Rust engine that can (1) validate moves, (2) simulate games, and (3) choose the move with best win odds under hidden information.

---

## 1) Model the cards

**Deck (40 cards)**
Suits: `Coins, Cups, Swords, Clubs` (names don’t matter; just 4 suits)
Ranks: `A, 2, 3, 4, 5, 6, 7, J, Q, K` (Italian: Asso, Due, Tre, Quattro, Cinque, Sei, Sette, Fante, Cavallo, Re)

**Point values** (for scoring tricks):

* `A = 11`
* `3 = 10`
* `K = 4`
* `Q = 3`
* `J = 2`
* others (`7,6,5,4,2`) = `0`

**Trick-taking strength order** (to decide who wins the trick):
`A > 3 > K > Q > J > 7 > 6 > 5 > 4 > 2`

Represent rank as an enum with:

* `points(rank) -> u8`
* `power(rank) -> u8` (higher wins inside same suit)

---

## 2) Briscola rules as state transitions

### Game state (information available to the engine)

At any decision point, you (the AI) know:

* `my_hand: Vec<Card>` (size 1..3)
* `opp_played: Option<Card>` (only if opponent led this trick)
* `briscola_suit: Suit`
* `talon_len: usize` (cards left to draw from deck)
* `last_face_up_trump: Card` (the card revealed at start; becomes last draw)
* `seen_cards: bitset[40]` (all cards that are in your hand, already played in tricks, opponent’s current played card if any, and the face-up trump card)
* `score_me, score_opp`
* `leader: Player` (who leads current trick; affects who plays first)
* optionally: `history` (previous tricks if you want advanced inference)

You do **not** know opponent’s hand nor the exact talon order.

### Setup algorithm

1. Create full 40-card deck.
2. Shuffle.
3. Deal 3 cards to each player (alternating or not; irrelevant if shuffled).
4. Reveal next card = `trump_card`; set `briscola_suit = trump_card.suit`.
5. Put `trump_card` face-up under the remaining deck: it will be drawn **last**.
6. First leader is usually “hand” player (varies by tradition, but pick one and keep consistent).

### Trick resolution algorithm (core)

A trick is two plays:

**Inputs**

* `lead_card` (played by leader)
* `reply_card` (played by follower)

**Winner determination**

```
if reply_card.suit == lead_card.suit:
    winner = higher_power(lead_card, reply_card)
else if reply_card.suit == briscola_suit and lead_card.suit != briscola_suit:
    winner = follower
else if lead_card.suit == briscola_suit and reply_card.suit != briscola_suit:
    winner = leader
else:
    winner = leader   // follower didn't follow suit and didn't trump
```

**Score update**
`trick_points = points(lead_card.rank) + points(reply_card.rank)`
Add to winner’s score.

**Draw phase (after trick, if talon_len > 0)**

* Winner draws first from the talon.
* Loser draws second.
* When only 1 card remains in talon, the second drawn is the face-up `trump_card`.

**Next leader = trick winner.**
Repeat until hands empty and talon empty ⇒ game over.
Winner is higher total points (max is 120).

### Legal moves

In Briscola (classic) there is **no obligation to follow suit**.
So legal moves are simply: “play any card in your hand”.

That’s great for AI: branching factor is ≤ 3 almost always.

---

## 3) What your solver is actually solving (hidden information)

Your decision is in an **information set**: many possible “true worlds” (opponent hand + talon order) are consistent with what you’ve seen.

So “best move” is:

* maximize `P(win)` (or maximize expected final score)
* under uncertainty of hidden cards

The usual robust approach is:

### A) Determinization + Monte Carlo / MCTS

1. From current public state, build the **unknown card pool**:
   `unknown = full_deck - seen_cards`
2. Sample a plausible completion:

   * choose `opp_hand_size` cards from `unknown` for opponent hand
   * remaining go into talon order (respecting that the face-up trump is fixed as last)
3. Now you have a perfect-information state; you can:

   * roll out the rest randomly (fast baseline)
   * or do deeper search (minimax/expectimax) on the determinized game
4. Repeat for many samples; choose the move with best average win rate.

This works well because:

* small hands
* simple rules
* only 40 cards

### B) Expectimax (exact) on late game

When `talon_len` is small (e.g., ≤ 6–8), you can consider switching to an exact/near-exact enumeration:

* enumerate all opponent hand combinations consistent with `unknown`
* enumerate remaining draw orders (or partially)
  This can become expensive quickly, but near the end it’s feasible.

A practical hybrid:

* Early/mid: MCTS/Monte Carlo determinization
* Late: exact enumeration / memoized search

---

## 4) Fast state representation (Rust-friendly)

### Card indexing

Map each of the 40 cards to a unique `u8` index `0..39`.

Then represent sets as bitsets:

* `u64` is enough for 40 cards.
* `seen_mask: u64`
* `my_hand_mask: u64`
* `played_mask: u64` etc.

This makes:

* “unknown cards” = `FULL_MASK ^ seen_mask`
* sampling = pick random bits
* caching = cheap hashing

### Minimal simulation state for a determinized world

Once you sample opponent hand + talon order, you can run very fast with:

* `my_hand: [u8;3] + len`
* `opp_hand: [u8;3] + len`
* `talon: Vec<u8> + cursor`
* `leader: bool`
* `pending_lead: Option<u8>`
* `scores: (u8,u8)` (0..120 fits in u8)
* `briscola_suit: u8`

### Transposition / memo

You can memoize late-game states:
Key should include:

* `my_hand_mask`
* `opp_hand_mask`
* `talon_mask` or `(talon_cursor, remaining_multiset_mask)`
* `leader`
* `pending_lead`
* `scores` (optional; sometimes you can store “score delta from here”)

For speed, use:

* `hashbrown::HashMap` or `fxhash` style hasher (or your own Zobrist hash)
* compact key struct with `#[repr(C)]`

---

## 5) Move evaluation algorithm (what your API returns)

At a decision point:

### If opponent already played (you are follower)

You choose among your 1..3 cards.
For each candidate `c`:

1. Evaluate `EV(c)` by Monte Carlo:

   * loop `N` samples:

     * determinize
     * force-play `c` as reply
     * finish game by:

       * either random policy
       * or “ε-greedy” policy (simple heuristics)
       * or MCTS from there
     * record win (1/0) or final score delta
2. Pick argmax.

### If you are leader

Same, but you force-play your lead first; opponent responds according to the sampled hidden world + policy/search.

---

## 6) Opponent policy inside simulations (important)

If you simulate opponent as purely random, you’ll get “odds of winning vs random”, not “best odds vs a competent player”.

Better options:

* **Heuristic policy** (very strong baseline in Briscola):

  * if can win trick cheaply, do it
  * avoid throwing high-point cards when likely to lose trick
  * preserve trump (briscola) for capturing point-rich tricks
  * when leading, often lead low non-trump to “probe”
* **Nested search**:

  * in each determinized world, have opponent also choose best move by small-depth search
  * even depth-2/3 makes simulations much more realistic

In practice:

* use a cheap heuristic opponent for rollouts
* use deeper search only at the root (your actual move choice)

---

## 7) Concrete heuristics (good enough to start)

Define helper:

* `wins_trick(lead, reply, briscola_suit) -> bool`
* `trick_points(lead, reply)`

Follower heuristic:

1. If you can win the trick:

   * prefer winning with **lowest power** card that still wins
   * especially if trick has many points (A/3/K/Q/J involved)
2. If you cannot win:

   * dump a **0-point** card if possible
   * else dump lowest point / lowest power non-trump
3. Avoid spending trump to win a low-point trick early unless you’re endgame.

Leader heuristic:

1. If you hold very strong trump and opponent likely has points out:

   * sometimes lead trump late game to pull theirs
2. Otherwise lead low non-trump.
3. Avoid leading A or 3 unless you expect it to take points (or endgame).

These heuristics make rollouts far less noisy.

---

## 8) Project structure suggestion (Rust)

* `briscola_core/`

  * `card.rs` (enums, indexing, points, power)
  * `rules.rs` (winner logic, scoring, draw logic)
  * `state.rs` (public state, determinized state, apply_move)
  * `bitset.rs` (u64 helpers)
* `briscola_ai/`

  * `determinize.rs` (sample opponent hand + talon)
  * `rollout.rs` (policies)
  * `mcts.rs` or `mc.rs` (root Monte Carlo evaluator)
  * `cache.rs` (transposition)
* `cli/` (play vs AI, debug)
* `api/` (HTTP JSON; returns best move + stats)
* `sim/`

  * self-play tournament
  * store `(public_state_hash -> recommended_move)` as an opening book / table

---

## 9) “Lookup very fast” data structure (what to store)

Be careful: the true optimal move depends on beliefs about hidden cards. But you *can* store something useful:

### Store per **public state**:

Key:

* `my_hand_mask`
* `played_history_mask` (or just `seen_mask`)
* `opp_played_card` (or none)
* `talon_len`
* `briscola_suit`
* `scores`, `leader`

Value:

* best move
* estimated win probability
* sample count / confidence

This becomes a **policy table** learned by simulation. It won’t be perfect (because different histories can lead to same `seen_mask` but different opponent-likelihoods), but it’s often strong.

If you want more accuracy, include small “features” of history:

* counts of trump seen
* counts of high cards seen (A/3) per suit
* last trick winner
  These keep the key compact but add inference power.

---

## 10) What your API can return

For each legal move:

* `p_win`
* `expected_score_delta`
* `n_samples`
* maybe `confidence_interval`

And the chosen `best_move`.

---
