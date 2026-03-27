//! Terminal UI entrypoint for human-vs-AI play.
//!
//! Provides keyboard-driven card selection, optional best-move hints,
//! and visualized table/hand state.

use std::collections::VecDeque;
use std::io;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use briscola_ai::mc::BestMoveResult;
use briscola_core::card::{Card, HAND_SIZE};
use briscola_core::state::Player;
use cli::advisor::format_card;
use cli::card_art::{TerminalCardRenderer, card_name_english, card_name_italian};
use cli::play::{PlayConfig, PlayError, PlayableGame, winner_from_scores};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Wrap};
use ratatui::{Frame, Terminal};

#[derive(Debug, Clone, Copy)]
struct CliOptions {
    seed: u64,
    hint_samples: usize,
    opponent_samples: usize,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self { seed: random_seed(), hint_samples: 128, opponent_samples: 96 }
    }
}

fn random_seed() -> u64 {
    let base = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let nanos = duration.as_nanos();
            let low = u64::try_from(nanos & u128::from(u64::MAX)).unwrap_or(0);
            let high = u64::try_from(nanos >> 64).unwrap_or(0);
            low ^ high
        }
        Err(_) => 0x9E37_79B9_7F4A_7C15,
    };
    base ^ u64::from(std::process::id())
}

struct UiState {
    game: PlayableGame,
    seed: u64,
    selected_index: usize,
    hint_enabled: bool,
    cached_hint: Option<BestMoveResult>,
    status: String,
    log: VecDeque<String>,
    renderer: TerminalCardRenderer,
    table_renderer: TerminalCardRenderer,
    art_error: Option<String>,
    last_trick: Option<CompletedTrickView>,
    winner_flash_on: bool,
}

#[derive(Debug, Clone, Copy)]
struct CompletedTrickView {
    my_card: Card,
    opp_card: Card,
    winner: Player,
}

#[derive(Debug, Clone, Copy)]
struct TableSlotView<'a> {
    title: &'a str,
    card: Option<Card>,
    is_winner_highlighted: bool,
}

const HAND_SLOTS: usize = HAND_SIZE;

impl UiState {
    fn new(config: CliOptions) -> Result<Self, String> {
        let game = PlayableGame::new(PlayConfig {
            seed: config.seed,
            hint_samples: config.hint_samples,
            opponent_samples: config.opponent_samples,
        })
        .map_err(|error| format!("cannot initialize game: {error:?}"))?;

        let mut log = VecDeque::with_capacity(16);
        log.push_back(String::from("Welcome. You are Me."));

        Ok(Self {
            game,
            seed: config.seed,
            selected_index: 0,
            hint_enabled: false,
            cached_hint: None,
            status: String::from("Select a card and press Enter."),
            log,
            renderer: TerminalCardRenderer::new(14),
            table_renderer: TerminalCardRenderer::new(10),
            art_error: None,
            last_trick: None,
            winner_flash_on: false,
        })
    }

