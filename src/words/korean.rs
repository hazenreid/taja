//! 한글 가짜 단어 생성기: 열린 키로 만들 수 있는 올바른 음절을 이어 붙입니다.

use rand::Rng;

use super::generator::pick_weighted;
use crate::hangul::{self, Parts};

/// 음절 수별 확률 (1, 2, 3음절).
const SYLLABLE_COUNTS: [(usize, f64); 3] = [(1, 1.0), (2, 3.0), (3, 2.0)];
/// 받침을 붙일 확률.
const JONG_RATE: f64 = 0.35;
/// 가짜 단어에 쓰는 받침 번호: ㄱ ㄴ ㄹ ㅁ ㅂ ㅅ ㅆ ㅇ (겹받침이나 드문 받침은 어색해서 뺌).
const COMMON_JONG: [usize; 8] = [1, 4, 8, 16, 17, 19, 20, 21];
/// 가짜 단어로 우연히 만들어지면 안 되는 글자 조합.
const BLOCKED: [&str; 8] = ["씨발", "시발", "개새", "새끼", "좆", "병신", "지랄", "섹스"];

/// (화면 글자, 누를 키). 자음이나 모음이 하나도 안 열렸으면 None.
pub fn generate(rng: &mut impl Rng, allowed: &[char], focus: Option<char>) -> Option<(String, String)> {
    let parts = hangul::parts(allowed);
    if parts.cho.is_empty() || parts.jung.is_empty() {
        return None;
    }
    let focus = focus.filter(|f| allowed.contains(f));
    let mut last = None;
    for _ in 0..60 {
        let text = random_word(rng, &parts);
        if BLOCKED.iter().any(|b| text.contains(b)) {
            continue;
        }
        let keys = hangul::word_keys(&text)?;
        if focus.is_none_or(|f| keys.contains(f)) {
            return Some((text, keys));
        }
        last = Some((text, keys));
    }
    last
}

fn random_word(rng: &mut impl Rng, p: &Parts) -> String {
    let n = pick_weighted(rng, &SYLLABLE_COUNTS).unwrap_or(2);
    (0..n)
        .map(|_| {
            let cho = p.cho[rng.random_range(0..p.cho.len())];
            let jung = p.jung[rng.random_range(0..p.jung.len())];
            let jongs: Vec<usize> = p
                .jong
                .iter()
                .copied()
                .filter(|j| COMMON_JONG.contains(j))
                .collect();
            let jong = if !jongs.is_empty() && rng.random_bool(JONG_RATE) {
                jongs[rng.random_range(0..jongs.len())]
            } else {
                0
            };
            hangul::make_syllable(cho, jung, jong)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn uses_only_allowed_keys() {
        let mut rng = StdRng::seed_from_u64(1);
        let allowed = ['f', 'j', 'd', 'k', 's', 'l'];
        for _ in 0..500 {
            let (text, keys) = generate(&mut rng, &allowed, Some('s')).unwrap();
            assert!(text.chars().all(hangul::is_syllable), "{text}");
            assert!(keys.chars().all(|k| allowed.contains(&k)), "{text} {keys}");
            assert!(keys.contains('s'), "{text} {keys}");
        }
    }

    #[test]
    fn needs_consonant_and_vowel() {
        let mut rng = StdRng::seed_from_u64(2);
        assert!(generate(&mut rng, &['k', 'j'], None).is_none());
        assert!(generate(&mut rng, &['r', 's'], None).is_none());
    }
}
