//! 앱 상태: 지금 줄, 다음 줄, 진도, 알림 메시지.

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rand::SeedableRng;
use rand::rngs::StdRng;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::layout::Layout;
use crate::layout::cost::{finger_name, row_name};
use crate::lesson::{Lesson, Outcome};
use crate::progress::{LineRecord, Profile, Targets};
use crate::storage::save_profile;
use crate::words::WordList;
use crate::words::line::build_line;

pub const MIN_LINE: usize = 20;
pub const MAX_LINE: usize = 56;

pub struct App {
    pub layout: Layout,
    pub words: WordList,
    pub profile: Profile,
    pub targets: Targets,
    pub lesson: Lesson,
    pub next_line: String,
    pub message: Option<String>,
    pub last_line: Option<(f64, f64)>,
    pub save_path: PathBuf,
    pub save_error: Option<String>,
    pub line_width: usize,
    pub quit: bool,
    rng: StdRng,
}

impl App {
    pub fn new(
        layout: Layout,
        words: WordList,
        profile: Profile,
        targets: Targets,
        save_path: PathBuf,
        message: Option<String>,
    ) -> Self {
        let mut app = App {
            layout,
            words,
            profile,
            targets,
            lesson: Lesson::new(""),
            next_line: String::new(),
            message,
            last_line: None,
            save_path,
            save_error: None,
            line_width: MAX_LINE,
            quit: false,
            rng: StdRng::from_os_rng(),
        };
        app.regenerate();
        app
    }

    pub fn focus(&self) -> Option<char> {
        self.profile.focus(self.targets)
    }

    fn make_line(&mut self) -> String {
        let focus = self.focus();
        let keys = self.profile.unlocked_keys().to_vec();
        build_line(&mut self.rng, &self.words, &keys, focus, self.line_width)
    }

    /// 지금 줄과 다음 줄을 새로 만듭니다.
    pub fn regenerate(&mut self) {
        let line = self.make_line();
        self.lesson = Lesson::new(&line);
        self.next_line = self.make_line();
    }

    pub fn set_width(&mut self, terminal_width: u16) {
        self.line_width = (terminal_width as usize)
            .saturating_sub(8)
            .clamp(MIN_LINE, MAX_LINE);
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') if ctrl => self.quit = true,
            KeyCode::Tab => self.regenerate(),
            KeyCode::Char(c) if !ctrl => self.type_char(c, Instant::now()),
            _ => {}
        }
    }

    pub fn type_char(&mut self, c: char, now: Instant) {
        if let Outcome::Correct { ch, sample } = self.lesson.type_char(c, now) {
            self.profile.record(ch, sample);
            if self.lesson.is_done() {
                self.finish_line(now);
            }
        }
    }

    fn finish_line(&mut self, now: Instant) {
        let wpm = self.lesson.wpm(now).unwrap_or(0.0);
        let acc = self.lesson.accuracy().unwrap_or(0.0);
        self.last_line = Some((wpm, acc));
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.profile.push_history(LineRecord {
            time,
            wpm,
            accuracy: acc,
            unlocked: self.profile.unlocked,
        });

        if let Some(k) = self.profile.try_unlock(self.targets) {
            self.message = Some(self.describe_unlock(k));
            // 새 키가 바로 나오도록 다음 줄도 새로 만듭니다.
            self.regenerate();
        } else {
            self.message = None;
            let next = std::mem::take(&mut self.next_line);
            self.lesson = Lesson::new(&next);
            self.next_line = self.make_line();
        }
        self.save();
    }

    fn describe_unlock(&self, k: char) -> String {
        match self.layout.position(k) {
            Some((r, c)) => format!("새 키 언락: '{k}'  ({}, {})", finger_name(c), row_name(r)),
            None => format!("새 키 언락: '{k}'"),
        }
    }

    pub fn save(&mut self) {
        self.save_error = save_profile(&self.save_path, &self.profile)
            .err()
            .map(|e| format!("저장 실패: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::plan::make_plan;
    use std::time::Duration;

    #[test]
    fn typing_lines_unlocks_next_key() {
        let layout = Layout::qwerty();
        let words = WordList::english();
        let profile = Profile::new(&layout, &make_plan(&layout, &words));
        let dir = std::env::temp_dir().join(format!("taja-app-{}", std::process::id()));
        let targets = Targets {
            wpm: 25.0,
            accuracy: 0.95,
        };
        let mut app = App::new(layout, words, profile, targets, dir.join("p.json"), None);

        // 150ms 간격으로 완벽하게 계속 치면 언젠가 'e'가 열려야 합니다.
        let mut now = Instant::now();
        for _ in 0..200 {
            if app.profile.unlocked > 7 {
                break;
            }
            for c in app.lesson.text.clone() {
                now += Duration::from_millis(150);
                app.type_char(c, now);
            }
        }
        assert_eq!(app.profile.unlocked_keys().last(), Some(&'e'));
        assert!(app.message.as_deref().unwrap().contains("'e'"));
        assert!(app.save_error.is_none(), "{:?}", app.save_error);
        let _ = std::fs::remove_dir_all(dir);
    }
}
