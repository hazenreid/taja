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
    // 연습 칸: 테두리 2 + 빈 줄, 목표 줄, (한글만) 조합 줄, 알림 줄.
    let practice_height = match app.lang() {
        Lang::En => 5,
        Lang::Ko => 6,
    };
    let [header, practice, keyboard, progress, footer] = Split::vertical([
        Constraint::Length(1),
        Constraint::Length(practice_height),
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

/// 화면 폭(칸)에 맞게 자르고, 잘랐으면 끝에 `…`.
fn fit_width(text: &str, width: usize) -> String {
    let total: usize = text.chars().map(char_width).sum();
    if total <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = char_width(c);
        if used + w + 1 > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('…');
    out
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

    let mut text = vec![Line::raw(""), Line::from(target)];
    // 한글: 친 키가 조합된 모습을 목표 줄 바로 아래에 같은 폭으로 보여줍니다 (영어는 이 줄이 없음).
    if app.lang() == Lang::Ko {
        text.push(Line::from(
            app.target
                .units
                .iter()
                .map(|u| match app.target.typed_form(u, pos) {
                    Some(c) if pos < u.end => Span::styled(c.to_string(), Style::new().fg(Color::Cyan)),
                    Some(c) => Span::styled(c.to_string(), Style::new().fg(Color::DarkGray)),
                    None => Span::raw(" ".repeat(char_width(u.ch))),
                })
                .collect::<Vec<_>>(),
        ));
    }

    let message = match &app.message {
        Some(m) => Line::styled(
            m.clone(),
            Style::new().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        ),
        None => Line::raw(""),
    };
    text.push(message);
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
    let rows = area.height.saturating_sub(2) as usize;
    let per_row = (inner_width / ENTRY_WIDTH).max(1);
    let focus = app.focus();
    let ws = app.profile.word_starts();
    let speed_factor = match app.lang() {
        Lang::En => 1.0,
        Lang::Ko => 5.0,
    };
    let passed = |k: char| app.profile.stats(k).is_some_and(|s| s.mastered(app.targets, ws));

    // 키 하나: 이름, 막대, 속도, 정확도.
    let entry = |k: char| -> Vec<Span<'static>> {
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
    };
    let to_lines = |keys: &[char]| -> Vec<Line<'static>> {
        keys.chunks(per_row)
            .map(|row| Line::from(row.iter().flat_map(|&k| entry(k)).collect::<Vec<_>>()))
            .collect()
    };

    let keys = app.profile.unlocked_keys();
    let lines = if keys.len().div_ceil(per_row) <= rows {
        // 다 들어가면 열린 순서대로 전부.
        to_lines(keys)
    } else {
        // 자리가 모자라면 아직 통과 못 한 키를 먼저 보여 주고, 통과한 키는 마지막 한 줄로 줄입니다.
        let (done, todo): (Vec<char>, Vec<char>) = keys.iter().copied().partition(|&k| passed(k));
        let room = rows.saturating_sub(1) * per_row;
        let shown = todo.len().min(room);
        let mut lines = to_lines(&todo[..shown]);
        let mut summary = String::new();
        if shown < todo.len() {
            summary += &format!("+{} more  ", todo.len() - shown);
        }
        if !done.is_empty() {
            let labels: Vec<String> = done.iter().map(|&k| app.label(k)).collect();
            summary += &format!("✓ {} passed: {}", done.len(), labels.join(" "));
        }
        lines.push(Line::styled(
            fit_width(&summary, inner_width),
            Style::new().fg(Color::Green),
        ));
        lines
    };

    let title = match app.lang() {
        Lang::En => " Per-key progress (WPM · accuracy) ",
        Lang::Ko => " Per-key progress (KPM · accuracy) ",
    };
    let block = Block::new().borders(Borders::ALL).title(title);
    f.render_widget(Paragraph::new(lines).block(block), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Layout;
    use crate::layout::plan::make_plan;
    use crate::progress::{Profile, Sample, Targets};
    use crate::words::WordList;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// 키 `unlocked`개를 열고, 그중 앞의 `passed`개는 목표를 넘게 기록한 앱.
    fn app_with(unlocked: usize, passed: usize) -> App {
        let layout = Layout::qwerty();
        let words = WordList::english();
        let mut profile = Profile::new(&layout, &make_plan(&layout, &words));
        profile.unlocked = unlocked;
        let fast = Sample {
            ms: Some(200),
            clean: true,
            word_start: false,
        };
        let keys: Vec<char> = profile.unlocked_keys()[..passed].to_vec();
        for k in keys {
            for _ in 0..30 {
                profile.record(k, fast);
            }
        }
        let targets = Targets {
            wpm: 30.0,
            accuracy: 0.90,
        };
        let path = std::env::temp_dir().join("taja-ui-test-never-saved.json");
        App::new(layout, words, profile, targets, path, None)
    }

    fn screen(app: &App, w: u16, h: u16) -> Vec<String> {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, app)).unwrap();
        let buf = t.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol().to_string()).collect())
            .collect()
    }

    /// 화면에 그 키의 막대 칸이 있는지 (`e  ███…`).
    fn has_entry(lines: &[String], key: char) -> bool {
        let head = format!("{key}  ");
        lines.iter().any(|l| {
            l.match_indices(&head)
                .any(|(i, _)| l[i + head.len()..].starts_with(['█', '░']))
        })
    }

    #[test]
    fn progress_shows_every_key_in_order_when_it_fits() {
        let app = app_with(26, 20);
        let lines = screen(&app, 100, 40);
        assert!(app.profile.unlocked_keys().iter().all(|&k| has_entry(&lines, k)));
        assert!(!lines.iter().any(|l| l.contains("passed")));
        assert!(!lines.iter().any(|l| l.contains("Next unlock")));
    }

    #[test]
    fn progress_puts_unpassed_keys_first_when_space_is_short() {
        let app = app_with(26, 20);
        // 80×20: 진행 칸 안쪽이 6줄, 한 줄에 3개 → 26개(9줄)는 안 들어감.
        let lines = screen(&app, 80, 20);
        let keys = app.profile.unlocked_keys();
        assert!(keys[20..].iter().all(|&k| has_entry(&lines, k)), "{lines:#?}");
        assert!(!keys[..20].iter().any(|&k| has_entry(&lines, k)), "{lines:#?}");
        let summary = lines
            .iter()
            .find(|l| l.contains("✓ 20 passed:"))
            .expect("summary line");
        assert!(summary.contains(&format!("{} {}", keys[0], keys[1])), "{summary}");
    }

    #[test]
    fn fit_width_cuts_with_ellipsis() {
        assert_eq!(fit_width("abc", 3), "abc");
        assert_eq!(fit_width("abcdef", 4), "abc…");
        assert_eq!(fit_width("ㄱㄴㄷ", 5), "ㄱㄴ…");
    }
}
