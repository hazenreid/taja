//! 한글 음절 ↔ 두벌식 키.
//!
//! 자판은 영어 레이아웃의 글자를 그대로 따라갑니다: `q`는 어느 자리에 있든 ㅂ,
//! Shift+`q`(= `Q`)는 ㅃ입니다.

const BASE: u32 = 0xAC00;
const LAST: u32 = 0xD7A3;
const JUNG_COUNT: u32 = 21;
const JONG_COUNT: u32 = 28;

/// 초성 19개를 치는 키.
const CHO_KEYS: [&str; 19] = [
    "r", "R", "s", "e", "E", "f", "a", "q", "Q", "t", "T", "d", "w", "W", "c", "z", "x", "v", "g",
];
/// 중성 21개를 치는 키 (겹모음은 키 두 개).
const JUNG_KEYS: [&str; 21] = [
    "k", "o", "i", "O", "j", "p", "u", "P", "h", "hk", "ho", "hl", "y", "n", "nj", "np", "nl", "b", "m",
    "ml", "l",
];
/// 종성 28개를 치는 키 (0번은 받침 없음, 겹받침은 키 두 개).
const JONG_KEYS: [&str; 28] = [
    "", "r", "R", "rt", "s", "sw", "sg", "e", "f", "fr", "fa", "fq", "ft", "fx", "fv", "fg", "a", "q", "qt",
    "t", "T", "d", "w", "c", "z", "x", "v", "g",
];
/// 초성을 화면에 보여줄 때 쓰는 호환 자모.
const CHO_JAMO: [char; 19] = [
    'ㄱ', 'ㄲ', 'ㄴ', 'ㄷ', 'ㄸ', 'ㄹ', 'ㅁ', 'ㅂ', 'ㅃ', 'ㅅ', 'ㅆ', 'ㅇ', 'ㅈ', 'ㅉ', 'ㅊ', 'ㅋ', 'ㅌ',
    'ㅍ', 'ㅎ',
];

/// 키 하나가 나타내는 자모 (두벌식 표준).
pub fn key_jamo(key: char) -> Option<char> {
    Some(match key {
        'q' => 'ㅂ',
        'w' => 'ㅈ',
        'e' => 'ㄷ',
        'r' => 'ㄱ',
        't' => 'ㅅ',
        'y' => 'ㅛ',
        'u' => 'ㅕ',
        'i' => 'ㅑ',
        'o' => 'ㅐ',
        'p' => 'ㅔ',
        'a' => 'ㅁ',
        's' => 'ㄴ',
        'd' => 'ㅇ',
        'f' => 'ㄹ',
        'g' => 'ㅎ',
        'h' => 'ㅗ',
        'j' => 'ㅓ',
        'k' => 'ㅏ',
        'l' => 'ㅣ',
        'z' => 'ㅋ',
        'x' => 'ㅌ',
        'c' => 'ㅊ',
        'v' => 'ㅍ',
        'b' => 'ㅠ',
        'n' => 'ㅜ',
        'm' => 'ㅡ',
        'Q' => 'ㅃ',
        'W' => 'ㅉ',
        'E' => 'ㄸ',
        'R' => 'ㄲ',
        'T' => 'ㅆ',
        'O' => 'ㅒ',
        'P' => 'ㅖ',
        _ => return None,
    })
}

/// Shift를 눌러야 하는 키들.
pub const SHIFT_KEYS: [char; 7] = ['Q', 'W', 'E', 'R', 'T', 'O', 'P'];

pub fn is_syllable(c: char) -> bool {
    (BASE..=LAST).contains(&(c as u32))
}

/// 한글 입력기가 켜진 상태에서 들어오는 글자인지 (음절 또는 호환 자모).
pub fn is_hangul_input(c: char) -> bool {
    is_syllable(c) || ('ㄱ'..='ㅣ').contains(&c)
}

/// 음절을 (초성, 중성, 종성) 번호로.
fn split(c: char) -> Option<(usize, usize, usize)> {
    if !is_syllable(c) {
        return None;
    }
    let i = c as u32 - BASE;
    let cho = i / (JUNG_COUNT * JONG_COUNT);
    let jung = (i / JONG_COUNT) % JUNG_COUNT;
    let jong = i % JONG_COUNT;
    Some((cho as usize, jung as usize, jong as usize))
}

