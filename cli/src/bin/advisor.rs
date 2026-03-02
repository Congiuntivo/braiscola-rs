use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use briscola_ai::rng::FastRng;
use briscola_core::card::{Card, Rank, Suit};
use briscola_core::rules::{TrickWinner, trick_winner};
use briscola_core::state::Player;
use cli::advisor::{
    InteractiveInit, InteractiveSession, format_card, parse_card, parse_card_list, parse_player,
    parse_suit, suggest_from_json_path,
};
use image::{ImageReader, imageops::FilterType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Interactive,
    Suggest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CliConfig {
    mode: Mode,
    json_path: Option<PathBuf>,
    samples_per_move: usize,
    seed: u64,
}

struct AsciiCardRenderer {
    cache: HashMap<Card, Vec<String>>,
    max_width: usize,
}

impl AsciiCardRenderer {
    fn new(max_width: usize) -> Self {
        Self { cache: HashMap::new(), max_width }
    }

    fn max_width(&self) -> usize {
        self.max_width
    }

    fn render_card(&mut self, card: Card) -> Result<Vec<String>, String> {
        if let Some(lines) = self.cache.get(&card) {
            return Ok(lines.clone());
        }

        let lines = self.render_card_uncached(card)?;
        self.cache.insert(card, lines.clone());
        Ok(lines)
    }

    fn render_card_uncached(&self, card: Card) -> Result<Vec<String>, String> {
        let image_path = card_asset_path(card);
        let image = ImageReader::open(&image_path)
            .map_err(|error| format!("cannot open card image '{}': {error}", image_path.display()))?
            .decode()
            .map_err(|error| {
                format!("cannot decode card image '{}': {error}", image_path.display())
            })?
            .to_rgba8();

        let Some((min_x, min_y, max_x, max_y)) = non_background_bounds(&image) else {
            return Err(format!("card image '{}' has no visible pixels", image_path.display()));
        };

        let crop_w = max_x - min_x + 1;
        let crop_h = max_y - min_y + 1;
        let cropped = image::imageops::crop_imm(&image, min_x, min_y, crop_w, crop_h).to_image();

        let target_w_u32 =
            u32::try_from(self.max_width).map_err(|_| String::from("invalid target width"))?;
        let mut target_h =
            crop_h.saturating_mul(target_w_u32).saturating_mul(55) / crop_w.saturating_mul(100);
        target_h = target_h.clamp(10, 24);

        let resized =
            image::imageops::resize(&cropped, target_w_u32, target_h, FilterType::Triangle);
        let shades: &[u8] = b"@%#*+=-:.";
        let shades_last = u32::try_from(shades.len().saturating_sub(1))
            .map_err(|_| String::from("invalid shade table"))?;

        let mut lines = Vec::with_capacity(
            usize::try_from(target_h).map_err(|_| String::from("invalid size"))?,
        );
        for y in 0..target_h {
            let mut line = String::with_capacity(self.max_width);
            for x in 0..target_w_u32 {
                let pixel = resized.get_pixel(x, y);
                if is_background(pixel.0) {
                    line.push(' ');
                    continue;
                }

                let r = u32::from(pixel.0[0]);
                let g = u32::from(pixel.0[1]);
                let b = u32::from(pixel.0[2]);
                let luminance = (2126_u32.saturating_mul(r)
                    + 7152_u32.saturating_mul(g)
                    + 722_u32.saturating_mul(b))
                    / 10_000;

                if luminance >= 245 {
                    line.push(' ');
                    continue;
                }

                let inverted = 255_u32.saturating_sub(luminance);
                let shade_index_u32 = inverted.saturating_mul(shades_last) / 255;
                let shade_index = usize::try_from(shade_index_u32)
                    .map_err(|_| String::from("invalid shade index"))?;
                line.push(char::from(shades[shade_index]));
            }
            lines.push(line);
        }

        Ok(lines)
    }
}

fn parse_cli() -> Result<CliConfig, String> {
    let mut args = std::env::args().skip(1);
    let mode = match args.next().as_deref() {
        Some("interactive") => Mode::Interactive,
        Some("suggest") => Mode::Suggest,
        _ => {
            return Err(String::from(
                "usage:\n  advisor interactive [--samples N] [--seed N]\n  advisor suggest --json PATH [--samples N] [--seed N]",
            ));
        }
    };

    let mut json_path = None;
    let mut samples_per_move = 128_usize;
    let mut seed = 42_u64;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--json" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --json"));
                };
                json_path = Some(PathBuf::from(value));
            }
            "--samples" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --samples"));
                };
                samples_per_move = value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --samples value '{value}': {error}"))?;
            }
            "--seed" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --seed"));
                };
                seed = value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid --seed value '{value}': {error}"))?;
            }
            "-h" | "--help" => {
                return Err(String::from(
                    "usage:\n  advisor interactive [--samples N] [--seed N]\n  advisor suggest --json PATH [--samples N] [--seed N]",
                ));
            }
            _ => {
                return Err(format!("unexpected argument '{arg}'"));
            }
        }
    }

    if mode == Mode::Suggest && json_path.is_none() {
        return Err(String::from("suggest mode requires --json PATH"));
    }

    Ok(CliConfig { mode, json_path, samples_per_move, seed })
}