    fn push_log(&mut self, line: String) {
        while self.log.len() >= 14 {
            let _ = self.log.pop_front();
        }
        self.log.push_back(line);
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

impl TerminalGuard {
    fn new() -> Result<Self, String> {
        enable_raw_mode().map_err(|error| format!("raw mode enable failed: {error}"))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)
            .map_err(|error| format!("terminal init failed: {error}"))?;
        let backend = CrosstermBackend::new(stdout);
        let terminal =
            Terminal::new(backend).map_err(|error| format!("terminal error: {error}"))?;
        Ok(Self { terminal })
    }

    fn terminal_mut(&mut self) -> &mut Terminal<CrosstermBackend<io::Stdout>> {
        &mut self.terminal
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
    }
}

fn parse_cli_options() -> Result<CliOptions, String> {
    let mut options = CliOptions::default();
    let mut args = std::env::args().skip(1);
    let mut parsed_positional_seed = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seed" | "-s" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --seed"));
                };
                options.seed = value
                    .parse::<u64>()
                    .map_err(|error| format!("invalid --seed value '{value}': {error}"))?;
            }
            "--hint-samples" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --hint-samples"));
                };
                options.hint_samples = value
                    .parse::<usize>()
                    .map_err(|error| format!("invalid --hint-samples value '{value}': {error}"))?;
            }
            "--opponent-samples" => {
                let Some(value) = args.next() else {
                    return Err(String::from("missing value after --opponent-samples"));
                };
                options.opponent_samples = value.parse::<usize>().map_err(|error| {
                    format!("invalid --opponent-samples value '{value}': {error}")
                })?;
            }
            "-h" | "--help" => {
                return Err(String::from(
                    "usage: play_tui [seed] [--seed N|-s N] [--hint-samples N] [--opponent-samples N]\nseed defaults to a random value when omitted",
                ));
            }
            _ if !arg.starts_with('-') && !parsed_positional_seed => {
                options.seed = arg
                    .parse::<u64>()
                    .map_err(|error| format!("invalid positional seed '{arg}': {error}"))?;
                parsed_positional_seed = true;
            }
            _ => return Err(format!("unexpected argument '{arg}'")),
        }
    }

    Ok(options)
}