fn compose(cho: usize, jung: usize, jong: usize) -> char {
    let i = (cho as u32 * JUNG_COUNT + jung as u32) * JONG_COUNT + jong as u32;
    char::from_u32(BASE + i).expect("valid syllable index")
}

/// 음절 하나를 치는 키 순서.
pub fn syllable_keys(c: char) -> Option<String> {
    let (cho, jung, jong) = split(c)?;
    Some(format!("{}{}{}", CHO_KEYS[cho], JUNG_KEYS[jung], JONG_KEYS[jong]))
}

/// 단어 전체를 치는 키 순서. 완성된 한글이 아닌 글자가 있으면 None.
pub fn word_keys(word: &str) -> Option<String> {
    word.chars().map(syllable_keys).collect()
}

/// 음절의 키 중 앞의 `typed`개만 쳤을 때 화면에 보일 글자.
/// 예: '학'에서 2개 → '하', '과'에서 2개 → '고'.
pub fn partial(c: char, typed: usize) -> Option<char> {
    let (cho, jung, jong) = split(c)?;
    let jung_len = JUNG_KEYS[jung].len();
    if typed == 0 {
        return None;
    }
    if typed == 1 {
        return Some(CHO_JAMO[cho]);
    }
    if typed <= 1 + jung_len {
        let keys = &JUNG_KEYS[jung][..typed - 1];
        let j = JUNG_KEYS.iter().position(|k| *k == keys)?;
        return Some(compose(cho, j, 0));
    }
    let keys = &JONG_KEYS[jong][..(typed - 1 - jung_len).min(JONG_KEYS[jong].len())];
    let f = JONG_KEYS.iter().position(|k| *k == keys)?;
    Some(compose(cho, jung, f))
}

/// 열린 키로 만들 수 있는 초성, 중성, 종성 번호.
pub struct Parts {
    pub cho: Vec<usize>,
    pub jung: Vec<usize>,
    /// 0(받침 없음)은 항상 포함.
    pub jong: Vec<usize>,
}

pub fn parts(allowed: &[char]) -> Parts {
    let ok = |keys: &str| keys.chars().all(|k| allowed.contains(&k));
    Parts {
        cho: (0..CHO_KEYS.len()).filter(|&i| ok(CHO_KEYS[i])).collect(),
        jung: (0..JUNG_KEYS.len()).filter(|&i| ok(JUNG_KEYS[i])).collect(),
        jong: (0..JONG_KEYS.len()).filter(|&i| ok(JONG_KEYS[i])).collect(),
    }
}

/// 이 키(들)로 치는 초성 번호.
pub fn cho_with_keys(keys: &str) -> Option<usize> {
    CHO_KEYS.iter().position(|k| *k == keys)
}

/// 키 하나로 치는 모음(겹모음이 아닌 것)인지.
pub fn is_simple_jung(jung: usize) -> bool {
    JUNG_KEYS[jung].len() == 1
}

/// 이 키(들)로 치는 중성 번호.
pub fn jung_with_keys(keys: &str) -> Option<usize> {
    JUNG_KEYS.iter().position(|k| *k == keys)
}

pub fn make_syllable(cho: usize, jung: usize, jong: usize) -> char {
    compose(cho, jung, jong)
}

/// 음절의 종성 번호 (0 = 받침 없음).
pub fn jong_of(c: char) -> Option<usize> {
    split(c).map(|p| p.2)
}

/// 모음 키인지 (겹모음이 아닌 모음 하나).
fn is_vowel_key(c: char) -> bool {
    JUNG_KEYS.iter().any(|j| j.len() == 1 && j.starts_with(c))
}

