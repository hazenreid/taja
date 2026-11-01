//! 화면 그리기.

use std::time::Instant;

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout as Split, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::app::App;
use crate::layout::cost::{finger_name, row_name};
use crate::layout::{COLS, ROWS};

const ROW_INDENT: [usize; ROWS] = [0, 1, 3];
const BAR: usize = 8;
const ENTRY_WIDTH: usize = 22;

pub fn draw(f: &mut Frame, app: &App) {
    let [header, practice, keyboard, progress, footer] = Split::vertical([
        Constraint::Length(1),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(f.area());

    draw_header(f, app, header);
    draw_practice(f, app, practice);
    draw_keyboard(f, app, keyboard);
    draw_progress(f, app, progress);

    let mut help = vec![Span::styled(
        " Esc 종료 · Tab 새 줄",
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
        Span::raw(format!(" {}  ", app.layout.name)),
        Span::styled("키 ", dim),
        Span::raw(format!("{}/{}  ", app.profile.unlocked, app.profile.order.len())),
        Span::styled("목표 ", dim),
        Span::raw(format!(
            "{:.0} WPM · {:.0}%  ",
            app.targets.wpm,
            app.targets.accuracy * 100.0
        )),
    ];
    if let Some(wpm) = app.lesson.wpm(Instant::now()) {
        spans.push(Span::styled("지금 ", dim));
        spans.push(Span::raw(format!("{wpm:.0} WPM  ")));
    }
    if let Some((wpm, acc)) = app.last_line {
        spans.push(Span::styled("지난 줄 ", dim));
        spans.push(Span::raw(format!("{wpm:.0} WPM · {:.0}%", acc * 100.0)));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_practice(f: &mut Frame, app: &App, area: Rect) {
    let lesson = &app.lesson;
    let blocked = lesson.is_blocked();
    let current: Vec<Span> = lesson
        .text
        .iter()
        .enumerate()
        .map(|(i, &ch)| {
            let style = if i < lesson.pos {
                if lesson.had_error[i] {
                    Style::new().fg(Color::Red)
                } else {
                    Style::new().fg(Color::Green)
                }
            } else if i == lesson.pos {
                if blocked {
                    Style::new()
                        .fg(Color::White)
                        .bg(Color::Red)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::new()
                        .fg(Color::Black)
                        .bg(Color::White)
                        .add_modifier(Modifier::BOLD)
                }
            } else {
                Style::new().fg(Color::White)
            };
            Span::styled(ch.to_string(), style)
        })
        .collect();

    let message = match &app.message {
        Some(m) => Line::styled(
            m.clone(),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        None => Line::raw(""),
    };
    let text = vec![
        Line::raw(""),
        Line::from(current),
        Line::raw(""),
        Line::styled(app.next_line.clone(), Style::new().fg(Color::DarkGray)),
        message,
    ];
    let focus = app.focus().map(|c| format!(" 집중 키: {c} ")).unwrap_or_default();
    let block = Block::new()
        .borders(Borders::ALL)
        .title(" 연습 ")
        .title_bottom(Line::from(focus).right_aligned());
    f.render_widget(
        Paragraph::new(text).alignment(Alignment::Center).block(block),
        area,
    );
}

fn draw_keyboard(f: &mut Frame, app: &App, area: Rect) {
    let next = app.lesson.current();
    let focus = app.focus();
    let max_indent = ROW_INDENT[ROWS - 1];

    let mut lines: Vec<Line> = Vec::new();
    for (r, row) in app.layout.keys.iter().enumerate() {
        let mut spans = vec![Span::raw(" ".repeat(ROW_INDENT[r]))];
        for c in 0..row.len() {
            if c == COLS / 2 {
                spans.push(Span::raw("  "));
            }
            match app.layout.key_at(r, c) {
                Some(ch) => spans.push(Span::styled(format!(" {ch} "), key_style(app, ch, next, focus))),
                None => spans.push(Span::styled(" · ", Style::new().fg(Color::DarkGray))),
            }
            spans.push(Span::raw(" "));
        }
        // 가운데 정렬해도 줄마다 엇갈린 모양이 유지되도록 오른쪽도 채웁니다.
        spans.push(Span::raw(" ".repeat(max_indent - ROW_INDENT[r])));
        lines.push(Line::from(spans));
    }

    let space_style = if next == Some(' ') {
        Style::new().fg(Color::Black).bg(Color::Cyan)
    } else {
        Style::new().fg(Color::White).bg(Color::DarkGray)
    };
    lines.push(Line::from(Span::styled(format!("{:^21}", "space"), space_style)));

    let hint = match next.and_then(|c| app.layout.position(c).map(|p| (c, p))) {
        Some((c, (r, col))) => format!("다음 키 '{c}': {} · {}", finger_name(col), row_name(r)),
        None if next == Some(' ') => "다음 키: 스페이스 (엄지)".to_string(),
        None => String::new(),
    };
    lines.push(Line::styled(hint, Style::new().fg(Color::Cyan)));

    let block = Block::new().borders(Borders::ALL).title(" 키보드 ");
    f.render_widget(
        Paragraph::new(lines).alignment(Alignment::Center).block(block),
        area,
    );
}

fn key_style(app: &App, ch: char, next: Option<char>, focus: Option<char>) -> Style {
    if Some(ch) == next {
        return Style::new()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);
    }
    if !app.profile.is_unlocked(ch) {
        return Style::new().fg(Color::DarkGray);
    }
    if Some(ch) == focus {
        return Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD);
    }
    let mastered = app.profile.stats(ch).is_some_and(|s| s.mastered(app.targets));
    Style::new().fg(if mastered { Color::Green } else { Color::White })
}

fn draw_progress(f: &mut Frame, app: &App, area: Rect) {
    let inner_width = area.width.saturating_sub(2) as usize;
    let per_row = (inner_width / ENTRY_WIDTH).max(1);
    let focus = app.focus();

    let entries: Vec<Vec<Span>> = app
        .profile
        .unlocked_keys()
        .iter()
        .map(|&k| {
            let stats = app.profile.stats(k).cloned().unwrap_or_default();
            let p = stats.progress(app.targets);
            let filled = (p * BAR as f64).round() as usize;
            let color = if stats.mastered(app.targets) {
                Color::Green
            } else if Some(k) == focus {
                Color::Yellow
            } else {
                Color::White
            };
            let wpm = stats
                .wpm()
                .map(|w| format!("{w:>4.0}"))
                .unwrap_or_else(|| "   -".into());
            let acc = stats
                .accuracy()
                .map(|a| format!(" {:>3.0}%", a * 100.0))
                .unwrap_or_else(|| "    -".into());
            vec![
                Span::styled(
                    format!("{k} "),
                    Style::new().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled("█".repeat(filled), Style::new().fg(color)),
                Span::styled("░".repeat(BAR - filled), Style::new().fg(Color::DarkGray)),
                Span::raw(format!("{wpm}{acc}  ")),
            ]
        })
        .collect();

    let mut lines: Vec<Line> = entries
        .chunks(per_row)
        .map(|row| Line::from(row.concat()))
        .collect();
    let next = match app.profile.next_locked() {
        Some(k) => format!("다음 언락: '{k}' — 열린 키가 모두 목표를 넘으면 열립니다"),
        None => "모든 키를 열었습니다!".to_string(),
    };
    lines.push(Line::styled(next, Style::new().fg(Color::DarkGray)));

    let block = Block::new()
        .borders(Borders::ALL)
        .title(" 키별 진도 (WPM · 정확도) ");
    f.render_widget(Paragraph::new(lines).block(block), area);
}
