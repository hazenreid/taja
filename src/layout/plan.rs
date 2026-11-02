//! 시작 키와 언락 순서를 레이아웃 위치와 단어 목록으로 자동 계산합니다.

use super::cost::cost;
use super::{HOME_ROW, Layout, is_letter};
use crate::words::{WordList, key_bit, mask_of};

/// 홈열에서 시작 키를 고르는 순서: 검지 → 중지 → 약지 → 새끼 (안쪽 4, 5열 제외).
const START_COLS: [usize; 8] = [3, 6, 2, 7, 1, 8, 0, 9];
/// 시작 키 개수 범위.
pub const MIN_START: usize = 4;
pub const MAX_START: usize = 8;
/// 시작 키로 칠 수 있는 진짜 단어가 이만큼 있으면 충분하다고 봅니다.
const MIN_START_WORDS: usize = 10;
/// 새 단어가 하나도 안 생기는 단계에서도 순서를 정할 수 있게 글자 빈도를 조금 섞습니다.
const FREQ_SHARE: f64 = 0.1;
/// Shift를 같이 눌러야 하는 키는 이만큼 더 어렵다고 봅니다.
const SHIFT_COST: f64 = 1.5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// 전체 키 순서. 앞의 `start_len`개가 시작 키입니다.
    pub order: Vec<char>,
    pub start_len: usize,
}

/// 키의 자리 비용. Shift 키(대문자)는 같은 자리 소문자 비용 + Shift 비용.
pub fn key_cost(layout: &Layout, key: char) -> Option<f64> {
    let (r, c) = layout.position(key.to_ascii_lowercase())?;
    let shift = if key.is_ascii_uppercase() { SHIFT_COST } else { 0.0 };
    Some(cost(r, c) + shift)
}

/// 홈열 키를 검지부터 하나씩 늘려 가며, 진짜 단어가 충분히 나오는 가장 적은 개수(4~8)를 고릅니다.
/// 8개로도 부족하면 8개를 쓰고, 모음이 하나도 없으면 가장 쉬운 자리의 모음을 하나 더합니다.
pub fn start_keys(layout: &Layout, words: &WordList) -> Vec<char> {
    let vowels = words.lang.vowel_keys();
    let home: Vec<char> = START_COLS
        .iter()
        .filter_map(|&c| layout.key_at(HOME_ROW, c))
        .collect();
    let word_count = |keys: &[char]| words.available(mask_of(keys.iter().copied())).len();
    // 홈열에 빈 칸이 있으면 쓸 수 있는 키가 8개보다 적을 수 있습니다.
    let max = MAX_START.min(home.len());
    let n = (MIN_START.min(max)..=max)
        .find(|&n| word_count(&home[..n]) >= MIN_START_WORDS)
        .unwrap_or(max);
    let mut start = home[..n].to_vec();

    if word_count(&start) < MIN_START_WORDS && !start.iter().any(|c| vowels.contains(c)) {
        let vowel = layout
            .iter()
            .filter(|p| vowels.contains(&p.2) && !start.contains(&p.2))
            .min_by(|a, b| cost(a.0, a.1).total_cmp(&cost(b.0, b.1)));
        if let Some(p) = vowel {
            start.push(p.2);
        }
    }
    start
}

