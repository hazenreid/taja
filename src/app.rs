//! 앱 상태: 지금 줄, 다음 줄, 진도, 알림 메시지.

use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use rand::SeedableRng;
use rand::rngs::StdRng;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::corpus::Ngrams;
use crate::hangul;
use crate::layout::Layout;
use crate::layout::cost::{finger_name, row_name};
use crate::layout::plan::stage_goals;
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
    /// 트라이그램 빈도 (단계마다 목표 트라이그램을 고를 때 씀).
    ngrams: Ngrams,
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
    /// 이 줄을 시작할 때 이미 통과해 있던 목표 트라이그램 (줄을 마치면 새로 통과한 것을 알려 줌).
    passed_at_start: Vec<[char; 3]>,
    rng: StdRng,
}

impl App {
    pub fn new(
        layout: Layout,
        words: WordList,
        ngrams: Ngrams,
        profile: Profile,
        targets: Targets,
        save_path: PathBuf,
        message: Option<String>,
    ) -> Self {
        let mut app = App {
            layout,
            words,
            ngrams,
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
            passed_at_start: Vec::new(),
            rng: StdRng::from_os_rng(),
        };
        app.ensure_goals();
        app.regenerate();
        app
    }

    /// 이번 단계의 목표 트라이그램이 아직 없으면 정합니다 (새 키가 열렸을 때, 예전 진도 파일).
    fn ensure_goals(&mut self) {
        if self.profile.goals.is_empty() {
            let unlocked = self.profile.unlocked_keys().to_vec();
            let new_keys = self.profile.stage_new_keys();
            self.profile.goals = stage_goals(&self.words, &self.ngrams, &unlocked, &new_keys);
        }
    }

    /// 아직 목표를 못 넘은 목표 트라이그램 (가장 연습이 필요한 것이 앞).
    pub fn pending_goals(&self) -> Vec<[char; 3]> {
        self.profile
            .goals_by_need(self.targets)
            .into_iter()
            .filter(|&g| !self.profile.goal_mastered(g, self.targets))
            .collect()
    }

    fn passed_goals(&self) -> Vec<[char; 3]> {
        self.profile
            .goals
            .iter()
            .copied()
            .filter(|&g| self.profile.goal_mastered(g, self.targets))
            .collect()
    }

    /// 지금 연습할 키 순서들: 못 넘은 목표 트라이그램 (가장 약한 것이 앞),
    /// 모두 넘었으면 집중 키 하나.
    pub fn patterns(&self) -> Vec<String> {
        let goals = self.pending_goals();
        if goals.is_empty() {
            self.focus().map(|k| vec![k.to_string()]).unwrap_or_default()
        } else {
            goals.iter().map(|g| g.iter().collect()).collect()
        }
    }

    /// 키 순서를 화면에 보일 글자로 (한글이면 자모): `her`, `ㅎㅏㄷ`.
    pub fn pattern_label(&self, keys: &str) -> String {
        keys.chars().map(|k| self.label(k)).collect()
    }

    pub fn lang(&self) -> Lang {
        self.words.lang
    }

    pub fn focus(&self) -> Option<char> {
        self.profile.focus(self.targets)
    }

    fn make_line(&mut self) -> Target {
        let patterns = self.patterns();
        let keys = self.profile.unlocked_keys().to_vec();
        let weak: Vec<([char; 2], f64)> = self
            .profile
            .weak_pairs
            .iter()
            .map(|w| (w.pair, w.score))
            .collect();
        Target::from_items(&build_line(
            &mut self.rng,
            &self.words,
            &keys,
            &patterns,
            &self.profile.missed,
            &weak,
            self.line_width,
        ))
    }

