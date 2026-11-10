//! 설정/진도 파일 위치와 읽기·쓰기.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::progress::Profile;

pub struct Paths {
    /// 직접 만든 레이아웃을 넣는 곳.
    pub layouts: Option<PathBuf>,
    /// 레이아웃별 진도 파일이 저장되는 곳.
    pub profiles: PathBuf,
    /// 영어 언락 순서 계산에 쓰는 corpus.bin 기본 위치.
    pub corpus: Option<PathBuf>,
}

impl Paths {
    pub fn detect() -> Self {
        match ProjectDirs::from("", "", "taja") {
            Some(d) => Paths {
                layouts: Some(d.config_dir().join("layouts")),
                profiles: d.data_dir().join("profiles"),
                corpus: Some(d.config_dir().join("corpus.bin")),
            },
            None => Paths {
                layouts: None,
                profiles: PathBuf::from(".taja"),
                corpus: None,
            },
        }
    }

    pub fn profile_file(&self, layout_name: &str) -> PathBuf {
        let safe: String = layout_name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        self.profiles.join(format!("{safe}.json"))
    }
}

pub fn load_profile(path: &Path) -> io::Result<Option<Profile>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// 임시 파일에 쓰고 이름을 바꿔서, 저장 중에 꺼져도 기존 파일이 망가지지 않게 합니다.
pub fn save_profile(path: &Path, profile: &Profile) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_string(profile).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::Layout;
    use crate::layout::plan::make_plan;
    use crate::words::WordList;

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("taja-test-{}", std::process::id()));
        let path = dir.join("profiles").join("qwerty.json");
        assert!(load_profile(&path).unwrap().is_none());
        let l = Layout::qwerty();
        let p = Profile::new(&l, &make_plan(&l, &WordList::english()));
        save_profile(&path, &p).unwrap();
        assert_eq!(load_profile(&path).unwrap(), Some(p));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn safe_file_names() {
        let p = Paths {
            layouts: None,
            profiles: PathBuf::from("x"),
            corpus: None,
        };
        assert_eq!(
            p.profile_file("my/../lay out"),
            PathBuf::from("x/my____lay_out.json")
        );
    }
}