pub fn make_plan(layout: &Layout, words: &WordList) -> Plan {
    let cost_of = |ch: char| key_cost(layout, ch).expect("key must be in the layout");
    // 레이아웃의 글자 키 + 이 언어에서 쓰는 Shift 키.
    let mut keys: Vec<char> = layout.iter().map(|p| p.2).filter(|&c| is_letter(c)).collect();
    let shifted: Vec<char> = words
        .lang
        .shift_keys()
        .iter()
        .copied()
        .filter(|k| keys.contains(&k.to_ascii_lowercase()))
        .collect();
    keys.extend(shifted);
    keys.sort_by(|a, b| cost_of(*a).total_cmp(&cost_of(*b)));

    // 1. 시작 키.
    let start = start_keys(layout, words);
    let start_len = start.len();

    // 2. 나머지 키: 새로 칠 수 있게 되는 단어가 가장 많은(자리 비용 대비) 키부터.
    let total_weight: f64 = words.words.iter().map(|w| w.weight).sum::<f64>().max(1e-9);
    let index = |ch: char| key_bit(ch).unwrap().trailing_zeros() as usize;
    let mut freq = [0.0; 52];
    for w in &words.words {
        for ch in w.keys.chars() {
            freq[index(ch)] += w.weight;
        }
    }
    let total_freq: f64 = freq.iter().sum::<f64>().max(1e-9);

    let mut order = start;
    let mut unlocked = mask_of(order.iter().copied());
    let mut remaining: Vec<char> = keys.iter().copied().filter(|c| !order.contains(c)).collect();
    while !remaining.is_empty() {
        let mut best: Option<(usize, f64)> = None;
        for (i, &ch) in remaining.iter().enumerate() {
            let next = unlocked | key_bit(ch).unwrap();
            let gain: f64 = words
                .words
                .iter()
                .filter(|w| w.mask & !next == 0 && w.mask & !unlocked != 0)
                .map(|w| w.weight)
                .sum();
            let f = freq[index(ch)] / total_freq;
            let score = (gain / total_weight + FREQ_SHARE * f) / cost_of(ch);
            // remaining은 비용순이라, 동점이면 쉬운 자리가 먼저 뽑힙니다.
            if best.is_none_or(|(_, s)| score > s) {
                best = Some((i, score));
            }
        }
        let (i, _) = best.unwrap();
        let ch = remaining.remove(i);
        unlocked |= key_bit(ch).unwrap();
        order.push(ch);
    }

    debug_assert_eq!(order.len(), keys.len());
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
        // ; , . / 는 빈 칸이라 키는 26개.
        assert_eq!(plan.order.len(), 26);
        // 모든 키가 한 번씩.
        let mut sorted = plan.order.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 26);
    }

    #[test]
    fn layout_with_empty_slots() {
        let l = Layout::parse("mine", "pmal.zbfjy\ninerqxot.s\nkvgd..hcuw").unwrap();
        let plan = make_plan(&l, &WordList::english());
        assert_eq!(plan.order.len(), 26);
        let start = &plan.order[..plan.start_len];
        assert!((MIN_START..=MAX_START).contains(&start.len()), "{start:?}");
        assert!(
            start.iter().all(|&c| l.position(c).unwrap().0 == HOME_ROW),
            "{start:?}"
        );
    }

    #[test]
    fn uses_fewest_keys_that_give_enough_words() {
        // 검지·중지 자리에 t n s e 가 있으면 4개로 충분 (set, ten, nest, seen ...).
        assert_eq!(start_of("qwfpgjluy;\narstdhneio\nzxcvbkm,./"), "tnse");
    }

    #[test]
    fn adds_vowel_when_home_row_has_none() {
        let start = start_of("qwertyuiop\nzsdfghjklx\nacvbnm,./;");
        assert_eq!(start.len(), MAX_START + 1, "{start}");
        assert!(start.chars().any(|c| "aeiou".contains(c)), "{start}");
    }

    #[test]
    fn korean_plan_includes_shift_keys() {
        let plan = make_plan(&Layout::qwerty(), &WordList::korean());
        assert_eq!(plan.order.len(), 26 + 7);
        let start = &plan.order[..plan.start_len];
        assert!((MIN_START..=MAX_START + 1).contains(&start.len()), "{start:?}");
        // ㅆ(T)은 아주 흔해서 Shift 키 중 가장 먼저 열립니다.
        let first_shift = plan.order.iter().find(|c| c.is_ascii_uppercase());
        assert_eq!(first_shift, Some(&'T'), "{:?}", plan.order);
    }
}