    fn start(&mut self, target: Target) {
        self.lesson = Lesson::new(&target.keys.iter().collect::<String>());
        self.target = target;
        self.passed_at_start = self.passed_goals();
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
        if let Outcome::Correct { ch, prev, sample } = self.lesson.type_char(c, now) {
            self.profile.record(ch, sample);
            // 앞 키 → 이 키 연결이 약한지 기록 (틀렸으면 점수 올림, 한 번에 맞히면 깎음).
            self.profile.record_pair(prev, ch, sample.clean);
            // 방금 끝난 트라이그램이 목표 트라이그램이면 기록.
            if let Some((tri, s)) = self.lesson.trigram_at(self.lesson.pos - 1) {
                self.profile.record_trigram(tri, s);
            }
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

        let newly_passed: Vec<[char; 3]> = self
            .passed_goals()
            .into_iter()
            .filter(|g| !self.passed_at_start.contains(g))
            .collect();
        self.message = match self.profile.try_unlock(self.targets) {
            Some(k) => {
                self.ensure_goals();
                let goals: Vec<String> = self
                    .profile
                    .goals
                    .iter()
                    .map(|g| self.pattern_label(&g.iter().collect::<String>()))
                    .collect();
                let mut msg = format!("New key unlocked: {}", self.describe_key(k));
                if !goals.is_empty() {
                    msg += &format!(" · next goals: {}", goals.join(" "));
                }
                Some(msg)
            }
            None if !newly_passed.is_empty() => {
                let names: Vec<String> = newly_passed
                    .iter()
                    .map(|g| format!("'{}'", self.pattern_label(&g.iter().collect::<String>())))
                    .collect();
                Some(format!(
                    "Goal passed: {} ({}/{})",
                    names.join(" "),
                    self.passed_goals().len(),
                    self.profile.goals.len()
                ))
            }
            None => None,
        };
        // 방금 틀린 단어와 새 목표가 바로 반영되도록 다음 줄은 지금 만듭니다.
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
    use crate::layout::plan::make_plan_with;
    use std::time::Duration;

    fn app_for(words: WordList, name: &str) -> (App, PathBuf) {
        let layout = Layout::qwerty();
        let ngrams = Ngrams::from_words(&words);
        let profile = Profile::new(&layout, &make_plan_with(&layout, &words, &ngrams));
        let dir = std::env::temp_dir().join(format!("taja-app-{name}-{}", std::process::id()));
        let targets = Targets {
            wpm: 25.0,
            accuracy: 0.95,
        };
        (
            App::new(layout, words, ngrams, profile, targets, dir.join("p.json"), None),
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

        // 다음 줄들에는 연습 중인 키 순서가 든 틀린 단어가 다시 나와야 합니다.
        let patterns = app.patterns();
        let with_focus: Vec<String> = app
            .profile
            .missed
            .iter()
            .filter(|m| patterns.iter().any(|p| m.keys.contains(p.as_str())))
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

    #[test]
    fn goal_trigrams_shape_lines_and_gate_unlocks() {
        for words in [WordList::english(), WordList::korean()] {
            let lang = words.lang.name();
            let (mut app, dir) = app_for(words, &format!("goals-{lang}"));
            let goals = app.profile.goals.clone();
            assert!(!goals.is_empty() && goals.len() <= 5, "{lang}: {goals:?}");
            // 줄의 모든 단어에 아직 못 넘은 목표 트라이그램이 하나 이상 들어 있음.
            for _ in 0..20 {
                app.regenerate();
                let patterns = app.patterns();
                for w in &app.target.words {
                    let keys = &w.item.keys;
                    assert!(
                        patterns.iter().any(|p| keys.contains(p.as_str())),
                        "{lang}: {keys} {patterns:?}"
                    );
                }
            }
            type_until_unlock(&mut app);
            // 목표를 모두 넘어서 열렸고, 새 단계의 목표에는 새 키가 들어 있음.
            assert!(
                goals.iter().all(|&g| app.profile.goal_mastered(g, app.targets)),
                "{lang}"
            );
            let new_key = *app.profile.unlocked_keys().last().unwrap();
            assert!(!app.profile.goals.is_empty(), "{lang}");
            assert!(
                app.profile.goals.iter().all(|g| g.contains(&new_key)),
                "{lang}: {:?}",
                app.profile.goals
            );
            assert!(
                app.message.as_deref().unwrap().contains("next goals"),
                "{:?}",
                app.message
            );
            let _ = std::fs::remove_dir_all(dir);
        }
    }

    #[test]
    fn passing_a_goal_shows_a_message() {
        let (mut app, dir) = app_for(WordList::english(), "passed");
        let mut now = Instant::now();
        for _ in 0..100 {
            for c in app.target.keys.clone() {
                now += Duration::from_millis(150);
                app.type_char(c, now);
            }
            if app
                .message
                .as_deref()
                .is_some_and(|m| m.starts_with("Goal passed"))
            {
                let _ = std::fs::remove_dir_all(dir);
                return;
            }
        }
        panic!("no 'Goal passed' message: {:?}", app.message);
    }

    #[test]
    fn mistakes_record_weak_pairs() {
        let (mut app, dir) = app_for(WordList::english(), "weak");
        let keys = app.target.keys.clone();
        let mut now = Instant::now();
        // 첫 키는 맞게, 두 번째 키에서 한 번 틀린 뒤 맞게 → 바로 (첫 키 → 둘째 키) 연결이 기록됨.
        app.type_char(keys[0], now);
        now += Duration::from_millis(150);
        app.type_char('Q', now);
        app.type_char(keys[1], now);
        let pair = [keys[0], keys[1]];
        let score = app
            .profile
            .weak_pairs
            .iter()
            .find(|w| w.pair == pair)
            .map(|w| w.score);
        assert_eq!(score, Some(1.0), "{:?}", app.profile.weak_pairs);
        let _ = std::fs::remove_dir_all(dir);
    }
}
