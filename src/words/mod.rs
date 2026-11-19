//! 단어 목록과 "열린 키로만 된 단어" 거르기.
//!
//! 단어마다 화면에 보일 글자(`text`)와 실제로 누를 키(`keys`)를 따로 둡니다.
//! 영어는 둘이 같고, 한글은 `학교` → `gkrry`처럼 두벌식 키로 바꿉니다.

pub mod generator;
pub mod korean;
pub mod line;

use generator::Bigram;

use crate::hangul;

/// 영어: SCOWL 공개 단어 목록 10단계 = 가장 흔한 단어 (scripts/import_scowl.py 로 생성).
const SCOWL: [(&str, f64); 1] = [(include_str!("../../assets/words/scowl/level10.txt"), 1.0)];
/// 한글: 국립국어원 「한국어 학습용 어휘 목록」(2003, 공공누리 제1유형) A·B·C등급
/// (scripts/import_ko_learner.py 로 생성). A가 가장 기초 단어라 더 자주 나옴.
const KO_LEARNER: [(&str, f64); 3] = [
    (include_str!("../../assets/words/ko/learner/a.txt"), 3.0),
    (include_str!("../../assets/words/ko/learner/b.txt"), 1.5),
    (include_str!("../../assets/words/ko/learner/c.txt"), 1.0),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ko,
}

impl Lang {
    /// 단어를 누를 키로 바꿉니다. 연습에 못 쓰는 단어면 None.
    pub fn keys_of(self, word: &str) -> Option<String> {
        match self {
            Lang::En => {
                let w = word.to_ascii_lowercase();
                (w.len() >= 2 && w.chars().all(|c| c.is_ascii_lowercase())).then_some(w)
            }
            Lang::Ko => hangul::word_keys(word),
        }
    }

    /// Shift를 눌러야 하는 키 (레이아웃의 소문자 키 + Shift).
    pub fn shift_keys(self) -> &'static [char] {
        match self {
            Lang::En => &[],
            Lang::Ko => &hangul::SHIFT_KEYS,
        }
    }

    /// 화면에 보여줄 키 이름.
    pub fn key_label(self, key: char) -> String {
        match self {
            Lang::En => key.to_string(),
            Lang::Ko => hangul::key_jamo(key).map_or_else(|| key.to_string(), |j| j.to_string()),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ko => "Korean",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Word {
    /// 화면에 보일 글자.
    pub text: String,
    /// 누를 키.
    pub keys: String,
    /// 이 단어에 쓰인 키의 비트마스크 ([`key_bit`]).
    pub mask: u64,
    pub weight: f64,
}

impl Word {
    pub fn uses(&self, key: char) -> bool {
        self.keys.contains(key)
    }
}

pub struct WordList {
    pub lang: Lang,
    pub words: Vec<Word>,
    pub bigram: Bigram,
}

/// 키 하나의 비트. 소문자 a~z는 0~25, 대문자(Shift) A~Z는 26~51.
pub fn key_bit(ch: char) -> Option<u64> {
    if ch.is_ascii_lowercase() {
        Some(1 << (ch as u64 - 'a' as u64))
    } else if ch.is_ascii_uppercase() {
        Some(1 << (26 + ch as u64 - 'A' as u64))
    } else {
        None
    }
}

/// 키들의 비트마스크. 영어 글자가 아닌 것은 무시합니다.
pub fn mask_of(chars: impl IntoIterator<Item = char>) -> u64 {
    chars.into_iter().filter_map(key_bit).fold(0, |m, b| m | b)
}

/// 화면에서 차지하는 칸 수 (한글은 2칸).
pub fn display_width(text: &str) -> usize {
    text.chars().map(char_width).sum()
}

pub fn char_width(c: char) -> usize {
    if hangul::is_hangul_input(c) { 2 } else { 1 }
}

impl WordList {
    /// 영어 단어 목록 (내장 SCOWL 10단계, 가장 흔한 단어 약 4천 개).
    pub fn english() -> Self {
        Self::from_sources(Lang::En, &SCOWL)
    }

    /// 한글 단어 목록 (내장 한국어 학습용 어휘 A·B·C등급, 약 5,500개).
    pub fn korean() -> Self {
        Self::from_sources(Lang::Ko, &KO_LEARNER)
    }

    /// 각 원본 텍스트를 공백으로 나눠 읽습니다. 중복은 처음 나온 것만 남깁니다.
    pub fn from_sources(lang: Lang, sources: &[(&str, f64)]) -> Self {
        let mut words: Vec<Word> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &(text, weight) in sources {
            let tokens = text
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .flat_map(str::split_whitespace);
            for token in tokens {
                let Some(keys) = lang.keys_of(token) else {
                    continue;
                };
                let text = match lang {
                    Lang::En => keys.clone(),
                    Lang::Ko => token.to_string(),
                };
                if seen.insert(text.clone()) {
                    words.push(Word {
                        mask: mask_of(keys.chars()),
                        text,
                        keys,
                        weight,
                    });
                }
            }
        }
        let bigram = match lang {
            Lang::En => Bigram::from_words(&words),
            Lang::Ko => Bigram::from_words(&[]),
        };
        WordList { lang, words, bigram }
    }

    /// 열린 키(`unlocked` 마스크)로만 칠 수 있는 단어.
    pub fn available(&self, unlocked: u64) -> Vec<&Word> {
        self.words.iter().filter(|w| w.mask & !unlocked == 0).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks() {
        assert_eq!(mask_of("ab".chars()), 0b11);
        assert_eq!(mask_of("a;".chars()), 0b1);
        assert_eq!(key_bit('A'), Some(1 << 26));
    }

    #[test]
    fn dedupes_and_skips_comments() {
        let l = WordList::from_sources(Lang::En, &[("# x\nsad dad Sad x it's", 2.0), ("dad lad", 1.0)]);
        let texts: Vec<_> = l.words.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["sad", "dad", "lad"]);
        assert_eq!(l.words[1].weight, 2.0);
    }

    #[test]
    fn filters_by_unlocked_keys() {
        let l = WordList::english();
        let mask = mask_of("asdfjkl".chars());
        let avail = l.available(mask);
        assert!(!avail.is_empty());
        for w in avail {
            assert!(w.text.chars().all(|c| "asdfjkl".contains(c)), "{}", w.text);
        }
    }

    #[test]
    fn builtin_english_is_scowl() {
        let l = WordList::english();
        assert!(l.words.len() > 3_000, "단어 수: {}", l.words.len());
        assert!(l.words.iter().any(|w| w.text == "the"));
        assert!(
            l.words
                .iter()
                .all(|w| w.text.len() >= 2 && w.text.chars().all(|c| c.is_ascii_lowercase()))
        );
    }

    #[test]
    fn korean_words_are_hangul_syllables() {
        for (text, _) in KO_LEARNER {
            let tokens = text
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .flat_map(str::split_whitespace);
            for w in tokens {
                assert!(w.chars().all(hangul::is_syllable), "완성된 한글이 아님: {w}");
            }
        }
        let l = WordList::korean();
        assert!(l.words.len() > 5_000, "한글 단어 수: {}", l.words.len());
        let hakgyo = l.words.iter().find(|w| w.text == "학교").unwrap();
        assert_eq!(hakgyo.keys, "gkrry");
        // A등급(가장 기초) 단어가 가장 큰 가중치.
        assert_eq!(hakgyo.weight, 3.0);
    }
}