fn run_game(options: CliOptions) -> Result<(), String> {
    let mut ui = UiState::new(options)?;
    let mut guard = TerminalGuard::new()?;

    loop {
        if let Some(opp_lead) = ui
            .game
            .maybe_play_opponent_lead()
            .map_err(|error| format!("opponent turn failed: {error:?}"))?
        {
            ui.push_log(format!("Opponent leads {}", format_card(opp_lead)));
            ui.cached_hint = None;
            ui.selected_index = 0;
            ui.status = String::from("Your response turn.");
        }

        let hand_len = ui.game.my_hand().len();
        if hand_len > 0 && ui.selected_index >= hand_len {
            ui.selected_index = hand_len - 1;
        }

        if ui.hint_enabled && ui.game.is_player_turn() && ui.cached_hint.is_none() {
            match ui.game.hint_best_move() {
                Ok(hint) => {
                    ui.cached_hint = Some(hint);
                }
                Err(error) => {
                    ui.status = format!("Hint failed: {error:?}");
                }
            }
        }

        guard
            .terminal_mut()
            .draw(|frame| render(frame, &mut ui))
            .map_err(|error| format!("draw failed: {error}"))?;

        if !event::poll(Duration::from_millis(120))
            .map_err(|error| format!("poll failed: {error}"))?
        {
            continue;
        }

        let event = event::read().map_err(|error| format!("input read failed: {error}"))?;
        let Event::Key(key_event) = event else {
            continue;
        };
        if key_event.kind != KeyEventKind::Press {
            continue;
        }

        if ui.game.is_game_over() {
            match key_event.code {
                KeyCode::Char('q') | KeyCode::Enter | KeyCode::Esc => break,
                _ => continue,
            }
        }

        match key_event.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Char('h') => {
                ui.hint_enabled = !ui.hint_enabled;
                ui.cached_hint = None;
                ui.status = if ui.hint_enabled {
                    String::from("Hint enabled")
                } else {
                    String::from("Hint disabled")
                };
            }
            KeyCode::Left | KeyCode::Up => {
                ui.selected_index = ui.selected_index.saturating_sub(1);
            }
            KeyCode::Right | KeyCode::Down => {
                if ui.selected_index + 1 < ui.game.my_hand().len() {
                    ui.selected_index += 1;
                }
            }
            KeyCode::Char(digit) if digit.is_ascii_digit() => {
                let Some(value) = digit.to_digit(10) else {
                    continue;
                };
                if value == 0 {
                    continue;
                }
                let Ok(target) = usize::try_from(value - 1) else {
                    continue;
                };
                if target < ui.game.my_hand().len() {
                    ui.selected_index = target;
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                if !ui.game.is_player_turn() {
                    ui.status = String::from("Wait for opponent action");
                    continue;
                }

                let Some(chosen_card) = ui.game.my_hand().get(ui.selected_index).copied() else {
                    ui.status = String::from("No selectable card");
                    continue;
                };

                match ui.game.play_player_card(chosen_card) {
                    Ok(outcome) => {
                        ui.last_trick = Some(CompletedTrickView {
                            my_card: chosen_card,
                            opp_card: outcome.opponent_card,
                            winner: outcome.winner,
                        });
                        ui.status = format!(
                            "Played {} | Opp {} | Winner {:?} | +{}",
                            format_card(chosen_card),
                            format_card(outcome.opponent_card),
                            outcome.winner,
                            outcome.trick_points
                        );
                        ui.push_log(ui.status.clone());
                        ui.cached_hint = None;
                        animate_turn_winner_flash(&mut guard, &mut ui)?;
                        ui.last_trick = None;
                    }
                    Err(PlayError::InvalidMove) => {
                        ui.status = String::from("Invalid card selection");
                    }
                    Err(error) => {
                        ui.status = format!("Turn failed: {error:?}");
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn render(frame: &mut Frame<'_>, ui: &mut UiState) {
    let root = frame.area();

    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(6),
            Constraint::Min(12),
            Constraint::Length(8),
            Constraint::Length(2),
        ])
        .split(root);

    let title = Paragraph::new(Text::from(vec![Line::from(vec![
        Span::styled(" brAIscola ", Style::default().fg(Color::Black).bg(Color::Yellow)),
        Span::raw("  play as Me against AI"),
    ])]))
    .block(Block::default().borders(Borders::ALL).title("Game"));
    frame.render_widget(title, vertical[0]);

    let info_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(30), Constraint::Length(30), Constraint::Min(20)])
        .split(vertical[1]);

    let score_block = Paragraph::new(Text::from(vec![
        Line::from(format!("Me: {}", ui.game.score_me())),
        Line::from(format!("Opponent: {}", ui.game.score_opp())),
        Line::from(format!("Trick #: {}", ui.game.completed_tricks() + 1)),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Score"));
    frame.render_widget(score_block, info_layout[0]);

    let trump = ui.game.briscola_card();
    let meta_block = Paragraph::new(Text::from(vec![
        Line::from(format!("Talon: {}", ui.game.talon_len())),
        Line::from(format!("Opp cards: {}", ui.game.opponent_cards_remaining())),
        Line::from(format!("Leader: {}", player_label(ui.game.leader()))),
        Line::from(format!("Seed: {}", ui.seed)),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Table"));
    frame.render_widget(meta_block, info_layout[1]);

    let opponent_played =
        ui.game.current_opponent_lead().map_or_else(|| String::from("-"), format_card);
    let trump_block = Paragraph::new(Text::from(vec![
        Line::from(format!("Trump: {} ({:?})", format_card(trump), ui.game.briscola_suit())),
        Line::from(format!("Trump points: {}", trump.rank.points())),
        Line::from(format!("Opponent played: {opponent_played}")),
    ]))
    .block(Block::default().borders(Borders::ALL).title("Visible"));
    frame.render_widget(trump_block, info_layout[2]);

    let play_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(vertical[2]);

    render_hand_cards(frame, play_area[0], ui);
    let side_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
        .split(play_area[1]);
    render_table_cards(frame, side_area[0], ui);
    render_hint_table(frame, side_area[1], ui);

    let log_lines: Vec<ListItem<'_>> =
        ui.log.iter().rev().take(6).map(|line| ListItem::new(line.as_str())).collect();
    let log_widget =
        List::new(log_lines).block(Block::default().borders(Borders::ALL).title("Activity"));
    frame.render_widget(log_widget, vertical[3]);

    let footer = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(ui.status.clone(), Style::default().fg(Color::Green))),
        Line::from("Controls: q quit | h toggle hint | left/right or 1..3 select | Enter play"),
    ]))
    .alignment(Alignment::Left)
    .block(Block::default().borders(Borders::ALL).title("Status"));
    frame.render_widget(footer, vertical[4]);

    if ui.game.is_game_over() {
        let popup_area = centered_rect(60, 25, root);
        frame.render_widget(Clear, popup_area);
        let winner_text = match winner_from_scores(ui.game.score_me(), ui.game.score_opp()) {
            Some(Player::Me) => String::from("You win"),
            Some(Player::Opponent) => String::from("Opponent wins"),
            None => String::from("Draw"),
        };

        let popup = Paragraph::new(Text::from(vec![
            Line::from(Span::styled(
                "Game Over",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )),
            Line::from(format!("Final score {}-{}", ui.game.score_me(), ui.game.score_opp())),
            Line::from(winner_text),
            Line::from("Press Enter, q or Esc to exit"),
        ]))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL).title("Result"));
        frame.render_widget(popup, popup_area);
    }
}

fn render_hint_table(frame: &mut Frame<'_>, area: Rect, ui: &UiState) {
    let hint_rows: Vec<Row<'_>> = if ui.hint_enabled {
        if let Some(hint) = ui.cached_hint.as_ref() {
            hint.moves
                .iter()
                .map(|stats| {
                    Row::new(vec![
                        Cell::from(format_card(stats.card)),
                        Cell::from(format!("{:.3}", stats.p_win)),
                        Cell::from(format!("{:.2}", stats.expected_score_delta)),
                    ])
                })
                .collect()
        } else {
            vec![Row::new(vec![Cell::from("calculating"), Cell::from("-"), Cell::from("-")])]
        }
    } else {
        vec![Row::new(vec![Cell::from("disabled"), Cell::from("-"), Cell::from("-")])]
    };

    let hint_table = Table::new(
        hint_rows,
        [Constraint::Length(8), Constraint::Length(8), Constraint::Length(10)],
    )
    .header(
        Row::new(vec!["Card", "p_win", "ev"])
            .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
    )
    .block(Block::default().borders(Borders::ALL).title(if ui.hint_enabled {
        "Hint (ON)"
    } else {
        "Hint (OFF)"
    }))
    .column_spacing(1);
    frame.render_widget(hint_table, area);
}

fn render_table_cards(frame: &mut Frame<'_>, area: Rect, ui: &mut UiState) {
    let outer = Block::default().borders(Borders::ALL).title("Table Cards");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    if inner.width < 6 || inner.height < 5 {
        return;
    }

    let (opp_card, my_card, winner) = if let Some(last_trick) = ui.last_trick {
        (Some(last_trick.opp_card), Some(last_trick.my_card), Some(last_trick.winner))
    } else {
        (ui.game.current_opponent_lead(), None, None)
    };

    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);

    let opp_highlight = ui.winner_flash_on && winner == Some(Player::Opponent);
    let my_highlight = ui.winner_flash_on && winner == Some(Player::Me);

    render_table_slot(
        frame,
        columns[0],
        TableSlotView { title: "Opponent", card: opp_card, is_winner_highlighted: opp_highlight },
        ui,
    );
    render_table_slot(
        frame,
        columns[1],
        TableSlotView { title: "Me", card: my_card, is_winner_highlighted: my_highlight },
        ui,
    );
}

