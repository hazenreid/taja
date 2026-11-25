//! 영어처럼 보이는 가짜 단어 생성기.
//!
//! 단어 목록에서 "글자 다음에 어떤 글자가 오는지"(bigram) 빈도를 세고,
//! 열린 글자만 써서 그 빈도대로 글자를 이어 붙입니다.

use rand::Rng;

use super::Word;

/// 단어 시작/끝을 나타내는 인덱스.
const EDGE: usize = 26;
/// 한 번도 안 나온 연결도 아주 가끔은 나오게 하는 값 (막다른 길 방지).
const SMOOTHING: f64 = 0.02;

pub struct Bigram {
    /// prob[앞 글자][다음 글자], 각 행의 합은 1.
    prob: [[f64; 27]; 27],
}

fn index(ch: char) -> usize {
    (ch as u8 - b'a') as usize
}

impl Bigram {
    pub fn from_words(words: &[Word]) -> Self {
        let mut prob = [[0.0; 27]; 27];
        for w in words {
            let mut prev = EDGE;
            for ch in w.text.chars() {
                let i = index(ch);
                prob[prev][i] += w.weight;
                prev = i;
            }
            prob[prev][EDGE] += w.weight;
        }
        for row in prob.iter_mut() {
            let sum: f64 = row.iter().sum();
            if sum > 0.0 {
                row.iter_mut().for_each(|p| *p /= sum);
            }
        }
        Bigram { prob }
    }

    /// 허용된 글자로만 단어 하나를 만듭니다. `allowed`는 영어 소문자여야 합니다.
    pub fn generate_one(&self, rng: &mut impl Rng, allowed: &[char], min: usize, max: usize) -> String {
        let mut word: Vec<char> = Vec::new();
        let mut prev = EDGE;
        while word.len() < max {
            let n = word.len();
            // 같은 글자 3번 연속(aaa)은 막습니다.
            let banned = (n >= 2 && word[n - 1] == word[n - 2]).then(|| word[n - 1]);
            let mut options: Vec<(Option<char>, f64)> = allowed
                .iter()
                .filter(|&&c| Some(c) != banned)
                .map(|&c| (Some(c), self.prob[prev][index(c)] + SMOOTHING))
                .collect();
            if n >= min {
                options.push((None, self.prob[prev][EDGE] + SMOOTHING));
            }
            match pick_weighted(rng, &options) {
                Some(Some(c)) => {
                    word.push(c);
                    prev = index(c);
                }
                _ => break,
            }
        }
        word.into_iter().collect()
    }

    /// 그럴듯한 단어를 만듭니다. `focus`가 있으면 그 글자가 꼭 들어갑니다.
    pub fn generate_with(
        &self,
        rng: &mut impl Rng,
        allowed: &[char],
        focus: Option<char>,
        min: usize,
        max: usize,
    ) -> String {
        let focus = focus.filter(|f| allowed.contains(f));
        let need_vowel = allowed.iter().any(|c| VOWELS.contains(c));
        let mut fallback = None;
        for _ in 0..60 {
            let w = self.generate_one(rng, allowed, min, max);
            if !looks_ok(&w, min, need_vowel) {
                continue;
            }
            match focus {
                Some(f) if !w.contains(f) => fallback = Some(w),
                _ => return w,
            }
        }
        // 집중 키가 안 나오면 그럴듯한 단어의 한 자리를 집중 키로 바꿉니다.
        let base = fallback.unwrap_or_else(|| self.generate_one(rng, allowed, min, max));
        let Some(f) = focus else { return base };
        let mut chars: Vec<char> = base.chars().collect();
        if chars.is_empty() {
            return f.to_string();
        }
        let i = rng.random_range(0..chars.len());
        chars[i] = f;
        let w: String = chars.into_iter().collect();
        if is_blocked(&w) {
            f.to_string().repeat(min.max(1))
        } else {
            w
        }
    }
}

const VOWELS: [char; 6] = ['a', 'e', 'i', 'o', 'u', 'y'];

/// 가짜 단어로 우연히 만들어지면 안 되는 단어 (정확히 같으면 버림).
const BLOCKED_WORDS: [&str; 12] = [
    "ass", "tit", "tits", "fag", "sex", "cum", "hell", "damn", "kill", "nazi", "anal", "anus",
];
/// 이 글자들이 들어 있으면 버림.
const BLOCKED_PARTS: [&str; 10] = [
    "fuck", "shit", "cunt", "nigg", "slut", "cock", "dick", "piss", "rape", "porn",
];

fn is_blocked(w: &str) -> bool {
    BLOCKED_WORDS.contains(&w) || BLOCKED_PARTS.iter().any(|p| w.contains(p))
}

/// 너무 짧거나, 모음이 없거나, 첫 글자가 겹치거나(jja), 자음이 4개 이상 이어지거나,
/// 부적절한 단어면 버립니다.
fn looks_ok(w: &str, min: usize, need_vowel: bool) -> bool {
    if is_blocked(w) {
        return false;
    }
    let chars: Vec<char> = w.chars().collect();
    if chars.len() < min {
        return false;
    }
    if need_vowel && !chars.iter().any(|c| VOWELS.contains(c)) {
        return false;
    }
    if chars.len() >= 2 && chars[0] == chars[1] {
        return false;
    }
    !need_vowel || chars.windows(4).all(|win| win.iter().any(|c| VOWELS.contains(c)))
}

pub fn pick_weighted<T: Clone>(rng: &mut impl Rng, options: &[(T, f64)]) -> Option<T> {
    let total: f64 = options.iter().map(|o| o.1).sum();
    if options.is_empty() || total <= 0.0 {
        return None;
    }
    let mut x = rng.random_range(0.0..total);
    for (item, w) in options {
        if x < *w {
            return Some(item.clone());
        }
        x -= w;
    }
    options.last().map(|o| o.0.clone())
}

#[cfg(test)]
mod tests {
    use super::super::WordList;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn uses_only_allowed_letters_and_focus() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(1);
        let allowed: Vec<char> = "asdfjkle".chars().collect();
        for _ in 0..500 {
            let w = list.bigram.generate_with(&mut rng, &allowed, Some('e'), 3, 6);
            assert!((1..=6).contains(&w.len()), "{w}");
            assert!(w.contains('e'), "{w}");
            assert!(w.chars().all(|c| allowed.contains(&c)), "{w}");
        }
    }

    #[test]
    fn never_generates_blocked_words() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(3);
        let allowed: Vec<char> = "fjdksla".chars().collect();
        for _ in 0..5000 {
            let w = list.bigram.generate_with(&mut rng, &allowed, Some('s'), 3, 6);
            assert!(!super::is_blocked(&w), "{w}");
        }
    }

    #[test]
    fn no_triple_letters() {
        let list = WordList::english();
        let mut rng = StdRng::seed_from_u64(2);
        let allowed = ['s', 'l'];
        for _ in 0..500 {
            let w: Vec<char> = list
                .bigram
                .generate_one(&mut rng, &allowed, 3, 7)
                .chars()
                .collect();
            assert!(w.windows(3).all(|t| !(t[0] == t[1] && t[1] == t[2])), "{w:?}");
        }
    }
}
