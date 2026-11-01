//! 단어 목록과 "열린 키로만 된 단어" 거르기.

pub mod generator;
pub mod line;

use generator::Bigram;

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

/// 단어 묶음별 가중치 (클수록 자주 나옴). 같은 단어는 먼저 나온 묶음의 가중치를 씁니다.
const COMMON_WEIGHT: f64 = 3.0;
const SCOWL_10_WEIGHT: f64 = 2.0;
const OTHER_WEIGHT: f64 = 1.0;
const SCOWL_20_WEIGHT: f64 = 0.7;

#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    /// 이 단어에 쓰인 글자의 비트마스크 (a = 비트 0).
    pub mask: u32,
    pub weight: f64,
}

pub struct WordList {
    pub words: Vec<Word>,
    pub bigram: Bigram,
}

pub fn letter_bit(ch: char) -> Option<u32> {
    ch.is_ascii_lowercase().then(|| 1 << (ch as u32 - 'a' as u32))
}

/// 글자들의 비트마스크. 영어 소문자가 아닌 글자는 무시합니다.
pub fn mask_of(chars: impl IntoIterator<Item = char>) -> u32 {
    chars.into_iter().filter_map(letter_bit).fold(0, |m, b| m | b)
}

impl WordList {
    pub fn english() -> Self {
        let mut sources = vec![(COMMON, COMMON_WEIGHT), (SCOWL_10, SCOWL_10_WEIGHT)];
        sources.extend(OTHERS.iter().map(|&t| (t, OTHER_WEIGHT)));
        sources.push((SCOWL_20, SCOWL_20_WEIGHT));
        Self::from_sources(&sources)
    }

    /// 각 원본 텍스트를 공백으로 나눠 읽습니다. 중복은 처음 나온 것만 남깁니다.
    pub fn from_sources(sources: &[(&str, f64)]) -> Self {
        let mut words: Vec<Word> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for &(text, weight) in sources {
            let tokens = text
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .flat_map(str::split_whitespace);
            for token in tokens {
                let w = token.to_ascii_lowercase();
                if w.len() < 2 || !w.chars().all(|c| c.is_ascii_lowercase()) {
                    continue;
                }
                if seen.insert(w.clone()) {
                    words.push(Word {
                        mask: mask_of(w.chars()),
                        text: w,
                        weight,
                    });
                }
            }
        }
        let bigram = Bigram::from_words(&words);
        WordList { words, bigram }
    }

    /// 열린 글자(`unlocked` 마스크)로만 칠 수 있는 단어.
    pub fn available(&self, unlocked: u32) -> Vec<&Word> {
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
    }

    #[test]
    fn dedupes_and_skips_comments() {
        let l = WordList::from_sources(&[("# x\nsad dad Sad x it's", 2.0), ("dad lad", 1.0)]);
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

    /// 한글 단어 목록 (아직 프로그램에 연결하지 않은 데이터).
    const KOREAN: [&str; 9] = [
        include_str!("../../assets/words/ko/basic.txt"),
        include_str!("../../assets/words/ko/short.txt"),
        include_str!("../../assets/words/ko/short2.txt"),
        include_str!("../../assets/words/ko/nouns.txt"),
        include_str!("../../assets/words/ko/daily.txt"),
        include_str!("../../assets/words/ko/themes.txt"),
        include_str!("../../assets/words/ko/verbs.txt"),
        include_str!("../../assets/words/ko/verbs2.txt"),
        include_str!("../../assets/words/ko/adjectives.txt"),
    ];

    #[test]
    fn korean_words_are_hangul_syllables() {
        let mut unique = std::collections::HashSet::new();
        for text in KOREAN {
            let tokens = text
                .lines()
                .filter(|l| !l.trim_start().starts_with('#'))
                .flat_map(str::split_whitespace);
            for w in tokens {
                assert!(
                    w.chars().all(|c| ('가'..='힣').contains(&c)),
                    "완성된 한글이 아님: {w}"
                );
                unique.insert(w);
            }
        }
        assert!(unique.len() >= 2000, "한글 단어 수: {}", unique.len());
    }
}