fn render_table_slot(frame: &mut Frame<'_>, area: Rect, slot: TableSlotView<'_>, ui: &mut UiState) {
    let border_style = if slot.is_winner_highlighted {
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let mut lines = Vec::new();
    if let Some(card) = slot.card {
        lines.push(Line::from(Span::styled(
            format_card(card),
            Style::default().fg(Color::LightCyan),
        )));
        lines.push(Line::from(format!("Pts {}", card.rank.points())));
        lines.push(Line::default());

        match ui.table_renderer.render_card(card) {
            Ok(card_lines) => lines.extend(card_lines),
            Err(error) => {
                if ui.art_error.is_none() {
                    ui.art_error = Some(error.clone());
                    ui.status = format!("Card rendering disabled: {error}");
                }
            }
        }
    } else {
        lines.push(Line::from("-"));
        lines.push(Line::from("waiting"));
    }

    let panel = Paragraph::new(Text::from(lines))
        .block(Block::default().borders(Borders::ALL).title(slot.title).border_style(border_style))
        .wrap(Wrap { trim: false });
    frame.render_widget(panel, area);
}

fn animate_turn_winner_flash(guard: &mut TerminalGuard, ui: &mut UiState) -> Result<(), String> {
    const FLASHES: usize = 3;
    const INTERVAL_MS: u64 = 300;

    for step in 0..(FLASHES * 2) {
        ui.winner_flash_on = step % 2 == 0;
        guard
            .terminal_mut()
            .draw(|frame| render(frame, ui))
            .map_err(|error| format!("draw failed: {error}"))?;
        thread::sleep(Duration::from_millis(INTERVAL_MS));
    }

    ui.winner_flash_on = false;
    clear_pending_input_events()?;
    Ok(())
}

fn clear_pending_input_events() -> Result<(), String> {
    while event::poll(Duration::from_millis(0)).map_err(|error| format!("poll failed: {error}"))? {
        let _ = event::read().map_err(|error| format!("input read failed: {error}"))?;
    }
    Ok(())
}

fn render_hand_cards(frame: &mut Frame<'_>, area: Rect, ui: &mut UiState) {
    let hand = ui.game.my_hand().to_vec();
    let card_constraints = vec![Constraint::Ratio(1, 3); HAND_SLOTS];
    let card_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(card_constraints)
        .split(area);

    for (index, slot_area) in card_areas.iter().enumerate().take(HAND_SLOTS) {
        let Some(card) = hand.get(index).copied() else {
            let empty_card = Paragraph::new(Text::from(vec![
                Line::from(format!("[{}]", index + 1)),
                Line::default(),
                Line::from("Empty"),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Your Hand")
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .wrap(Wrap { trim: false });
            frame.render_widget(empty_card, *slot_area);
            continue;
        };

        let is_selected = index == ui.selected_index;
        let is_briscola = card.suit == ui.game.briscola_suit();
        let name_style = if is_briscola {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Cyan)
        };
        let italian_name_style = if is_briscola {
            Style::default().fg(Color::LightYellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };

        let mut lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", index + 1),
                    Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD),
                ),
                Span::styled(card_name_english(card), name_style),
            ]),
            Line::from(Span::styled(card_name_italian(card), italian_name_style)),
            Line::from(format!("Points: {}", card.rank.points())),
            Line::default(),
        ];

        match ui.renderer.render_card(card) {
            Ok(card_art_lines) => {
                lines.extend(card_art_lines);
            }
            Err(error) => {
                if ui.art_error.is_none() {
                    ui.art_error = Some(error.clone());
                    ui.status = format!("Card rendering disabled: {error}");
                }
                lines.push(Line::from(Span::styled(
                    format_card(card),
                    Style::default().fg(Color::LightBlue),
                )));
            }
        }

        let border_style = if is_selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let card_block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .title(if is_selected { "Selected" } else { "Your Hand" });

        let card_widget =
            Paragraph::new(Text::from(lines)).block(card_block).wrap(Wrap { trim: false });
        frame.render_widget(card_widget, *slot_area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100_u16.saturating_sub(percent_y)) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100_u16.saturating_sub(percent_y)) / 2),
        ])
        .split(area);

    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100_u16.saturating_sub(percent_x)) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100_u16.saturating_sub(percent_x)) / 2),
        ])
        .split(vertical[1]);

    horizontal[1]
}

fn player_label(player: Player) -> &'static str {
    match player {
        Player::Me => "Me",
        Player::Opponent => "Opponent",
    }
}

fn main() {
    let options = match parse_cli_options() {
        Ok(options) => options,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };

    if let Err(error) = run_game(options) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
