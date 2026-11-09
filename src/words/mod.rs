//! 단어 목록과 "열린 키로만 된 단어" 거르기.
//!
//! 단어마다 화면에 보일 글자(`text`)와 실제로 누를 키(`keys`)를 따로 둡니다.
//! 영어는 둘이 같고, 한글은 `학교` → `gkrry`처럼 두벌식 키로 바꿉니다.

pub mod generator;
pub mod korean;
pub mod line;

use generator::Bigram;

use crate::hangul;

/// 자주 쓰는 기본 단어 (가중치 높음).
const COMMON: &str = include_str!("../../assets/words/common.txt");
/// 일반 단어 파일들.
const OTHERS: [&str; 5] = [
    include_str!("../../assets/words/early.txt"),
    include_str!("../../assets/words/extra.txt"),
    include_str!("../../assets/words/verbs.txt"),
    include_str!("../../assets/words/nouns.txt"),
    include_str!("../../assets/words/adjectives.txt"),
];

/// SCOWL 공개 단어 목록 (scripts/import_scowl.py 로 생성). 10단계가 더 흔한 단어.
const SCOWL_10: &str = include_str!("../../assets/words/scowl/level10.txt");
const SCOWL_20: &str = include_str!("../../assets/words/scowl/level20.txt");

/// 한글: 영화 자막 빈도순 목록 (scripts/import_ko_subtitles.py 로 생성, CC BY-SA 4.0).
const KO_SUBTITLES: &str = include_str!("../../assets/words/ko/subtitles/top.txt");
const KO_BASIC: &str = include_str!("../../assets/words/ko/basic.txt");
const KO_OTHERS: [&str; 8] = [
    include_str!("../../assets/words/ko/short.txt"),
    include_str!("../../assets/words/ko/short2.txt"),
    include_str!("../../assets/words/ko/nouns.txt"),
    include_str!("../../assets/words/ko/daily.txt"),
    include_str!("../../assets/words/ko/themes.txt"),
    include_str!("../../assets/words/ko/verbs.txt"),
    include_str!("../../assets/words/ko/verbs2.txt"),
    include_str!("../../assets/words/ko/adjectives.txt"),
];

/// 단어 묶음별 가중치 (클수록 자주 나옴). 같은 단어는 먼저 나온 묶음의 가중치를 씁니다.
const COMMON_WEIGHT: f64 = 3.0;
const SCOWL_10_WEIGHT: f64 = 2.0;
const OTHER_WEIGHT: f64 = 1.0;
const SCOWL_20_WEIGHT: f64 = 0.7;
/// 자막 목록은 한 줄에 15단어. 앞쪽(더 흔한) 줄일수록 가중치가 큼.
const KO_SUBTITLE_TIERS: [(usize, f64); 3] = [(130, 2.5), (330, 1.2), (usize::MAX, 0.6)];

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

    /// 모음 키. 시작 키에 하나도 없으면 진짜 단어가 안 나옵니다.
    pub fn vowel_keys(self) -> &'static [char] {
        match self {
            Lang::En => &['a', 'e', 'i', 'o', 'u'],
            Lang::Ko => &hangul::VOWEL_KEYS,
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
    /// 보조 사전 단어 (예: 맥의 /usr/share/dict/words). 내장 단어가 모자랄 때만 쓰고,
    /// 언락 순서 계산에는 쓰지 않습니다.
    pub extra: Vec<Word>,
}

/// 보조 사전 단어의 가중치 (내장 단어보다 낮게).
const EXTRA_WEIGHT: f64 = 0.3;
/// 맥과 대부분의 리눅스에 있는 영어 단어 목록.
pub const DEFAULT_DICT: &str = "/usr/share/dict/words";

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

/// 앞에서 `n`줄과 나머지로 나눕니다.
fn split_lines(text: &str, n: usize) -> (&str, &str) {
    match text.match_indices('\n').nth(n.saturating_sub(1)) {
        Some((i, _)) if n > 0 => text.split_at(i + 1),
        _ if n == 0 => ("", text),
        _ => (text, ""),
    }
}

impl WordList {
    pub fn for_lang(lang: Lang) -> Self {
        match lang {
            Lang::En => Self::english(),
            Lang::Ko => Self::korean(),
        }
    }

