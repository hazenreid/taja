//! 앱 상태: 지금 줄, 다음 줄, 진도, 알림 메시지.

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rand::SeedableRng;
use rand::rngs::StdRng;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::hangul;
use crate::layout::Layout;
use crate::layout::cost::{finger_name, row_name};
use crate::lesson::{Lesson, Outcome};
use crate::progress::{LineRecord, Profile, Targets};
use crate::storage::save_profile;
use crate::target::Target;
use crate::words::line::build_line;
use crate::words::{Lang, WordList};

pub const MIN_LINE: usize = 20;
pub const MAX_LINE: usize = 56;

const IME_WARNING: &str = "Switch your OS input to English (Han/Yeong key) - taja composes Hangul itself";

pub struct App {
    pub layout: Layout,
    pub words: WordList,
    pub profile: Profile,
    pub targets: Targets,
    pub target: Target,
    pub lesson: Lesson,
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
            target: Target::default(),
            lesson: Lesson::new(""),
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

    pub fn lang(&self) -> Lang {
        self.words.lang
    }

    pub fn focus(&self) -> Option<char> {
        self.profile.focus()
    }

    fn make_line(&mut self) -> Target {
        let focus = self.focus();
        let keys = self.profile.unlocked_keys().to_vec();
        Target::from_items(&build_line(
            &mut self.rng,
            &self.words,
            &keys,
            focus,
            &self.profile.missed,
            self.line_width,
        ))
    }

    fn start(&mut self, target: Target) {
        self.lesson = Lesson::new(&target.keys.iter().collect::<String>());
        self.target = target;
    }

    /// 줄을 새로 만듭니다.
    pub fn regenerate(&mut self) {
        let line = self.make_line();
        self.start(line);
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
        if hangul::is_hangul_input(c) {
            self.message = Some(IME_WARNING.to_string());
            return;
        }
        if self.message.as_deref() == Some(IME_WARNING) {
            self.message = None;
        }
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
        // 단어마다 틀렸는지 기록 (틀린 진짜 단어는 다음 줄들에 다시 나옴).
        for w in &self.target.words {
            let had_error = self.lesson.had_error[w.start..w.end].iter().any(|&e| e);
            self.profile.record_word(&w.item, had_error);
        }
        self.profile.push_history(LineRecord {
            time,
            wpm,
            accuracy: acc,
            unlocked: self.profile.unlocked,
        });

        self.message = self
            .profile
            .try_unlock(self.targets)
            .map(|k| format!("New key unlocked: {}", self.describe_key(k)));
        // 방금 틀린 단어와 새 집중 키가 바로 반영되도록 다음 줄은 지금 만듭니다.
        self.regenerate();
        self.save();
    }

    /// 키 이름 (한글이면 자모).
    pub fn label(&self, key: char) -> String {
        self.lang().key_label(key)
    }

    /// 예: `'e' (왼손 중지, 윗줄)`, `'ㅆ' (Shift + t, 왼손 검지, 윗줄)`.
    pub fn describe_key(&self, k: char) -> String {
        let base = k.to_ascii_lowercase();
        let press = match (self.lang(), k.is_ascii_uppercase()) {
            (Lang::En, _) => String::new(),
            (Lang::Ko, true) => format!("Shift + {base}, "),
            (Lang::Ko, false) => format!("{base}, "),
        };
        match self.layout.position(base) {
            Some((r, c)) => format!("'{}' ({press}{}, {})", self.label(k), finger_name(c), row_name(r)),
            None => format!("'{}'", self.label(k)),
        }
    }

    /// 속도 표시. 영어는 WPM, 한글은 분당 타수(= WPM × 5).
    pub fn speed(&self, wpm: f64) -> String {
        match self.lang() {
            Lang::En => format!("{wpm:.0} WPM"),
            Lang::Ko => format!("{:.0} KPM", wpm * 5.0),
        }
    }

