//! 연습할 한 줄: 누를 키 순서와, 화면에 보여줄 글자 단위(영어 글자, 한글 음절, 띄어쓰기).

use crate::hangul;
use crate::words::line::Item;

/// 화면 글자 하나와 그 글자를 치는 키의 범위 `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unit {
    pub ch: char,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Target {
    pub keys: Vec<char>,
    pub units: Vec<Unit>,
}

impl Target {
    pub fn from_items(items: &[Item]) -> Self {
        let mut t = Target::default();
        for (i, item) in items.iter().enumerate() {
            if i > 0 {
                t.push(' ', " ");
            }
            let mut keys = item.keys.as_str();
            for ch in item.text.chars() {
                let n = if hangul::is_syllable(ch) {
                    hangul::syllable_keys(ch).map_or(1, |k| k.chars().count())
                } else {
                    1
                };
                let split = keys.char_indices().nth(n).map_or(keys.len(), |(i, _)| i);
                let (head, rest) = keys.split_at(split);
                t.push(ch, head);
                keys = rest;
            }
        }
        t
    }

    fn push(&mut self, ch: char, keys: &str) {
        let start = self.keys.len();
        self.keys.extend(keys.chars());
        self.units.push(Unit {
            ch,
            start,
            end: self.keys.len(),
        });
    }

    #[cfg(test)]
    pub fn width(&self) -> usize {
        self.units.iter().map(|u| crate::words::char_width(u.ch)).sum()
    }

    /// 키를 `pos`개 쳤을 때 이 글자가 어떻게 보이는지. 아직 시작 전이면 None.
    pub fn typed_form(&self, unit: &Unit, pos: usize) -> Option<char> {
        if pos <= unit.start {
            return None;
        }
        if pos >= unit.end {
            return Some(unit.ch);
        }
        hangul::partial(unit.ch, pos - unit.start)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(text: &str, keys: &str) -> Item {
        Item {
            text: text.into(),
            keys: keys.into(),
        }
    }

    #[test]
    fn english_units_are_letters() {
        let t = Target::from_items(&[item("ask", "ask"), item("dad", "dad")]);
        assert_eq!(t.keys.iter().collect::<String>(), "ask dad");
        assert_eq!(t.units.len(), 7);
        assert_eq!(t.width(), 7);
    }

    #[test]
    fn korean_units_are_syllables() {
        let t = Target::from_items(&[item("학교", "gkrry"), item("과일", "rhkdlf")]);
        assert_eq!(t.keys.iter().collect::<String>(), "gkrry rhkdlf");
        let chars: String = t.units.iter().map(|u| u.ch).collect();
        assert_eq!(chars, "학교 과일");
        assert_eq!((t.units[0].start, t.units[0].end), (0, 3));
        assert_eq!((t.units[3].start, t.units[3].end), (6, 9));
        assert_eq!(t.width(), 9);
        // '과'를 두 키(r h)까지 치면 '고'로 보입니다.
        assert_eq!(t.typed_form(&t.units[3], 8), Some('고'));
        assert_eq!(t.typed_form(&t.units[3], 6), None);
        assert_eq!(t.typed_form(&t.units[0], 5), Some('학'));
    }
}
