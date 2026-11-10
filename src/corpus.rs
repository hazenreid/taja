//! 글자 빈도와 트라이그램(세 글자 연속) 통계.
//!
//! 영어는 사용자가 만든 `corpus.bin`에서 읽고, 한글은 내장 단어 목록의 빈도로 계산합니다.
//!
//! `corpus.bin` 형식: `CorpusStats { letters, bigrams, trigrams }`를 bincode 1 기본 설정
//! (리틀엔디언, 고정 길이 정수)으로 직렬화한 것. 헤더 없이 맵 세 개가 차례로 들어 있습니다.
//!
//! ```text
//! [u64 개수] + (char, u64) × 개수                ← letters
//! [u64 개수] + (char, char, u64) × 개수          ← bigrams
//! [u64 개수] + (char, char, char, u64) × 개수    ← trigrams
//! ```
//! char는 UTF-8(1~4바이트), 횟수는 u64.

use std::collections::HashMap;

use crate::words::WordList;

/// 단어 경계(공백). 공백은 항상 칠 수 있는 키로 봅니다.
pub const SPACE: char = ' ';

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Ngrams {
    /// 글자 → 빈도.
    pub letters: HashMap<char, f64>,
    /// (세 글자, 빈도).
    pub trigrams: Vec<([char; 3], f64)>,
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.pos + n;
        let slice = self
            .bytes
            .get(self.pos..end)
            .ok_or_else(|| format!("file ends early at byte {}", self.pos))?;
        self.pos = end;
        Ok(slice)
    }

    fn u64(&mut self) -> Result<u64, String> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn char(&mut self) -> Result<char, String> {
        let at = self.pos;
        let first = *self
            .bytes
            .get(at)
            .ok_or_else(|| format!("file ends early at byte {at}"))?;
        let len = match first {
            0x00..=0x7F => 1,
            0xC0..=0xDF => 2,
            0xE0..=0xEF => 3,
            0xF0..=0xF7 => 4,
            _ => return Err(format!("invalid UTF-8 at byte {at}")),
        };
        let s = std::str::from_utf8(self.take(len)?).map_err(|_| format!("invalid UTF-8 at byte {at}"))?;
        Ok(s.chars().next().unwrap())
    }

    /// 맵 하나: 개수, 그다음 (키 글자 `n`개, 횟수) 반복.
    fn map<const N: usize>(&mut self) -> Result<Vec<([char; N], u64)>, String> {
        let count = self.u64()?;
        // 항목 하나는 최소 N + 8바이트이므로, 남은 크기로 개수가 말이 되는지 먼저 확인.
        let left = (self.bytes.len() - self.pos) as u64;
        if count > left / (N as u64 + 8) {
            return Err(format!("entry count {count} is too large for the file"));
        }
        let mut out = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let mut key = [SPACE; N];
            for k in key.iter_mut() {
                *k = self.char()?;
            }
            out.push((key, self.u64()?));
        }
        Ok(out)
    }
}

impl Ngrams {
    /// `corpus.bin` 내용을 읽습니다. 영어는 소문자로 바꾸고, 글자와 공백이 아닌 것은 버립니다.
    pub fn parse_corpus(bytes: &[u8]) -> Result<Self, String> {
        let mut r = Reader { bytes, pos: 0 };
        let letters = r.map::<1>()?;
        let _bigrams = r.map::<2>()?;
        let trigrams = r.map::<3>()?;
        if r.pos != bytes.len() {
            return Err(format!(
                "{} unexpected bytes after the trigrams",
                bytes.len() - r.pos
            ));
        }

        let norm = |c: char| {
            let c = c.to_ascii_lowercase();
            (c.is_ascii_lowercase() || c == SPACE).then_some(c)
        };
        let mut out = Ngrams::default();
        for ([c], n) in letters {
            if let Some(c) = norm(c) {
                *out.letters.entry(c).or_insert(0.0) += n as f64;
            }
        }
        let mut tri: HashMap<[char; 3], f64> = HashMap::new();
        for ([a, b, c], n) in trigrams {
            if let (Some(a), Some(b), Some(c)) = (norm(a), norm(b), norm(c)) {
                *tri.entry([a, b, c]).or_insert(0.0) += n as f64;
            }
        }
        out.trigrams = tri.into_iter().collect();
        out.trigrams
            .sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(out)
    }

