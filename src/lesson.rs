//! 한 줄 연습. 틀린 키를 누르면 맞는 키를 칠 때까지 커서가 멈춥니다.

use std::time::{Duration, Instant};

use crate::progress::Sample;

/// 이보다 오래 쉬고 친 키는 속도 측정에서 뺍니다.
const PAUSE: Duration = Duration::from_millis(2000);

pub struct Lesson {
    pub text: Vec<char>,
    pub pos: usize,
    /// 자리마다 틀린 적이 있는지.
    pub had_error: Vec<bool>,
    /// 자리마다 맞게 친 시각.
    hits: Vec<Option<Instant>>,
    started: Option<Instant>,
    last_hit: Option<Instant>,
    finished: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// 맞게 쳤음. `ch`는 친 글자, `prev`는 바로 앞 글자 (줄 첫 글자면 공백 = 단어 시작).
    Correct {
        ch: char,
        prev: char,
        sample: Sample,
    },
    Wrong,
    /// 이미 끝난 줄.
    Ignored,
}

impl Lesson {
    pub fn new(text: &str) -> Self {
        let text: Vec<char> = text.chars().collect();
        let n = text.len();
        Lesson {
            text,
            pos: 0,
            had_error: vec![false; n],
            hits: vec![None; n],
            started: None,
            last_hit: None,
            finished: None,
        }
    }

    pub fn is_done(&self) -> bool {
        self.pos >= self.text.len()
    }

    /// 지금 쳐야 하는 글자.
    pub fn current(&self) -> Option<char> {
        self.text.get(self.pos).copied()
    }

    /// 지금 자리에서 틀린 키를 눌러 멈춰 있는지.
    pub fn is_blocked(&self) -> bool {
        !self.is_done() && self.had_error[self.pos]
    }

    pub fn type_char(&mut self, c: char, now: Instant) -> Outcome {
        let Some(target) = self.current() else {
            return Outcome::Ignored;
        };
        self.started.get_or_insert(now);
        if c != target {
            self.had_error[self.pos] = true;
            return Outcome::Wrong;
        }
        let ms = self
            .last_hit
            .map(|t| now.saturating_duration_since(t))
            .filter(|d| *d <= PAUSE)
            .map(|d| d.as_millis() as u32);
        let sample = Sample {
            ms,
            clean: !self.had_error[self.pos],
        };
        let prev = if self.pos == 0 {
            ' '
        } else {
            self.text[self.pos - 1]
        };
        self.last_hit = Some(now);
        self.hits[self.pos] = Some(now);
        self.pos += 1;
        if self.is_done() {
            self.finished = Some(now);
        }
        Outcome::Correct {
            ch: target,
            prev,
            sample,
        }
    }

    /// `end` 자리에서 끝나는 트라이그램(띄어쓰기가 없는 세 키)과 그 기록.
    /// - 속도: 세 키를 치는 데 걸린 시간의 키당 평균 (둘 중 한 간격이라도 너무 길면 None)
    /// - 정확도: 세 자리 모두 한 번에 맞혔는지
    pub fn trigram_at(&self, end: usize) -> Option<([char; 3], Sample)> {
        if end < 2 || end >= self.pos {
            return None;
        }
        let tri = [self.text[end - 2], self.text[end - 1], self.text[end]];
        if tri.contains(&' ') {
            return None;
        }
        let (a, b, c) = (self.hits[end - 2]?, self.hits[end - 1]?, self.hits[end]?);
        let (d1, d2) = (b.saturating_duration_since(a), c.saturating_duration_since(b));
        let ms = (d1 <= PAUSE && d2 <= PAUSE).then(|| ((d1 + d2).as_millis() / 2) as u32);
        let clean = !self.had_error[end - 2..=end].iter().any(|&e| e);
        Some((tri, Sample { ms, clean }))
    }

    /// 첫 키부터 지금(또는 끝난 시점)까지의 WPM.
    pub fn wpm(&self, now: Instant) -> Option<f64> {
        let start = self.started?;
        let end = self.finished.unwrap_or(now);
        let secs = end.saturating_duration_since(start).as_secs_f64();
        // 첫 글자는 시작 시점이라 간격이 없으므로 하나 뺍니다.
        let typed = self.pos.saturating_sub(1);
        (secs > 0.5 && typed > 0).then(|| typed as f64 / 5.0 / (secs / 60.0))
    }

    /// 친 자리 중 한 번에 맞힌 비율.
    pub fn accuracy(&self) -> Option<f64> {
        let typed = self.pos + self.is_blocked() as usize;
        if typed == 0 {
            return None;
        }
        let errors = self.had_error[..typed].iter().filter(|&&e| e).count();
        Some(1.0 - errors as f64 / typed as f64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_key_blocks_until_correct() {
        let t0 = Instant::now();
        let mut l = Lesson::new("ab");
        assert!(matches!(l.type_char('a', t0), Outcome::Correct { ch: 'a', .. }));
        assert_eq!(l.type_char('x', t0 + Duration::from_millis(100)), Outcome::Wrong);
        assert!(l.is_blocked());
        assert_eq!(l.pos, 1);
        let out = l.type_char('b', t0 + Duration::from_millis(300));
        assert_eq!(
            out,
            Outcome::Correct {
                ch: 'b',
                prev: 'a',
                sample: Sample {
                    ms: Some(300),
                    clean: false
                }
            }
        );
        assert!(l.is_done());
        assert_eq!(l.type_char('b', t0), Outcome::Ignored);
        assert_eq!(l.accuracy(), Some(0.5));
    }

    #[test]
    fn first_key_and_long_pause_have_no_time() {
        let t0 = Instant::now();
        let mut l = Lesson::new("abc");
        let Outcome::Correct { sample, .. } = l.type_char('a', t0) else {
            panic!()
        };
        assert_eq!(
            sample,
            Sample {
                ms: None,
                clean: true
            }
        );
        let Outcome::Correct { sample, .. } = l.type_char('b', t0 + Duration::from_secs(5)) else {
            panic!()
        };
        assert_eq!(sample.ms, None);
    }

    #[test]
    fn wpm_of_line() {
        let t0 = Instant::now();
        let mut l = Lesson::new("aaaaaa");
        for i in 0..6 {
            l.type_char('a', t0 + Duration::from_millis(240 * i));
        }
        // 240ms 간격 = 50 WPM
        let w = l.wpm(t0 + Duration::from_secs(100)).unwrap();
        assert!((w - 50.0).abs() < 0.01, "{w}");
    }

    #[test]
    fn trigram_samples() {
        let t0 = Instant::now();
        let mut l = Lesson::new("the at");
        let ms = |m| t0 + Duration::from_millis(m);
        l.type_char('t', ms(0));
        l.type_char('h', ms(200));
        assert_eq!(l.trigram_at(1), None); // 아직 두 키
        l.type_char('x', ms(300)); // e에서 한 번 틀림
        l.type_char('e', ms(500));
        // t→h 200ms, h→e 300ms → 키당 평균 250ms, e를 틀렸으니 clean 아님.
        assert_eq!(
            l.trigram_at(2),
            Some((
                ['t', 'h', 'e'],
                Sample {
                    ms: Some(250),
                    clean: false
                }
            ))
        );
        l.type_char(' ', ms(600));
        l.type_char('a', ms(700));
        // 띄어쓰기가 든 세 키는 트라이그램이 아님.
        assert_eq!(l.trigram_at(4), None);
        assert_eq!(l.trigram_at(5), None); // 아직 안 친 자리
    }
}
