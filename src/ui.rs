//! 화면 그리기.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout as Split, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::layout::COLS;
use crate::progress::WordStarts;
use crate::words::{Lang, char_width};

const BAR: usize = 8;
const ENTRY_WIDTH: usize = 22;

pub fn draw(f: &mut Frame, app: &App) {
    let [header, practice, keyboard, progress, footer] = Split::vertical([
        Constraint::Length(1),
        Constraint::Length(6),
        Constraint::Length(5),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(f.area());

    draw_header(f, app, header);
    draw_practice(f, app, practice);
    draw_keyboard(f, app, keyboard);
    draw_progress(f, app, progress);

    let mut help = vec![Span::styled(
        " Esc quit · Tab new line",
        Style::new().fg(Color::DarkGray),
    )];
    if let Some(err) = &app.save_error {
        help.push(Span::styled(format!("   {err}"), Style::new().fg(Color::Red)));
    }
    f.render_widget(Paragraph::new(Line::from(help)), footer);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let dim = Style::new().fg(Color::DarkGray);
    let mut spans = vec![
        Span::styled(
            " taja ",
            Style::new()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" {} · {}  ", app.layout.name, app.lang().name())),
        Span::styled("keys ", dim),
        Span::raw(format!("{}/{}  ", app.profile.unlocked, app.profile.order.len())),
        Span::styled("target ", dim),
        Span::raw(format!(
            "{} · {:.0}%  ",
            app.speed(app.targets.wpm),
            app.targets.accuracy * 100.0
        )),
    ];
    if let Some(wpm) = app.lesson.wpm(Instant::now()) {
        spans.push(Span::styled("now ", dim));
        spans.push(Span::raw(format!("{}  ", app.speed(wpm))));
    }
    if let Some((wpm, acc)) = app.last_line {
        spans.push(Span::styled("last line ", dim));
        spans.push(Span::raw(format!("{} · {:.0}%", app.speed(wpm), acc * 100.0)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_practice(f: &mut Frame, app: &App, area: Rect) {
    let lesson = &app.lesson;
    let pos = lesson.pos;
    let blocked = lesson.is_blocked();

    let target: Vec<Span> = app
        .target
        .units
        .iter()
        .map(|u| {
            let style = if u.end <= pos {
                let error = lesson.had_error[u.start..u.end].iter().any(|&e| e);
                Style::new().fg(if error { Color::Red } else { Color::Green })
            } else if u.start <= pos {
                let bg = if blocked { Color::Red } else { Color::White };
                let fg = if blocked { Color::White } else { Color::Black };
                Style::new().fg(fg).bg(bg).add_modifier(Modifier::BOLD)
            } else {
                Style::new().fg(Color::White)
            };
            Span::styled(u.ch.to_string(), style)
        })
        .collect();

    // 한글: 친 키가 조합된 모습을 목표 줄 바로 아래에 같은 폭으로 보여줍니다.
    let typed = match app.lang() {
        Lang::En => Line::raw(""),
        Lang::Ko => Line::from(
            app.target
                .units
                .iter()
                .map(|u| match app.target.typed_form(u, pos) {
                    Some(c) if pos < u.end => Span::styled(c.to_string(), Style::new().fg(Color::Cyan)),
                    Some(c) => Span::styled(c.to_string(), Style::new().fg(Color::DarkGray)),
                    None => Span::raw(" ".repeat(char_width(u.ch))),
                })
                .collect::<Vec<_>>(),
        ),
    };

    let message = match &app.message {
        Some(m) => Line::styled(
            m.clone(),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        None => Line::raw(""),
    };
    let text = vec![Line::raw(""), Line::from(target), typed, message];
    let focus = app
        .focus()
        .map(|c| format!(" focus: {} ", app.label(c)))
        .unwrap_or_default();
    let block = Block::new()
        .borders(Borders::ALL)
        .title(" Practice ")
        .title_bottom(Line::from(focus).right_aligned());
    f.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(block),
        area,
    );
}

fn draw_keyboard(f: &mut Frame, app: &App, area: Rect) {
    let next = app.lesson.current();
    let focus = app.focus();
    let ws = app.profile.word_starts();

    let mut lines: Vec<Line> = Vec::new();
    for (r, row) in app.layout.keys.iter().enumerate() {
        // 격자(오소리니어) 배열: 줄마다 들여쓰지 않아서 키가 세로로 맞춰집니다.
        let mut spans = Vec::new();
        for c in 0..row.len() {
            if c == COLS / 2 {
                spans.push(Span::raw("  "));
            }
            match app.layout.key_at(r, c) {
                Some(ch) => {
                    let label = app.label(ch);
                    // 칸 폭을 3으로 맞춤 (한글 자모는 2칸).
                    let cell = if char_width(label.chars().next().unwrap_or(' ')) == 2 {
                        format!(" {label}")
                    } else {
                        format!(" {label} ")
                    };
                    spans.push(Span::styled(cell, key_style(app, ch, next, focus, ws)));
                }
                None => spans.push(Span::styled(" · ", Style::new().fg(Color::DarkGray))),
            }
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }

    let block = Block::new().borders(Borders::ALL).title(" Keyboard ");
    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).block(block),
        area,
    );
}

fn key_style(app: &App, ch: char, next: Option<char>, focus: Option<char>, ws: WordStarts) -> Style {
    if Some(ch) == next {
        return Style::new()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
    }
    // Shift 글자(예: ㅆ = Shift + t)를 칠 차례면 다른 색으로.
    if next.is_some_and(|n| n.is_ascii_uppercase() && n.to_ascii_lowercase() == ch) {
        return Style::new()
            .fg(Color::Black)
            .bg(Color::Magenta)
            .add_modifier(Modifier::BOLD);
    }
    if !app.profile.is_unlocked(ch) {
        return Style::new().fg(Color::DarkGray);
    }
    if Some(ch) == focus {
        return Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    }
    let mastered = app.profile.stats(ch).is_some_and(|s| s.mastered(app.targets, ws));
    Style::new().fg(if mastered { Color::Green } else { Color::White })
}

fn draw_progress(f: &mut Frame, app: &App, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let per_row = (inner_width / ENTRY_WIDTH).max(1);
    let focus = app.focus();
    let ws = app.profile.word_starts();
    let speed_factor = match app.lang() {
        Lang::En => 1.0,
        Lang::Ko => 5.0,
    };

    let entries: Vec<Vec<Span>> = app
        .profile
        .unlocked_keys()
        .iter()
        .map(|&k| {
            let stats = app.profile.stats(k).cloned().unwrap_or_default();
            let p = stats.progress(app.targets, ws);
            // 내림: 막대가 꽉 차는 건 목표를 넘었을 때뿐 (반올림하면 목표의 94%만 돼도 꽉 차 보임).
            let filled = (p * BAR as f64).floor() as usize;
            let color = if stats.mastered(app.targets, ws) {
                Color::Green
            } else if Some(k) == focus {
                Color::Yellow
            } else {
                Color::White
            };
            // 목표에 못 미치는 값은 빨간색.
            let below = |low: bool| {
                if low {
                    Style::new().fg(Color::Red)
                } else {
                    Style::new()
                }
            };
            let speed = stats
                .wpm(ws)
                .map(|w| format!("{:>4.0}", w * speed_factor))
                .unwrap_or_else(|| "   -".into());
            let speed_style = below(stats.wpm(ws).is_some_and(|w| w < app.targets.wpm));
            let acc = stats
                .accuracy()
                .map(|a| format!(" {:>3.0}%", a * 100.0))
                .unwrap_or_else(|| "    -".into());
            let acc_style = below(stats.accuracy().is_some_and(|a| a < app.targets.accuracy));
            let label = app.label(k);
            let pad = if char_width(label.chars().next().unwrap_or(' ')) == 2 {
                ""
            } else {
                " "
            };
            vec![
                Span::styled(
                    format!("{label}{pad} "),
                    Style::new().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled("█".repeat(filled), Style::new().fg(color)),
                Span::styled("░".repeat(BAR - filled), Style::new().fg(Color::DarkGray)),
                Span::styled(speed, speed_style),
                Span::styled(acc, acc_style),
                Span::raw("  "),
            ]
        })
        .collect();

    let mut lines: Vec<Line> = entries
        .chunks(per_row)
        .map(|row| Line::from(row.concat()))
        .collect();
    // 아직 목표를 못 넘은 키 (모두 넘어야 다음 키가 열림).
    let waiting: Vec<String> = app
        .profile
        .unlocked_keys()
        .iter()
        .filter(|&&k| !app.profile.stats(k).is_some_and(|s| s.mastered(app.targets, ws)))
        .map(|&k| app.label(k))
        .collect();
    let next = match app.profile.next_locked() {
        Some(k) if waiting.is_empty() => format!(
            "Next unlock: {} - opens when this line is done",
            app.describe_key(k)
        ),
        Some(k) => format!(
            "Next unlock: {} - waiting for: {}",
            app.describe_key(k),
            waiting.join(" ")
        ),
        None => "All keys unlocked!".to_string(),
    };
    lines.push(Line::styled(next, Style::new().fg(Color::DarkGray)));

    let title = match app.lang() {
        Lang::En => " Per-key progress (WPM · accuracy) ",
        Lang::Ko => " Per-key progress (KPM · accuracy) ",
    };
    let block = Block::new().borders(Borders::ALL).title(title);
    f.render_widget(Paragraph::new(lines).block(block), area);
}
