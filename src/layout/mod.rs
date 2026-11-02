//! 10×3 키보드 레이아웃.
//!
//! 레이아웃 파일은 텍스트 3줄이고, 줄마다 글자 10개입니다 (공백은 무시).
//! 영어 글자(a~z)가 아닌 칸은 모두 빈 칸(키 없음)으로 처리합니다.
//! 손가락, 시작 키, 언락 순서는 모두 위치로부터 자동 계산합니다.

pub mod cost;
pub mod plan;

use std::fmt;
use std::path::Path;

pub const ROWS: usize = 3;
pub const COLS: usize = 10;
pub const HOME_ROW: usize = 1;

const QWERTY: &str = include_str!("../../assets/layouts/qwerty.txt");

/// 빈 칸을 문자열로 보여줄 때 쓰는 글자.
const EMPTY_MARK: char = '.';
/// 내부에서 빈 칸을 나타내는 값 (파일에서 공백은 무시되므로 실제 키와 겹치지 않음).
const BLANK: char = ' ';

/// 레이아웃에 최소한 있어야 하는 영어 글자 수.
const MIN_LETTERS: usize = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub name: String,
    pub keys: [[char; COLS]; ROWS],
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    RowCount(usize),
    RowLength {
        row: usize,
        len: usize,
    },
    Duplicate {
        ch: char,
        first: (usize, usize),
        second: (usize, usize),
    },
    TooFewLetters(usize),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::RowCount(n) => {
                write!(f, "expected {ROWS} rows, found {n}")
            }
            ParseError::RowLength { row, len } => {
                write!(f, "row {}: expected {COLS} keys, found {len}", row + 1)
            }
            ParseError::Duplicate { ch, first, second } => write!(
                f,
                "'{ch}' appears twice (row {} col {}, row {} col {})",
                first.0 + 1,
                first.1 + 1,
                second.0 + 1,
                second.1 + 1
            ),
            ParseError::TooFewLetters(n) => {
                write!(f, "only {n} letters (need at least {MIN_LETTERS})")
            }
        }
    }
}

impl std::error::Error for ParseError {}

impl Layout {
    pub fn parse(name: &str, text: &str) -> Result<Self, ParseError> {
        let rows: Vec<Vec<char>> = text
            .lines()
            .map(|l| l.chars().filter(|c| !c.is_whitespace()).collect::<Vec<_>>())
            .filter(|r| !r.is_empty())
            .collect();
        if rows.len() != ROWS {
            return Err(ParseError::RowCount(rows.len()));
        }

        let mut keys = [[' '; COLS]; ROWS];
        for (r, row) in rows.iter().enumerate() {
            if row.len() != COLS {
                return Err(ParseError::RowLength {
                    row: r,
                    len: row.len(),
                });
            }
            for (c, &ch) in row.iter().enumerate() {
                let ch = ch.to_ascii_lowercase();
                keys[r][c] = if is_letter(ch) { ch } else { BLANK };
            }
        }

        let layout = Layout {
            name: name.to_string(),
            keys,
        };
        let positions: Vec<_> = layout.iter().collect();
        for (i, &(r1, c1, a)) in positions.iter().enumerate() {
            if let Some(&(r2, c2, _)) = positions[i + 1..].iter().find(|p| p.2 == a) {
                return Err(ParseError::Duplicate {
                    ch: a,
                    first: (r1, c1),
                    second: (r2, c2),
                });
            }
        }

        let letters = layout.iter().filter(|p| is_letter(p.2)).count();
        if letters < MIN_LETTERS {
            return Err(ParseError::TooFewLetters(letters));
        }
        Ok(layout)
    }

    pub fn qwerty() -> Self {
        Self::parse("qwerty", QWERTY).expect("built-in QWERTY layout is invalid")
    }

