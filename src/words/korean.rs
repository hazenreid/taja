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
    for _ in 0..60 {
        let text = random_word(rng, &parts);
        if BLOCKED.iter().any(|b| text.contains(b)) {
            continue;
        }
        let keys = hangul::word_keys(&text)?;
        if focus.is_none_or(|f| keys.contains(f)) {
            return Some((text, keys));
        }
    }
    // 60번 안에 집중 키가 안 나오면, 집중 키로 음절을 직접 만듭니다.
    match focus {
        Some(f) => with_key(rng, &parts, f),
        None => {
            let text = random_word(rng, &parts);
            hangul::word_keys(&text).map(|keys| (text, keys))
        }
    }
}

/// 키 순서 `pattern`이 그대로 들어간 가짜 단어 (화면 글자, 누를 키). 앞뒤에 열린 키를 0~2개씩
/// 붙여 보면서 올바른 음절로 조합되는 것을 찾습니다. 못 찾으면 None.
pub fn generate_with_pattern(
    rng: &mut impl Rng,
    allowed: &[char],
    pattern: &str,
) -> Option<(String, String)> {
    if allowed.is_empty() || !pattern.chars().all(|c| allowed.contains(&c)) {
        return None;
    }
    let random_keys = |rng: &mut _| -> String {
        let n = rand::Rng::random_range(rng, 0..=2);
        (0..n)
            .map(|_| allowed[rand::Rng::random_range(rng, 0..allowed.len())])
            .collect()
    };
    // 받침이 흔한 것(없음, ㄱ ㄴ ㄹ ㅁ ㅂ ㅅ ㅆ ㅇ)만 있는 단어를 먼저 찾고, 없으면 처음 찾은 단어.
    let natural = |text: &str| {
        text.chars()
            .all(|c| hangul::jong_of(c).is_some_and(|j| j == 0 || COMMON_JONG.contains(&j)))
    };
    let mut fallback = None;
    for _ in 0..300 {
        let keys = format!("{}{pattern}{}", random_keys(rng), random_keys(rng));
        let Some(text) = hangul::compose_keys(&keys) else {
            continue;
        };
        if BLOCKED.iter().any(|b| text.contains(b)) {
            continue;
        }
        if natural(&text) {
            return Some((text, keys));
        }
        fallback.get_or_insert((text, keys));
    }
    fallback
}

/// 집중 키를 초성(자음) 또는 중성(모음)으로 쓰는 음절 하나.
fn with_key(rng: &mut impl Rng, p: &Parts, key: char) -> Option<(String, String)> {
    let one = key.to_string();
    let pick = |rng: &mut _, v: &[usize]| v[rand::Rng::random_range(rng, 0..v.len())];
    let syllable = if let Some(cho) = hangul::cho_with_keys(&one) {
        hangul::make_syllable(cho, pick(rng, &p.jung), 0)
    } else {
        let jung = hangul::jung_with_keys(&one)?;
        hangul::make_syllable(pick(rng, &p.cho), jung, 0)
    };
    let text = syllable.to_string();
    hangul::word_keys(&text).map(|keys| (text, keys))
}

fn random_word(rng: &mut impl Rng, p: &Parts) -> String {
    let n = pick_weighted(rng, &SYLLABLE_COUNTS).unwrap_or(2);
    (0..n)
        .map(|_| {
            let cho = p.cho[rng.random_range(0..p.cho.len())];
            // 겹모음(ㅘ ㅚ …)은 가짜 단어에서 어색해서 홑모음만 씀.
            let simple: Vec<usize> = p
                .jung
                .iter()
                .copied()
                .filter(|&j| hangul::is_simple_jung(j))
                .collect();
            let jungs = if simple.is_empty() { &p.jung } else { &simple };
            let jung = jungs[rng.random_range(0..jungs.len())];
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
    fn pattern_words_contain_the_pattern() {
        let mut rng = StdRng::seed_from_u64(3);
        let allowed: Vec<char> = "dkelgrhfs".chars().collect();
        // ㅏㄷㅏ(모음으로 시작), ㄹㄷㅏ(받침 + 초성), ㅎㅗㅏ(겹모음)
        for pattern in ["kek", "fek", "ghk"] {
            for _ in 0..100 {
                let (text, keys) = generate_with_pattern(&mut rng, &allowed, pattern).unwrap();
                assert!(keys.contains(pattern), "{text} {keys}");
                assert_eq!(hangul::word_keys(&text).as_deref(), Some(keys.as_str()));
                assert!(keys.chars().all(|k| allowed.contains(&k)), "{text} {keys}");
            }
        }
        // 받침이 흔하지 않은 음절(잏, 핳)보다 자연스러운 것 (이하)을 먼저 만듦.
        let allowed: Vec<char> = "dkegl".chars().collect();
        for _ in 0..100 {
            let (text, _) = generate_with_pattern(&mut rng, &allowed, "dlg").unwrap();
            assert!(!text.contains('잏') && !text.contains('핳'), "{text}");
        }
        // 잠긴 키가 든 패턴은 못 만듦.
        assert!(generate_with_pattern(&mut rng, &allowed, "kq").is_none());
    }

    #[test]
    fn needs_consonant_and_vowel() {
        let mut rng = StdRng::seed_from_u64(2);
        assert!(generate(&mut rng, &['k', 'j'], None).is_none());
        assert!(generate(&mut rng, &['r', 's'], None).is_none());
    }
}