fn prompt_line(prompt: &str) -> Result<String, String> {
    print!("{prompt}");
    io::stdout().flush().map_err(|error| format!("stdout flush failed: {error}"))?;

    let mut line = String::new();
    io::stdin().read_line(&mut line).map_err(|error| format!("stdin read failed: {error}"))?;
    Ok(line.trim().to_string())
}

fn prompt_card(prompt: &str) -> Result<Card, String> {
    loop {
        let line = prompt_line(prompt)?;
        match parse_card(&line) {
            Ok(card) => return Ok(card),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_hand(prompt: &str) -> Result<Vec<Card>, String> {
    loop {
        let line = prompt_line(prompt)?;
        let pieces: Vec<String> = line
            .split(',')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .collect();

        match parse_card_list(&pieces) {
            Ok(cards) if !cards.is_empty() && cards.len() <= 3 => return Ok(cards),
            Ok(_) => println!("hand must have 1 to 3 cards"),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_player(prompt: &str) -> Result<Player, String> {
    loop {
        let line = prompt_line(prompt)?;
        match parse_player(&line) {
            Ok(player) => return Ok(player),
            Err(error) => println!("{error}"),
        }
    }
}

fn prompt_u8(prompt: &str) -> Result<u8, String> {
    loop {
        let line = prompt_line(prompt)?;
        match line.parse::<u8>() {
            Ok(value) => return Ok(value),
            Err(error) => println!("invalid number: {error}"),
        }
    }
}

fn prompt_usize(prompt: &str) -> Result<usize, String> {
    loop {
        let line = prompt_line(prompt)?;
        match line.parse::<usize>() {
            Ok(value) => return Ok(value),
            Err(error) => println!("invalid number: {error}"),
        }
    }
}

fn print_card_syntax_legend() {
    println!("Card syntax:");
    println!("  compact: <suit><rank>  (example: oA, s3, cK)");
    println!("  explicit: <suit>:<rank> (example: clubs:K, coins:A)");
    println!("Suits:");
    println!("  o = coins, u = cups, s = swords, c = clubs");
    println!("Ranks:");
    println!("  A = Ace, 2..7 = numeric cards, J = Jack/Fante, Q = Queen/Cavallo, K = King/Re");
}

fn print_cards_ascii(
    title: &str,
    cards: &[Card],
    renderer: &mut AsciiCardRenderer,
) -> Result<(), String> {
    if cards.is_empty() {
        return Ok(());
    }

    println!("{title}");
    let mut rendered_cards = Vec::with_capacity(cards.len());
    for card in cards {
        rendered_cards.push(renderer.render_card(*card)?);
    }

    let Some(line_count) = rendered_cards.first().map(Vec::len) else {
        return Ok(());
    };

    for line_index in 0..line_count {
        for (card_index, rendered) in rendered_cards.iter().enumerate() {
            if card_index > 0 {
                print!("  ");
            }
            print!("{}", rendered[line_index]);
        }
        println!();
    }

    for (index, card) in cards.iter().enumerate() {
        if index > 0 {
            print!("  ");
        }
        print!("{:width$}", format_card(*card), width = renderer.max_width());
    }
    println!();
    Ok(())
}

fn card_asset_path(card: Card) -> PathBuf {
    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../res/napoletane");
    base.join(card_asset_name(card))
}

fn card_asset_name(card: Card) -> String {
    let suit_name = match card.suit {
        Suit::Coins => "denara",
        Suit::Cups => "coppe",
        Suit::Swords => "spade",
        Suit::Clubs => "bastoni",
    };
    let rank_number = match card.rank {
        Rank::Ace => 1_u8,
        Rank::Two => 2_u8,
        Rank::Three => 3_u8,
        Rank::Four => 4_u8,
        Rank::Five => 5_u8,
        Rank::Six => 6_u8,
        Rank::Seven => 7_u8,
        Rank::Jack => 8_u8,
        Rank::Queen => 9_u8,
        Rank::King => 10_u8,
    };
    format!("{suit_name}{rank_number}.webp")
}

fn non_background_bounds(image: &image::RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let width = image.width();
    let height = image.height();
    if width == 0 || height == 0 {
        return None;
    }

    let mut found = false;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;

    for y in 0..height {
        for x in 0..width {
            let pixel = image.get_pixel(x, y);
            if is_background(pixel.0) {
                continue;
            }
            found = true;
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
        }
    }

    if found { Some((min_x, min_y, max_x, max_y)) } else { None }
}

fn is_background(pixel: [u8; 4]) -> bool {
    let alpha = pixel[3];
    let red = pixel[0];
    let green = pixel[1];
    let blue = pixel[2];
    alpha < 16 || (red > 245 && green > 245 && blue > 245)
}

fn run_interactive(samples_per_move: usize, seed: u64) -> Result<(), String> {
    println!("Briscola move advisor (interactive)");
    print_card_syntax_legend();
    let mut renderer = AsciiCardRenderer::new(20);

    let briscola_suit = loop {
        let suit_raw = prompt_line("briscola suit (coins/cups/swords/clubs): ")?;
        match parse_suit(&suit_raw) {
            Ok(suit) => break suit,
            Err(error) => println!("{error}"),
        }
    };
    let face_up_trump = prompt_card("face-up trump card: ")?;
    let my_initial_hand = prompt_hand("your hand (comma-separated, e.g. oA,s3,cK): ")?;
    let leader = prompt_player("first leader (me/opponent): ")?;
    let talon_len = prompt_usize("talon length (33 at start): ")?;
    let score_me = prompt_u8("your score: ")?;
    let score_opp = prompt_u8("opponent score: ")?;

    let init =
        InteractiveInit { briscola_suit, face_up_trump, leader, talon_len, score_me, score_opp };

    let mut session = InteractiveSession::new(init, my_initial_hand)
        .map_err(|error| format!("cannot create session: {error:?}"))?;
    let mut rng = FastRng::new(seed);

    while !session.game_over() {
        println!();
        println!(
            "turn {} | score {}-{} | talon {} | leader {:?}",
            session.trick_number(),
            session.score_me(),
            session.score_opp(),
            session.talon_len(),
            session.leader()
        );
        print_cards_ascii("your hand:", session.my_hand(), &mut renderer)?;

        let opp_played = if session.leader() == Player::Opponent {
            Some(prompt_card("opponent played: ")?)
        } else {
            None
        };
        if let Some(card) = opp_played {
            print_cards_ascii("opponent played:", &[card], &mut renderer)?;
        }

        let suggestion = session
            .suggest_move(opp_played, &mut rng, samples_per_move)
            .map_err(|error| format!("cannot compute suggestion: {error:?}"))?;

        println!("suggested moves:");
        for stats in &suggestion.moves {
            println!(
                "  {} -> p_win={:.3}, ev_delta={:.2}, samples={}",
                format_card(stats.card),
                stats.p_win,
                stats.expected_score_delta,
                stats.n_samples
            );
        }
        let suggested_cards: Vec<Card> = suggestion.moves.iter().map(|stats| stats.card).collect();
        print_cards_ascii("suggested card art:", &suggested_cards, &mut renderer)?;

        let default_card = format_card(suggestion.best_move);
        let my_played = loop {
            let line = prompt_line(&format!("your played card [{default_card}]: "))?;
            let chosen = if line.is_empty() {
                suggestion.best_move
            } else {
                match parse_card(&line) {
                    Ok(card) => card,
                    Err(error) => {
                        println!("{error}");
                        continue;
                    }
                }
            };

            if session.my_hand().contains(&chosen) {
                break chosen;
            }
            println!("card {} is not in your hand", format_card(chosen));
        };

        let opp_reply_if_me_lead = if session.leader() == Player::Me {
            Some(prompt_card("opponent reply card: ")?)
        } else {
            None
        };

        let (lead, reply) = if session.leader() == Player::Me {
            let Some(opp_reply) = opp_reply_if_me_lead else {
                return Err(String::from("missing opponent reply"));
            };
            (my_played, opp_reply)
        } else {
            let Some(opp_lead) = opp_played else {
                return Err(String::from("missing opponent lead"));
            };
            (opp_lead, my_played)
        };

        let winner = if trick_winner(lead, reply, briscola_suit) == TrickWinner::Leader {
            session.leader()
        } else {
            session.leader().other()
        };

        let needs_draw =
            session.talon_len() > 1 || (session.talon_len() == 1 && winner == Player::Me);
        let my_draw_card =
            if needs_draw { Some(prompt_card("your drawn card from talon: ")?) } else { None };

        let result = session
            .apply_turn(opp_played, my_played, opp_reply_if_me_lead, my_draw_card)
            .map_err(|error| format!("cannot apply turn: {error:?}"))?;

        println!(
            "winner {:?}, trick points {}, score {}-{}",
            result.winner, result.points, result.score_me, result.score_opp
        );
    }

    println!();
    println!("game over | final score {}-{}", session.score_me(), session.score_opp());
    Ok(())
}

fn run_suggest(config: &CliConfig) -> Result<(), String> {
    let Some(path) = &config.json_path else {
        return Err(String::from("missing json path"));
    };

    print_card_syntax_legend();
    let mut renderer = AsciiCardRenderer::new(20);
    let result = suggest_from_json_path(path, Some(config.samples_per_move), Some(config.seed))?;
    println!("best move: {}", format_card(result.best_move));
    print_cards_ascii("best move card art:", &[result.best_move], &mut renderer)?;
    for stats in result.moves {
        println!(
            "  {} -> p_win={:.3}, ev_delta={:.2}, samples={}",
            format_card(stats.card),
            stats.p_win,
            stats.expected_score_delta,
            stats.n_samples
        );
    }
    Ok(())
}

fn main() {
    let config = match parse_cli() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    let outcome = match config.mode {
        Mode::Interactive => run_interactive(config.samples_per_move, config.seed),
        Mode::Suggest => run_suggest(&config),
    };

    if let Err(error) = outcome {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
