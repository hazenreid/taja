//! 키별 속도/정확도 기록과 언락 판정.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::layout::Layout;
use crate::layout::plan::Plan;

/// 키마다 최근 몇 번의 입력으로 평가할지.
pub const WINDOW: usize = 30;
/// 언락 판정에 필요한 최소 입력 횟수.
pub const MIN_SAMPLES: usize = 20;
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

    /// 목표까지 얼마나 왔는지 0.0 ~ 1.0. 입력 횟수가 부족하면 그만큼 깎습니다.
    pub fn progress(&self, t: Targets) -> f64 {
        let (Some(wpm), Some(acc)) = (self.wpm(), self.accuracy()) else {
            return 0.0;
        };
        let p = (wpm / t.wpm).min(acc / t.accuracy).min(1.0);
        let enough = (self.count() as f64 / MIN_SAMPLES as f64).min(1.0);
        p * enough
    }

    pub fn mastered(&self, t: Targets) -> bool {
        self.count() >= MIN_SAMPLES
            && self.wpm().is_some_and(|w| w >= t.wpm)
            && self.accuracy().is_some_and(|a| a >= t.accuracy)
    }
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
}

impl Profile {
    pub fn new(layout: &Layout, plan: &Plan) -> Self {
        Profile {
            layout_rows: layout.rows_as_strings(),
            order: plan.order.clone(),
            unlocked: plan.start_len,
            stats: vec![KeyStats::default(); plan.order.len()],
            history: Vec::new(),
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

    /// 열린 키 중 가장 약한 키. 동점이면 나중에 열린 키.
    pub fn focus(&self, t: Targets) -> Option<char> {
        self.unlocked_keys()
            .iter()
            .enumerate()
            .rev()
            .min_by(|a, b| {
                self.stats[a.0]
                    .progress(t)
                    .total_cmp(&self.stats[b.0].progress(t))
            })
            .map(|(_, &c)| c)
    }

    /// 열린 키가 모두 목표를 넘었으면 다음 키를 열고 그 키를 돌려줍니다.
    pub fn try_unlock(&mut self, t: Targets) -> Option<char> {
        let next = self.next_locked()?;
        if self.stats[..self.unlocked].iter().all(|s| s.mastered(t)) {
            self.unlocked += 1;
            Some(next)
        } else {
            None
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
    fn unlocks_only_when_all_keys_mastered() {
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
        assert_eq!(p.try_unlock(T), Some('e'));
        assert_eq!(p.focus(T), Some('e'));
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
        for _ in 0..WINDOW {
            p.record(
                'd',
                Sample {
                    ms: Some(1000),
                    clean: true,
                },
            ); // 12 WPM
        }
        assert_eq!(p.try_unlock(T), None);
        assert_eq!(p.focus(T), Some('d'));
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
}