    pub fn english() -> Self {
        let mut sources = vec![(COMMON, COMMON_WEIGHT), (SCOWL_10, SCOWL_10_WEIGHT)];
        sources.extend(OTHERS.iter().map(|&t| (t, OTHER_WEIGHT)));
        sources.push((SCOWL_20, SCOWL_20_WEIGHT));
        Self::from_sources(Lang::En, &sources)
    }

    pub fn korean() -> Self {
        let mut sources = vec![(KO_BASIC, COMMON_WEIGHT)];
        let mut rest = KO_SUBTITLES;
        let mut done = 0;
        for (until, weight) in KO_SUBTITLE_TIERS {
            let (head, tail) = split_lines(rest, until.saturating_sub(done));
            sources.push((head, weight));
            done = until;
            rest = tail;
        }
        sources.extend(KO_OTHERS.iter().map(|&t| (t, OTHER_WEIGHT)));
        Self::from_sources(Lang::Ko, &sources)
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
        WordList {
            lang,
            words,
            bigram,
            extra: Vec::new(),
        }
    }

    /// 보조 사전을 더합니다 (한 줄에 한 단어). 모두 소문자로 바꾸고, 영어 글자로만 된
    /// 2글자 이상 단어 중 내장 목록에 없는 것만 씁니다. 더한 단어 수를 돌려줍니다.
    pub fn add_extra(&mut self, text: &str) -> usize {
        let mut seen: std::collections::HashSet<String> = self
            .words
            .iter()
            .chain(&self.extra)
            .map(|w| w.text.clone())
            .collect();
        let before = self.extra.len();
        for line in text.lines() {
            let Some(keys) = self.lang.keys_of(line.trim()) else {
                continue;
            };
            if seen.insert(keys.clone()) {
                self.extra.push(Word {
                    mask: mask_of(keys.chars()),
                    text: keys.clone(),
                    keys,
                    weight: EXTRA_WEIGHT,
                });
            }
        }
        self.extra.len() - before
    }

    /// 보조 사전에서 열린 키로만 칠 수 있는 단어.
    pub fn available_extra(&self, unlocked: u64) -> Vec<&Word> {
        self.extra.iter().filter(|w| w.mask & !unlocked == 0).collect()
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
    fn builtin_list_is_reasonable() {
        let l = WordList::english();
        assert!(l.words.len() > 10_000, "단어 수: {}", l.words.len());
    }

    #[test]
    fn extra_dictionary_is_lowercased_and_deduped() {
        let mut l = WordList::english();
        let n = l.add_extra("Paris\nthe\nJak\njak\nfoo-bar\nx\n  Dajl  \n");
        // the는 내장 목록에 있음, foo-bar와 x는 못 씀, Jak/jak는 하나로.
        assert_eq!(n, 3);
        let texts: Vec<_> = l.extra.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texts, ["paris", "jak", "dajl"]);
        // 보조 사전은 기본 목록(언락 순서 계산용)에 섞이지 않음.
        assert!(l.words.iter().all(|w| w.text != "jak"));
    }

    #[test]
    fn splits_lines() {
        assert_eq!(split_lines("a\nb\nc\n", 2), ("a\nb\n", "c\n"));
        assert_eq!(split_lines("a\nb\n", 5), ("a\nb\n", ""));
        assert_eq!(split_lines("a\n", 0), ("", "a\n"));
    }

    #[test]
    fn korean_words_are_hangul_syllables() {
        let sources = std::iter::once(KO_SUBTITLES).chain([KO_BASIC]).chain(KO_OTHERS);
        for text in sources {
            let tokens = text
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .flat_map(str::split_whitespace);
            for w in tokens {
                assert!(w.chars().all(hangul::is_syllable), "완성된 한글이 아님: {w}");
            }
        }
        let l = WordList::korean();
        assert!(l.words.len() >= 10_000, "한글 단어 수: {}", l.words.len());
        let hakgyo = l.words.iter().find(|w| w.text == "학교").unwrap();
        assert_eq!(hakgyo.keys, "gkrry");
        // 자막 목록 맨 앞 단어가 가장 큰 가중치.
        assert_eq!(l.words.iter().find(|w| w.text == "내가").unwrap().weight, 2.5);
    }
}