/// 두벌식 키 순서를 입력기처럼 음절로 조합합니다. 모든 음절이 초성 + 중성(+ 종성)으로
/// 완성되지 않으면 None (예: 모음으로 시작, 모음 두 개가 겹모음이 안 됨, 받침이 될 수 없는 ㄸ).
/// 자음 다음에 모음이 오면 그 자음은 다음 음절의 초성이 됩니다 (`dlTdj` → 있어).
pub fn compose_keys(keys: &str) -> Option<String> {
    let k: Vec<char> = keys.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < k.len() {
        let cho = cho_with_keys(&k[i].to_string())?;
        i += 1;
        if !k.get(i).is_some_and(|&c| is_vowel_key(c)) {
            return None;
        }
        let mut jung = jung_with_keys(&k[i].to_string())?;
        i += 1;
        if let Some(&c) = k.get(i).filter(|&&c| is_vowel_key(c)) {
            jung = jung_with_keys(&format!("{}{c}", k[i - 1]))?;
            i += 1;
        }
        // 다음 모음 전까지의 자음: 모음이 뒤따르면 마지막 하나는 다음 음절의 초성.
        let start = i;
        while k.get(i).is_some_and(|&c| !is_vowel_key(c)) {
            i += 1;
        }
        let jong_len = if i < k.len() {
            (i - start).checked_sub(1)?
        } else {
            i - start
        };
        let jong_keys: String = k[start..start + jong_len].iter().collect();
        let jong = JONG_KEYS.iter().position(|j| *j == jong_keys)?;
        out.push(compose(cho, jung, jong));
        i = start + jong_len;
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_of_words() {
        assert_eq!(word_keys("학교").as_deref(), Some("gkrry"));
        assert_eq!(word_keys("과일").as_deref(), Some("rhkdlf"));
        assert_eq!(word_keys("닭").as_deref(), Some("ekfr"));
        assert_eq!(word_keys("있어").as_deref(), Some("dlTdj"));
        assert_eq!(word_keys("의사").as_deref(), Some("dmltk"));
        assert_eq!(word_keys("abc"), None);
    }

    #[test]
    fn every_syllable_roundtrips_through_partial() {
        for code in BASE..=LAST {
            let c = char::from_u32(code).unwrap();
            let n = syllable_keys(c).unwrap().chars().count();
            assert_eq!(partial(c, n), Some(c), "{c}");
            for k in 1..n {
                assert!(partial(c, k).is_some(), "{c} {k}");
            }
        }
    }

    #[test]
    fn partial_syllables() {
        assert_eq!(partial('학', 1), Some('ㅎ'));
        assert_eq!(partial('학', 2), Some('하'));
        assert_eq!(partial('과', 2), Some('고'));
        assert_eq!(partial('닭', 3), Some('달'));
        assert_eq!(partial('학', 0), None);
    }

    #[test]
    fn all_keys_have_jamo() {
        for c in ('a'..='z').chain(SHIFT_KEYS) {
            assert!(key_jamo(c).is_some(), "{c}");
        }
        assert_eq!(key_jamo('A'), None);
    }

    #[test]
    fn compose_keys_like_an_ime() {
        assert_eq!(compose_keys("gkrry").as_deref(), Some("학교"));
        assert_eq!(compose_keys("dlTdj").as_deref(), Some("있어"));
        assert_eq!(compose_keys("ekfrdl").as_deref(), Some("닭이"));
        assert_eq!(compose_keys("dmltk").as_deref(), Some("의사"));
        assert_eq!(compose_keys("rhkdlf").as_deref(), Some("과일"));
        assert_eq!(compose_keys("kr"), None); // 모음으로 시작
        assert_eq!(compose_keys("rkk"), None); // ㅏ 다음 ㅏ
        assert_eq!(compose_keys("rkE"), None); // ㄸ은 받침이 안 됨
        assert_eq!(compose_keys("r"), None); // 모음 없음
    }

    #[test]
    fn compose_keys_inverts_word_keys() {
        for w in &crate::words::WordList::korean().words {
            assert_eq!(
                compose_keys(&w.keys).as_deref(),
                Some(w.text.as_str()),
                "{}",
                w.keys
            );
        }
    }

    #[test]
    fn parts_from_keys() {
        let p = parts(&['r', 'k', 'h', 'l']);
        assert_eq!(p.cho, vec![0]); // ㄱ
        // ㅏ ㅗ ㅘ(hk) ㅚ(hl) ㅣ
        assert_eq!(p.jung.len(), 5);
        assert_eq!(p.jong, vec![0, 1]); // 없음, ㄱ
    }
}