    /// `arg`가 "qwerty"면 내장 레이아웃, 존재하는 파일 경로면 그 파일,
    /// 아니면 `layouts_dir/<arg>.txt`를 읽습니다.
    pub fn load(arg: &str, layouts_dir: Option<&Path>) -> Result<Self, String> {
        if arg.eq_ignore_ascii_case("qwerty") {
            return Ok(Self::qwerty());
        }
        let direct = Path::new(arg);
        let path = if direct.is_file() {
            direct.to_path_buf()
        } else if let Some(dir) = layouts_dir {
            dir.join(format!("{arg}.txt"))
        } else {
            direct.to_path_buf()
        };
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read layout file ({}): {e}", path.display()))?;
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or(arg);
        Self::parse(name, &text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// 빈 칸을 뺀 (줄, 칸, 글자)를 모두 돌려줍니다.
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, char)> + '_ {
        self.keys
            .iter()
            .enumerate()
            .flat_map(|(r, row)| row.iter().enumerate().map(move |(c, &ch)| (r, c, ch)))
            .filter(|p| p.2 != BLANK)
    }

    /// 그 자리의 키. 빈 칸이면 None.
    pub fn key_at(&self, row: usize, col: usize) -> Option<char> {
        Some(self.keys[row][col]).filter(|&c| c != BLANK)
    }

    pub fn position(&self, ch: char) -> Option<(usize, usize)> {
        self.iter().find(|p| p.2 == ch).map(|p| (p.0, p.1))
    }

    /// a~z 중 레이아웃에 없는 글자.
    pub fn missing_letters(&self) -> Vec<char> {
        ('a'..='z').filter(|&c| self.position(c).is_none()).collect()
    }

    /// 프로필이 같은 레이아웃용인지 확인할 때 쓰는 3줄 문자열.
    pub fn rows_as_strings(&self) -> Vec<String> {
        self.keys
            .iter()
            .map(|r| {
                r.iter()
                    .map(|&c| if c == BLANK { EMPTY_MARK } else { c })
                    .collect()
            })
            .collect()
    }
}

pub fn is_letter(ch: char) -> bool {
    ch.is_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_qwerty() {
        let l = Layout::qwerty();
        assert_eq!(l.keys[1][0], 'a');
        assert_eq!(l.key_at(1, 9), None);
        assert_eq!(l.iter().count(), 26);
        assert_eq!(l.position('e'), Some((0, 2)));
        assert!(l.missing_letters().is_empty());
    }

    #[test]
    fn ignores_spaces_and_uppercase() {
        let l = Layout::parse("t", "Q W E R T Y U I O P\nasdfghjkl;\n\nzxcvbnm,./\n").unwrap();
        assert_eq!(l.keys[0][0], 'q');
    }

    #[test]
    fn non_letters_are_empty_slots() {
        let l = Layout::parse("mine", "pmal.zbfjy\ninerqxot.s\nkvgd..hcuw").unwrap();
        assert_eq!(l.iter().count(), 26);
        assert_eq!(l.key_at(0, 4), None);
        assert_eq!(l.key_at(0, 3), Some('l'));
        assert_eq!(l.position('.'), None);
        assert!(l.missing_letters().is_empty());
        assert_eq!(l.rows_as_strings()[2], "kvgd..hcuw");
    }

    #[test]
    fn rejects_bad_files() {
        assert_eq!(
            Layout::parse("t", "qwertyuiop\nasdfghjkl;"),
            Err(ParseError::RowCount(2))
        );
        assert_eq!(
            Layout::parse("t", "qwertyuiop\nasdfghjkl\nzxcvbnm,./"),
            Err(ParseError::RowLength { row: 1, len: 9 })
        );
        assert!(matches!(
            Layout::parse("t", "qwertyuiop\nasdfghjkla\nzxcvbnm,./"),
            Err(ParseError::Duplicate { ch: 'a', .. })
        ));
        assert_eq!(
            Layout::parse("t", "1234567890\n!@#$%^&*()\nabcd,./;'["),
            Err(ParseError::TooFewLetters(4))
        );
    }
}