    /// 단어 목록의 빈도(가중치)로 계산합니다. 단어 앞뒤에 공백을 붙여 경계도 셉니다.
    /// 한글은 두벌식 키 순서로 셉니다 (`학교` → ` gkrry `).
    pub fn from_words(words: &WordList) -> Self {
        let mut out = Ngrams::default();
        let mut tri: HashMap<[char; 3], f64> = HashMap::new();
        for w in &words.words {
            let seq: Vec<char> = std::iter::once(SPACE)
                .chain(w.keys.chars())
                .chain([SPACE])
                .collect();
            for &c in &seq[1..seq.len() - 1] {
                *out.letters.entry(c).or_insert(0.0) += w.weight;
            }
            for t in seq.windows(3) {
                *tri.entry([t[0], t[1], t[2]]).or_insert(0.0) += w.weight;
            }
        }
        out.trigrams = tri.into_iter().collect();
        out.trigrams
            .sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        out
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// 테스트용: `CorpusStats`를 bincode 1과 같은 형식으로 씁니다.
    pub fn encode(
        letters: &[(char, u64)],
        bigrams: &[([char; 2], u64)],
        trigrams: &[([char; 3], u64)],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let put_char = |out: &mut Vec<u8>, c: char| {
            let mut buf = [0; 4];
            out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        };
        out.extend_from_slice(&(letters.len() as u64).to_le_bytes());
        for &(c, n) in letters {
            put_char(&mut out, c);
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(&(bigrams.len() as u64).to_le_bytes());
        for &(k, n) in bigrams {
            k.iter().for_each(|&c| put_char(&mut out, c));
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(&(trigrams.len() as u64).to_le_bytes());
        for &(k, n) in trigrams {
            k.iter().for_each(|&c| put_char(&mut out, c));
            out.extend_from_slice(&n.to_le_bytes());
        }
        out
    }

    #[test]
    fn reads_bincode_layout() {
        let bytes = encode(
            &[('t', 9), ('h', 7), (' ', 20)],
            &[(['t', 'h'], 5)],
            &[([' ', 't', 'h'], 4), (['t', 'h', 'e'], 3)],
        );
        // 크기 = (8 + 3×9) + (8 + 1×10) + (8 + 2×11)
        assert_eq!(bytes.len(), 35 + 18 + 30);
        let n = Ngrams::parse_corpus(&bytes).unwrap();
        assert_eq!(n.letters[&'t'], 9.0);
        assert_eq!(n.letters[&' '], 20.0);
        assert_eq!(n.trigrams, vec![([' ', 't', 'h'], 4.0), (['t', 'h', 'e'], 3.0)]);
    }

    #[test]
    fn handles_multibyte_chars_and_drops_non_letters() {
        let bytes = encode(
            &[('é', 1), ('A', 2)],
            &[],
            &[(['é', 'a', 'b'], 5), (['A', 'b', 'c'], 6)],
        );
        let n = Ngrams::parse_corpus(&bytes).unwrap();
        assert_eq!(n.letters.get(&'a'), Some(&2.0));
        assert_eq!(n.trigrams, vec![(['a', 'b', 'c'], 6.0)]);
    }

    #[test]
    fn rejects_broken_files() {
        let good = encode(&[('a', 1)], &[], &[(['a', 'b', 'c'], 1)]);
        assert!(Ngrams::parse_corpus(&good[..good.len() - 1]).is_err());
        let mut extra = good.clone();
        extra.push(0);
        assert!(Ngrams::parse_corpus(&extra).is_err());
        // 개수가 터무니없이 큰 경우.
        assert!(Ngrams::parse_corpus(&u64::MAX.to_le_bytes()).is_err());
    }

    #[test]
    fn from_words_counts_word_boundaries() {
        let words = WordList::from_sources(crate::words::Lang::En, &[("the", 2.0)]);
        let n = Ngrams::from_words(&words);
        let get = |k: [char; 3]| n.trigrams.iter().find(|t| t.0 == k).map(|t| t.1);
        assert_eq!(get([' ', 't', 'h']), Some(2.0));
        assert_eq!(get(['t', 'h', 'e']), Some(2.0));
        assert_eq!(get(['h', 'e', ' ']), Some(2.0));
        assert_eq!(n.letters[&'t'], 2.0);
    }
}
