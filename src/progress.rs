//! 키별 속도/정확도 기록과 언락 판정.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::layout::plan::Plan;
use crate::words::line::Item;

/// 키마다 최근 몇 번의 입력으로 평가할지.
pub const WINDOW: usize = 30;
/// 언락 판정에 필요한 최소 입력 횟수 (키).
pub const MIN_SAMPLES: usize = 20;
/// 목표 트라이그램 판정에 필요한 최소 횟수.
pub const TRI_MIN_SAMPLES: usize = 10;
/// 틀린 단어는 이만큼만 보관 (넘치면 오래된 것부터 버림).
pub const MAX_MISSED: usize = 100;
/// 약한 연결(앞 키 → 키)은 이만큼만 보관 (넘치면 점수가 가장 낮은 것부터 버림).
pub const MAX_WEAK_PAIRS: usize = 50;
/// 틀릴 때 더하는 점수와 최대 점수.
const WEAK_HIT: f64 = 1.0;
const WEAK_MAX: f64 = 5.0;
/// 한 번에 맞게 칠 때마다 곱하는 값, 이보다 낮아지면 목록에서 뺌.
const WEAK_DECAY: f64 = 0.8;
const WEAK_MIN: f64 = 0.3;
/// 기록을 이만큼만 보관.
const MAX_HISTORY: usize = 2000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Targets {
    pub wpm: f64,
    /// 0.0 ~ 1.0
    pub accuracy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sample {
    /// 바로 앞 키를 친 뒤 이 키를 맞게 칠 때까지 걸린 시간. 너무 오래 쉬었으면 None.
    pub ms: Option<u32>,
    /// 한 번에 맞게 쳤는지.
    pub clean: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyStats {
    samples: VecDeque<Sample>,
}

impl KeyStats {
    pub fn record(&mut self, s: Sample) {
        self.samples.push_back(s);
        while self.samples.len() > WINDOW {
            self.samples.pop_front();
        }
    }

    pub fn count(&self) -> usize {
        self.samples.len()
    }

    pub fn accuracy(&self) -> Option<f64> {
        if self.samples.is_empty() {
            return None;
        }
        let clean = self.samples.iter().filter(|s| s.clean).count();
        Some(clean as f64 / self.samples.len() as f64)
    }

    /// 평균 키 간격을 WPM으로 (단어 = 5타).
    pub fn wpm(&self) -> Option<f64> {
        let times: Vec<u32> = self.samples.iter().filter_map(|s| s.ms).collect();
        if times.is_empty() {
            return None;
        }
        let avg = times.iter().map(|&t| t as f64).sum::<f64>() / times.len() as f64;
        Some(60_000.0 / (avg.max(1.0) * 5.0))
    }

    /// 목표까지 얼마나 왔는지 0.0 ~ 1.0. 입력 횟수가 `min`보다 적으면 그만큼 깎습니다.
    pub fn progress_with(&self, t: Targets, min: usize) -> f64 {
        let (Some(wpm), Some(acc)) = (self.wpm(), self.accuracy()) else {
            return 0.0;
        };
        let p = (wpm / t.wpm).min(acc / t.accuracy).min(1.0);
        let enough = (self.count() as f64 / min as f64).min(1.0);
        p * enough
    }

    pub fn mastered_with(&self, t: Targets, min: usize) -> bool {
        self.count() >= min
            && self.wpm().is_some_and(|w| w >= t.wpm)
            && self.accuracy().is_some_and(|a| a >= t.accuracy)
    }

    /// 키 기준 (MIN_SAMPLES).
    pub fn progress(&self, t: Targets) -> f64 {
        self.progress_with(t, MIN_SAMPLES)
    }

    pub fn mastered(&self, t: Targets) -> bool {
        self.mastered_with(t, MIN_SAMPLES)
    }

    /// 연습이 필요한 정도 (작을수록 먼저). `min`번을 못 채운 것 → 정확도 미달(낮은 순)
    /// → 속도 미달(느린 순) → 모두 넘음(정확도 낮은 순).
    fn need_rank(&self, t: Targets, min: usize) -> (u8, f64, f64) {
        let acc = self.accuracy().unwrap_or(0.0);
        let wpm = self.wpm().unwrap_or(0.0);
        if self.count() < min {
            (0, 0.0, 0.0)
        } else if acc < t.accuracy {
            (1, acc, wpm)
        } else if wpm < t.wpm {
            (2, wpm, acc)
        } else {
            (3, acc, wpm)
        }
    }
}

fn cmp_rank(a: (u8, f64, f64), b: (u8, f64, f64)) -> std::cmp::Ordering {
    a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.total_cmp(&b.2))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineRecord {
    /// 유닉스 시간(초).
    pub time: u64,
    pub wpm: f64,
    pub accuracy: f64,
    pub unlocked: usize,
}

/// 레이아웃 하나에 대한 진도. 레이아웃마다 파일 하나로 저장됩니다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub layout_rows: Vec<String>,
    pub order: Vec<char>,
    pub unlocked: usize,
    /// `order`와 같은 순서.
    pub stats: Vec<KeyStats>,
    pub history: Vec<LineRecord>,
    /// 전에 틀린 진짜 단어 (오래된 것이 앞). 예전 진도 파일에는 없으므로 기본값은 빈 목록.
    #[serde(default)]
    pub missed: Vec<Item>,
    /// 약한 연결: 바로 앞 키 → 이 키를 칠 때 틀린 기록. 공백은 단어 경계.
    #[serde(default)]
    pub weak_pairs: Vec<WeakPair>,
    /// 시작 키 개수 (예전 진도 파일에는 없어서 0).
    #[serde(default)]
    pub start_len: usize,
    /// 지금 단계의 목표 트라이그램 (키 순서). 모두 통과하면 다음 키가 열림.
    /// 비어 있으면 예전처럼 열린 키가 모두 목표를 넘어야 열림.
    #[serde(default)]
    pub goals: Vec<[char; 3]>,
    /// 목표 트라이그램 기록 (지나간 단계 것도 남김).
    #[serde(default)]
    pub trigram_stats: Vec<TrigramStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrigramStats {
    pub tri: [char; 3],
    pub stats: KeyStats,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeakPair {
    /// [앞 키, 키]. 예: `the`에서 h를 틀리면 ['t', 'h'], 단어 첫 글자면 [' ', 't'].
    pub pair: [char; 2],
    pub score: f64,
}

impl Profile {
    pub fn new(layout: &Layout, plan: &Plan) -> Self {
        Profile {
            layout_rows: layout.rows_as_strings(),
            order: plan.order.clone(),
            unlocked: plan.start_len,
            stats: vec![KeyStats::default(); plan.order.len()],
            history: Vec::new(),
            missed: Vec::new(),
            weak_pairs: Vec::new(),
            start_len: plan.start_len,
            goals: Vec::new(),
            trigram_stats: Vec::new(),
        }
    }

    /// 저장된 프로필이 이 레이아웃용이고 내용이 망가지지 않았는지.
    pub fn is_valid_for(&self, layout: &Layout) -> bool {
        self.layout_rows == layout.rows_as_strings()
            && self.stats.len() == self.order.len()
            && self.unlocked <= self.order.len()
            && self
                .order
                .iter()
                .all(|&c| layout.position(c.to_ascii_lowercase()).is_some())
    }

    pub fn unlocked_keys(&self) -> &[char] {
        &self.order[..self.unlocked]
    }

    pub fn is_unlocked(&self, ch: char) -> bool {
        self.unlocked_keys().contains(&ch)
    }

    pub fn next_locked(&self) -> Option<char> {
        self.order.get(self.unlocked).copied()
    }

    pub fn stats(&self, ch: char) -> Option<&KeyStats> {
        let i = self.order.iter().position(|&c| c == ch)?;
        self.stats.get(i)
    }

    pub fn record(&mut self, ch: char, s: Sample) {
        if let Some(i) = self.order.iter().position(|&c| c == ch) {
            self.stats[i].record(s);
        }
    }

    /// 집중 키: 열린 키 중 가장 연습이 필요한 키.
    /// 1. 아직 MIN_SAMPLES번을 못 친 키 (새로 열린 키는 기록이 없어서 먼저)
    /// 2. 정확도가 목표 미달인 키 중 정확도가 가장 낮은 키
    /// 3. 속도가 목표 미달인 키 중 속도가 가장 느린 키
    /// 4. 모두 목표를 넘었으면 정확도가 가장 낮은 키 (같으면 느린 키)
    ///
    /// 같은 단계에서 값이 같으면 나중에 열린 키.
    pub fn focus(&self, t: Targets) -> Option<char> {
        self.unlocked_keys()
            .iter()
            .enumerate()
            .rev()
            .min_by(|a, b| {
                cmp_rank(
                    self.stats[a.0].need_rank(t, MIN_SAMPLES),
                    self.stats[b.0].need_rank(t, MIN_SAMPLES),
                )
            })
            .map(|(_, &c)| c)
    }

    /// 지금 단계에서 새로 열린 키들 (첫 단계면 시작 키 전부, 그다음부터는 마지막에 열린 키).
    pub fn stage_new_keys(&self) -> Vec<char> {
        if self.unlocked <= self.start_len {
            self.unlocked_keys().to_vec()
        } else {
            self.order[self.unlocked - 1..self.unlocked].to_vec()
        }
    }

    pub fn trigram(&self, tri: [char; 3]) -> Option<&KeyStats> {
        self.trigram_stats.iter().find(|t| t.tri == tri).map(|t| &t.stats)
    }

    /// 목표 트라이그램을 쳤을 때만 기록합니다.
    pub fn record_trigram(&mut self, tri: [char; 3], s: Sample) {
        if !self.goals.contains(&tri) {
            return;
        }
        match self.trigram_stats.iter_mut().find(|t| t.tri == tri) {
            Some(t) => t.stats.record(s),
            None => {
                let mut stats = KeyStats::default();
                stats.record(s);
                self.trigram_stats.push(TrigramStats { tri, stats });
            }
        }
    }

    /// 목표 트라이그램을 연습이 필요한 순서로 (못 넘은 것이 앞). 같으면 원래 순서(빈도순).
    pub fn goals_by_need(&self, t: Targets) -> Vec<[char; 3]> {
        let empty = KeyStats::default();
        let mut goals = self.goals.clone();
        goals.sort_by(|a, b| {
            let sa = self.trigram(*a).unwrap_or(&empty);
            let sb = self.trigram(*b).unwrap_or(&empty);
            cmp_rank(sa.need_rank(t, TRI_MIN_SAMPLES), sb.need_rank(t, TRI_MIN_SAMPLES))
        });
        goals
    }

    pub fn goal_mastered(&self, tri: [char; 3], t: Targets) -> bool {
        self.trigram(tri)
            .is_some_and(|s| s.mastered_with(t, TRI_MIN_SAMPLES))
    }

    /// 다음 키를 열 조건을 채웠으면 열고 그 키를 돌려줍니다. 목표 트라이그램은 비워지므로
    /// 새 단계의 목표를 다시 정해야 합니다.
    /// - 목표 트라이그램이 있으면: 목표가 모두 목표 속도·정확도를 넘었을 때
    /// - 없으면 (연습할 트라이그램이 없는 단계): 이번 단계에 열린 키가 모두 목표를 넘었을 때
    pub fn try_unlock(&mut self, t: Targets) -> Option<char> {
        let next = self.next_locked()?;
        let ready = if self.goals.is_empty() {
            self.stage_new_keys()
                .iter()
                .all(|&k| self.stats(k).is_some_and(|s| s.mastered(t)))
        } else {
            self.goals.iter().all(|&g| self.goal_mastered(g, t))
        };
        if ready {
            self.unlocked += 1;
            self.goals.clear();
            Some(next)
        } else {
            None
        }
    }

    /// 키 하나를 맞게 쳤을 때 (`prev` → `ch`). 이 자리에서 틀렸었으면 그 연결의 점수를 올리고,
    /// 한 번에 맞혔으면 점수를 깎아서 충분히 낮아지면 목록에서 뺍니다.
    pub fn record_pair(&mut self, prev: char, ch: char, clean: bool) {
        let pair = [prev, ch];
        let i = self.weak_pairs.iter().position(|w| w.pair == pair);
        match (i, clean) {
            (Some(i), true) => {
                self.weak_pairs[i].score *= WEAK_DECAY;
                if self.weak_pairs[i].score < WEAK_MIN {
                    self.weak_pairs.remove(i);
                }
            }
            (Some(i), false) => {
                let w = &mut self.weak_pairs[i];
                w.score = (w.score + WEAK_HIT).min(WEAK_MAX);
            }
            (None, true) => {}
            (None, false) => {
                self.weak_pairs.push(WeakPair {
                    pair,
                    score: WEAK_HIT,
                });
                if self.weak_pairs.len() > MAX_WEAK_PAIRS {
                    let lowest = (0..self.weak_pairs.len())
                        .min_by(|&a, &b| self.weak_pairs[a].score.total_cmp(&self.weak_pairs[b].score))
                        .unwrap();
                    self.weak_pairs.remove(lowest);
                }
            }
        }
    }

    /// 단어 하나를 다 쳤을 때. 틀렸으면 목록에 넣고(진짜 단어만), 한 번에 맞혔으면 목록에서 뺍니다.
    pub fn record_word(&mut self, item: &Item, had_error: bool) {
        if !had_error {
            self.missed.retain(|m| m.text != item.text);
        } else if item.real && !self.missed.iter().any(|m| m.text == item.text) {
            self.missed.push(item.clone());
            if self.missed.len() > MAX_MISSED {
                self.missed.remove(0);
            }
        }
    }

    pub fn push_history(&mut self, r: LineRecord) {
        self.history.push(r);
        if self.history.len() > MAX_HISTORY {
            let extra = self.history.len() - MAX_HISTORY;
            self.history.drain(..extra);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::plan::make_plan;
    use crate::words::WordList;

    const T: Targets = Targets {
        wpm: 25.0,
        accuracy: 0.95,
    };

    fn fast() -> Sample {
        // 200ms = 60 WPM
        Sample {
            ms: Some(200),
            clean: true,
        }
    }

    fn profile() -> Profile {
        let l = Layout::qwerty();
        Profile::new(&l, &make_plan(&l, &WordList::english()))
    }

    #[test]
    fn wpm_and_accuracy() {
        let mut s = KeyStats::default();
        s.record(Sample {
            ms: Some(480),
            clean: true,
        }); // 25 WPM
        s.record(Sample {
            ms: None,
            clean: false,
        });
        assert_eq!(s.wpm(), Some(25.0));
        assert_eq!(s.accuracy(), Some(0.5));
    }

    #[test]
    fn keeps_only_window() {
        let mut s = KeyStats::default();
        for _ in 0..WINDOW + 10 {
            s.record(fast());
        }
        assert_eq!(s.count(), WINDOW);
    }

    #[test]
    fn needs_min_samples_to_master() {
        let mut s = KeyStats::default();
        for _ in 0..MIN_SAMPLES - 1 {
            s.record(fast());
        }
        assert!(!s.mastered(T));
        s.record(fast());
        assert!(s.mastered(T));
    }

    #[test]
    fn without_goals_only_the_new_key_must_reach_the_target() {
        let mut p = profile();
        p.unlocked += 1;
        let new_key = p.order[p.unlocked - 1];
        assert_eq!(p.try_unlock(T), None);
        for _ in 0..MIN_SAMPLES {
            p.record(new_key, fast());
        }
        // 시작 키는 하나도 안 쳤어도 새 키만 목표를 넘으면 열림.
        let next = p.next_locked();
        assert_eq!(p.try_unlock(T), next);
    }

    #[test]
    fn unlocks_only_when_all_start_keys_mastered() {
        let mut p = profile();
        let keys: Vec<char> = p.unlocked_keys().to_vec();
        for &k in &keys[..keys.len() - 1] {
            for _ in 0..MIN_SAMPLES {
                p.record(k, fast());
            }
        }
        assert_eq!(p.try_unlock(T), None);
        // 아직 안 친 마지막 키가 집중 키.
        assert_eq!(p.focus(T), keys.last().copied());
        for _ in 0..MIN_SAMPLES {
            p.record(*keys.last().unwrap(), fast());
        }
        let next = p.next_locked();
        assert_eq!(p.try_unlock(T), next);
        assert_eq!(p.focus(T), next);
    }

    #[test]
    fn slow_or_sloppy_keys_block_unlock() {
        let mut p = profile();
        let keys: Vec<char> = p.unlocked_keys().to_vec();
        for &k in &keys {
            for _ in 0..MIN_SAMPLES {
                p.record(k, fast());
            }
        }
        let slow = keys[1];
        for _ in 0..WINDOW {
            p.record(
                slow,
                Sample {
                    ms: Some(1000),
                    clean: true,
                },
            ); // 12 WPM
        }
        assert_eq!(p.try_unlock(T), None);
        assert_eq!(p.focus(T), Some(slow));
    }

    #[test]
    fn json_roundtrip() {
        let mut p = profile();
        p.record('a', fast());
        let json = serde_json::to_string(&p).unwrap();
        let back: Profile = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
        assert!(back.is_valid_for(&Layout::qwerty()));
    }

    #[test]
    fn remembers_missed_real_words() {
        let mut p = profile();
        let word = |w: &str, real: bool| Item {
            text: w.into(),
            keys: w.into(),
            real,
        };
        p.record_word(&word("sad", true), true);
        p.record_word(&word("sad", true), true); // 중복 없음
        p.record_word(&word("fjdk", false), true); // 가짜 단어는 안 넣음
        p.record_word(&word("lad", true), false); // 맞힌 단어는 안 넣음
        assert_eq!(p.missed, [word("sad", true)]);
        p.record_word(&word("sad", true), false); // 맞히면 빠짐
        assert!(p.missed.is_empty());
        for i in 0..MAX_MISSED + 5 {
            p.record_word(&word(&format!("w{i}"), true), true);
        }
        assert_eq!(p.missed.len(), MAX_MISSED);
        assert_eq!(p.missed[0].text, "w5");
    }

    #[test]
    fn old_profiles_without_missed_still_load() {
        let mut json: serde_json::Value = serde_json::to_value(profile()).unwrap();
        json.as_object_mut().unwrap().remove("missed");
        let p: Profile = serde_json::from_value(json).unwrap();
        assert!(p.missed.is_empty());
    }

    #[test]
    fn focus_picks_unmet_accuracy_then_unmet_speed() {
        let mut p = profile();
        let keys: Vec<char> = p.unlocked_keys().to_vec();
        let (a, b, c) = (keys[0], keys[1], keys[2]);
        for &k in &keys {
            for _ in 0..WINDOW {
                p.record(k, fast());
            }
        }
        let set = |p: &mut Profile, k: char, ms: u32, wrong_every: usize| {
            for i in 0..WINDOW {
                p.record(
                    k,
                    Sample {
                        ms: Some(ms),
                        clean: wrong_every == 0 || i % wrong_every != 0,
                    },
                );
            }
        };
        // a: 정확도 96.7%(목표 95% 넘음)지만 다른 키보다 낮음, b: 느림(목표 미달), 나머지: 완벽.
        set(&mut p, a, 200, 30);
        set(&mut p, b, 1000, 0);
        // 목표를 넘은 a가 아니라, 목표 미달인 b가 집중 키.
        assert_eq!(p.focus(T), Some(b));
        // c가 정확도 미달이 되면, 속도 미달인 b보다 c가 먼저.
        set(&mut p, c, 200, 5); // 80%
        assert_eq!(p.focus(T), Some(c));
        // 모두 목표를 넘으면 정확도가 가장 낮은 키.
        set(&mut p, b, 200, 0);
        set(&mut p, c, 200, 0);
        assert_eq!(p.focus(T), Some(a));
    }

    #[test]
    fn weak_pairs_grow_on_errors_and_fade_on_clean_hits() {
        let mut p = profile();
        p.record_pair('t', 'h', false);
        p.record_pair('t', 'h', false);
        p.record_pair(' ', 't', false);
        p.record_pair('h', 'e', true); // 기록 없는 연결을 맞힌 건 무시
        assert_eq!(p.weak_pairs.len(), 2);
        assert_eq!(p.weak_pairs[0].pair, ['t', 'h']);
        assert_eq!(p.weak_pairs[0].score, 2.0);
        for _ in 0..10 {
            p.record_pair('t', 'h', false);
        }
        assert_eq!(p.weak_pairs[0].score, WEAK_MAX);
        // 한 번에 맞게 치면 점점 줄다가 빠짐.
        let mut n = 0;
        while p.weak_pairs.iter().any(|w| w.pair == ['t', 'h']) {
            p.record_pair('t', 'h', true);
            n += 1;
        }
        assert!((5..30).contains(&n), "{n}");
    }

    #[test]
    fn weak_pairs_are_capped() {
        let mut p = profile();
        p.record_pair('a', 'b', false);
        p.record_pair('a', 'b', false); // 점수 2라 안 버려짐
        for i in 0..MAX_WEAK_PAIRS + 5 {
            p.record_pair('x', char::from(b'a' + (i % 26) as u8), false);
            p.record_pair('y', char::from(b'a' + (i % 26) as u8), false);
        }
        assert_eq!(p.weak_pairs.len(), MAX_WEAK_PAIRS);
        assert!(p.weak_pairs.iter().any(|w| w.pair == ['a', 'b']));
    }

    #[test]
    fn goal_trigrams_gate_the_next_key() {
        let mut p = profile();
        let keys: Vec<char> = p.unlocked_keys().to_vec();
        let g1 = [keys[0], keys[1], keys[0]];
        let g2 = [keys[1], keys[0], keys[1]];
        p.goals = vec![g1, g2];
        // 키 기록이 하나도 없어도, 목표 트라이그램만 통과하면 열림.
        for _ in 0..TRI_MIN_SAMPLES {
            p.record_trigram(g1, fast());
        }
        assert_eq!(p.try_unlock(T), None);
        assert_eq!(p.goals_by_need(T)[0], g2); // 아직 기록 없는 g2가 먼저
        for _ in 0..TRI_MIN_SAMPLES - 1 {
            p.record_trigram(g2, fast());
        }
        assert_eq!(p.try_unlock(T), None);
        p.record_trigram(g2, fast());
        let next = p.next_locked();
        assert_eq!(p.try_unlock(T), next);
        assert!(p.goals.is_empty());
        // 목표가 아닌 트라이그램은 기록하지 않음.
        p.record_trigram(g1, fast());
        assert_eq!(p.trigram(g1).unwrap().count(), TRI_MIN_SAMPLES);
    }

    #[test]
    fn goals_by_need_puts_inaccurate_before_slow() {
        let mut p = profile();
        let (a, b, c) = (['a', 'b', 'c'], ['d', 'e', 'f'], ['g', 'h', 'i']);
        p.goals = vec![a, b, c];
        for i in 0..TRI_MIN_SAMPLES {
            p.record_trigram(a, fast());
            p.record_trigram(
                b,
                Sample {
                    ms: Some(1000),
                    clean: true,
                },
            ); // 느림
            p.record_trigram(
                c,
                Sample {
                    ms: Some(200),
                    clean: i % 2 == 0,
                },
            ); // 자주 틀림
        }
        assert_eq!(p.goals_by_need(T), vec![c, b, a]);
    }

    #[test]
    fn stage_new_keys() {
        let mut p = profile();
        assert_eq!(p.stage_new_keys(), p.unlocked_keys().to_vec());
        p.unlocked += 1;
        assert_eq!(p.stage_new_keys(), vec![p.order[p.unlocked - 1]]);
    }
}