    pub fn save(&mut self) {
        self.save_error = save_profile(&self.save_path, &self.profile)
            .err()
            .map(|e| format!("Save failed: {e}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::plan::make_plan;
    use std::time::Duration;

    fn app_for(words: WordList, name: &str) -> (App, PathBuf) {
        let layout = Layout::qwerty();
        let profile = Profile::new(&layout, &make_plan(&layout, &words));
        let dir = std::env::temp_dir().join(format!("taja-app-{name}-{}", std::process::id()));
        let targets = Targets {
            wpm: 25.0,
            accuracy: 0.95,
        };
        (
            App::new(layout, words, profile, targets, dir.join("p.json"), None),
            dir,
        )
    }

    /// 150ms 간격으로 완벽하게 계속 쳐서 다음 키가 열릴 때까지.
    fn type_until_unlock(app: &mut App) {
        let start = app.profile.unlocked;
        let mut now = Instant::now();
        for _ in 0..300 {
            if app.profile.unlocked > start {
                return;
            }
            for c in app.target.keys.clone() {
                now += Duration::from_millis(150);
                app.type_char(c, now);
            }
        }
        panic!("다음 키가 열리지 않음");
    }

    #[test]
    fn typing_lines_unlocks_next_key() {
        let (mut app, dir) = app_for(WordList::english(), "en");
        type_until_unlock(&mut app);
        let next = app.profile.order[app.profile.unlocked - 1];
        assert_eq!(app.profile.unlocked_keys().last(), Some(&next));
        assert!(app.message.as_deref().unwrap().contains(&format!("'{next}'")));
        assert!(app.save_error.is_none(), "{:?}", app.save_error);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn korean_typing_unlocks_next_key() {
        let (mut app, dir) = app_for(WordList::korean(), "ko");
        let before = app.profile.unlocked;
        type_until_unlock(&mut app);
        assert_eq!(app.profile.unlocked, before + 1);
        let new_key = *app.profile.unlocked_keys().last().unwrap();
        assert!(app.message.as_deref().unwrap().contains(&app.label(new_key)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn warns_when_korean_ime_is_on() {
        let (mut app, dir) = app_for(WordList::korean(), "ime");
        let pos = app.lesson.pos;
        app.type_char('ㅎ', Instant::now());
        assert_eq!(app.lesson.pos, pos);
        assert_eq!(app.message.as_deref(), Some(IME_WARNING));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn missed_real_words_come_back_while_they_have_the_focus_key() {
        let (mut app, dir) = app_for(WordList::english(), "review");
        let mut now = Instant::now();
        // 줄 하나를 치되, 진짜 단어마다 첫 글자에서 한 번씩 틀립니다.
        let target = app.target.clone();
        let starts: Vec<usize> = target.words.iter().map(|w| w.start).collect();
        for (i, &c) in target.keys.iter().enumerate() {
            now += Duration::from_millis(150);
            if starts.contains(&i) {
                app.type_char('q', now);
            }
            app.type_char(c, now);
        }
        // 틀린 단어 목록 = 이 줄의 진짜 단어들 (가짜 단어는 안 들어감).
        use std::collections::HashSet;
        let missed: HashSet<&str> = app.profile.missed.iter().map(|m| m.text.as_str()).collect();
        let real: HashSet<&str> = target
            .words
            .iter()
            .filter(|w| w.item.real)
            .map(|w| w.item.text.as_str())
            .collect();
        assert_eq!(missed, real);
        assert!(app.profile.missed.iter().all(|m| m.real));

        // 다음 줄들에는 집중 키가 든 틀린 단어가 다시 나와야 합니다.
        let focus = app.focus().unwrap();
        let with_focus: Vec<String> = app
            .profile
            .missed
            .iter()
            .filter(|m| m.keys.contains(focus))
            .map(|m| m.text.clone())
            .collect();
        if !with_focus.is_empty() {
            let mut seen = false;
            for _ in 0..30 {
                app.regenerate();
                seen |= app.target.words.iter().any(|w| with_focus.contains(&w.item.text));
            }
            assert!(seen, "틀린 단어가 다시 안 나옴: {with_focus:?}");
        }
        let _ = std::fs::remove_dir_all(dir);
    }
}
