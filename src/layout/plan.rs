//! 시작 키와 언락 순서를 레이아웃 위치와 단어 목록으로 자동 계산합니다.

use super::cost::cost;
use super::{COLS, HOME_ROW, Layout, is_letter};
use crate::words::{WordList, letter_bit};

const VOWELS: [char; 5] = ['a', 'e', 'i', 'o', 'u'];
/// 홈열에서 시작 키를 고르는 순서: 검지 → 중지 → 약지 → 새끼 (안쪽 4, 5열 제외).
const START_COLS: [usize; 8] = [3, 6, 2, 7, 1, 8, 0, 9];
/// 시작 키 개수 범위.
pub const MIN_START: usize = 4;
pub const MAX_START: usize = 8;
/// 시작 키로 칠 수 있는 진짜 단어가 이만큼 있으면 충분하다고 봅니다.
const MIN_START_WORDS: usize = 10;
/// 새 단어가 하나도 안 생기는 단계에서도 순서를 정할 수 있게 글자 빈도를 조금 섞습니다.
const FREQ_SHARE: f64 = 0.1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// 전체 키 순서. 앞의 `start_len`개가 시작 키입니다.
    pub order: Vec<char>,
    pub start_len: usize,
}

/// 홈열 키를 검지부터 하나씩 늘려 가며, 진짜 단어가 충분히 나오는 가장 적은 개수(4~8)를 고릅니다.
/// 8개로도 부족하면 8개를 쓰고, 모음이 하나도 없으면 가장 쉬운 자리의 모음을 하나 더합니다.
pub fn start_keys(layout: &Layout, words: &WordList) -> Vec<char> {
    let home: Vec<char> = START_COLS.iter().map(|&c| layout.keys[HOME_ROW][c]).collect();
    let word_count = |keys: &[char]| {
        let mask = keys.iter().filter_map(|&c| letter_bit(c)).fold(0, |m, b| m | b);
        words.available(mask).len()
    };
    let n = (MIN_START..=MAX_START)
        .find(|&n| word_count(&home[..n]) >= MIN_START_WORDS)
        .unwrap_or(MAX_START);
    let mut start = home[..n].to_vec();

    if word_count(&start) < MIN_START_WORDS && !start.iter().any(|c| VOWELS.contains(c)) {
        let vowel = layout
            .iter()
            .filter(|p| VOWELS.contains(&p.2) && !start.contains(&p.2))
            .min_by(|a, b| cost(a.0, a.1).total_cmp(&cost(b.0, b.1)));
        if let Some(p) = vowel {
            start.push(p.2);
        }
    }
    start
}

pub fn make_plan(layout: &Layout, words: &WordList) -> Plan {
    let cost_of = |ch: char| {
        let (r, c) = layout.position(ch).expect("레이아웃에 있는 글자");
        cost(r, c)
    };
    let mut letters: Vec<char> = layout.iter().map(|p| p.2).filter(|&c| is_letter(c)).collect();
    letters.sort_by(|a, b| cost_of(*a).total_cmp(&cost_of(*b)));

    // 1. 시작 키.
    let start = start_keys(layout, words);
    let start_len = start.len();

    // 2. 나머지 글자: 새로 칠 수 있게 되는 단어가 가장 많은(자리 비용 대비) 글자부터.
    let total_weight: f64 = words.words.iter().map(|w| w.weight).sum::<f64>().max(1e-9);
    let mut freq = [0.0; 26];
    for w in &words.words {
        for ch in w.text.chars() {
            freq[(ch as u8 - b'a') as usize] += w.weight;
        }
    }
    let total_freq: f64 = freq.iter().sum::<f64>().max(1e-9);

    let mut order = start;
    let mut unlocked: u32 = order.iter().filter_map(|&c| letter_bit(c)).fold(0, |m, b| m | b);
    let mut remaining: Vec<char> = letters.iter().copied().filter(|c| !order.contains(c)).collect();
    while !remaining.is_empty() {
        let mut best: Option<(usize, f64)> = None;
        for (i, &ch) in remaining.iter().enumerate() {
            let next = unlocked | letter_bit(ch).unwrap();
            let gain: f64 = words
                .words
                .iter()
                .filter(|w| w.mask & !next == 0 && w.mask & !unlocked != 0)
                .map(|w| w.weight)
                .sum();
            let f = freq[(ch as u8 - b'a') as usize] / total_freq;
            let score = (gain / total_weight + FREQ_SHARE * f) / cost_of(ch);
            // remaining은 비용순이라, 동점이면 쉬운 자리가 먼저 뽑힙니다.
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((i, score));
            }
        }
        let (i, _) = best.unwrap();
        let ch = remaining.remove(i);
        unlocked |= letter_bit(ch).unwrap();
        order.push(ch);
    }

    // 3. 글자가 아닌 키(; , . / 등)는 맨 뒤에, 쉬운 자리부터.
    let mut others: Vec<(usize, usize, char)> = layout
        .iter()
        .filter(|p| !is_letter(p.2) && !order.contains(&p.2))
        .collect();
    others.sort_by(|a, b| cost(a.0, a.1).total_cmp(&cost(b.0, b.1)).then(a.1.cmp(&b.1)));
    order.extend(others.into_iter().map(|p| p.2));

    debug_assert_eq!(order.len(), layout.iter().count());
    debug_assert!(order.len() <= 3 * COLS);
    Plan { order, start_len }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start_of(rows: &str) -> String {
        let l = Layout::parse("t", rows).unwrap();
        start_keys(&l, &WordList::english()).into_iter().collect()
    }

    #[test]
    fn qwerty_plan() {
        let plan = make_plan(&Layout::qwerty(), &WordList::english());
        let start: String = plan.order[..plan.start_len].iter().collect();
        // f j d k s l 에는 모음이 없어서 a가 들어가는 7개에서 멈춥니다.
        assert_eq!(start, "fjdksla");
        assert_eq!(plan.order[plan.start_len], 'e', "{:?}", plan.order);
        assert_eq!(plan.order.len(), 30);
        // 구두점은 맨 뒤 4개.
        let tail: String = plan.order[26..].iter().collect();
        assert!(tail.chars().all(|c| ";,./".contains(c)), "{tail}");
        // 모든 키가 한 번씩.
        let mut sorted = plan.order.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 30);
    }

    #[test]
    fn uses_fewest_keys_that_give_enough_words() {
        // 검지·중지 자리에 t n s e 가 있으면 4개로 충분 (set, ten, nest, seen ...).
        assert_eq!(start_of("qwfpgjluy;\narstdhneio\nzxcvbkm,./"), "tnse");
    }

    #[test]
    fn punctuation_in_start_is_not_repeated() {
        // 홈열 8칸을 다 써도 단어가 부족한 경우 ; 가 시작 키에 포함됩니다.
        let l = Layout::parse("t", "qwertyuiop\nzsdfghjkl;\naxcvbnm,./").unwrap();
        let plan = make_plan(&l, &WordList::english());
        assert_eq!(plan.order.iter().filter(|&&c| c == ';').count(), 1);
        assert_eq!(plan.order.len(), 30);
    }

    #[test]
    fn adds_vowel_when_home_row_has_none() {
        let start = start_of("qwertyuiop\nzsdfghjklx\nacvbnm,./;");
        assert_eq!(start.len(), MAX_START + 1, "{start}");
        assert!(start.chars().any(|c| VOWELS.contains(&c)), "{start}");
    }
}
