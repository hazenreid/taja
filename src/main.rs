mod app;
mod hangul;
mod layout;
mod lesson;
mod progress;
mod storage;
mod target;
mod ui;
mod words;

use std::io;
use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, ValueEnum};
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use app::App;
use layout::Layout;
use layout::plan::make_plan;
use progress::{Profile, Targets};
use storage::{Paths, load_profile};
use words::{Lang, WordList};

#[derive(Clone, Copy, ValueEnum)]
enum LangArg {
    /// 영어
    En,
    /// 한글 (두벌식, 영어 레이아웃의 글자를 따라감: q=ㅂ, w=ㅈ …)
    Ko,
}

/// 터미널 자판 학습기: 홈열 몇 개 키로 시작해서 목표 속도에 닿으면 다음 키를 엽니다.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// 레이아웃 이름(설정 폴더의 <이름>.txt) 또는 파일 경로
    #[arg(short, long, default_value = "qwerty")]
    layout: String,

    /// 연습할 언어
    #[arg(long, value_enum, default_value = "en")]
    lang: LangArg,

    /// 다음 키를 열기 위한 키별 목표 속도 (WPM, 한글은 ×5 해서 타수로 표시)
    #[arg(long, default_value_t = 25.0)]
    target_wpm: f64,

    /// 다음 키를 열기 위한 키별 목표 정확도 (%)
    #[arg(long, default_value_t = 95.0)]
    target_acc: f64,

    /// 이 레이아웃의 진도를 지우고 처음부터 시작
    #[arg(long)]
    reset: bool,

    /// 레이아웃/진도 파일 위치를 보여주고 끝냄
    #[arg(long)]
    paths: bool,
}

fn main() -> ExitCode {
    match run_app(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("오류: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_app(args: Args) -> Result<(), String> {
    let paths = Paths::detect();
    if args.paths {
        match &paths.layouts {
            Some(dir) => println!("레이아웃 폴더: {}", dir.display()),
            None => println!("레이아웃 폴더: (찾을 수 없음, 파일 경로로 지정하세요)"),
        }
        println!("진도 폴더:     {}", paths.profiles.display());
        return Ok(());
    }
    if !(1.0..=300.0).contains(&args.target_wpm) {
        return Err("--target-wpm은 1~300 사이여야 합니다".into());
    }
    if !(1.0..=100.0).contains(&args.target_acc) {
        return Err("--target-acc는 1~100 사이여야 합니다".into());
    }
    let targets = Targets {
        wpm: args.target_wpm,
        accuracy: args.target_acc / 100.0,
    };

    let layout = Layout::load(&args.layout, paths.layouts.as_deref())?;
    let lang = match args.lang {
        LangArg::En => Lang::En,
        LangArg::Ko => Lang::Ko,
    };
    let words = WordList::for_lang(lang);
    // 진도는 레이아웃과 언어마다 따로.
    let save_path = match lang {
        Lang::En => paths.profile_file(&layout.name),
        Lang::Ko => paths.profile_file(&format!("{}-ko", layout.name)),
    };

    let (profile, message) = match (args.reset, load_profile(&save_path)) {
        (false, Ok(Some(p))) if p.is_valid_for(&layout) => (p, None),
        (false, Ok(Some(_))) => (
            Profile::new(&layout, &make_plan(&layout, &words)),
            Some("레이아웃이 바뀌어서 진도를 새로 시작합니다".to_string()),
        ),
        (false, Err(e)) => {
            return Err(format!(
                "진도 파일을 읽을 수 없습니다 ({}): {e}",
                save_path.display()
            ));
        }
        _ => {
            let p = Profile::new(&layout, &make_plan(&layout, &words));
            let keys: Vec<String> = p.unlocked_keys().iter().map(|&k| lang.key_label(k)).collect();
            let msg = format!("시작 키: {}", keys.join(" "));
            (p, Some(msg))
        }
    };
    let missing = layout.missing_letters();
    let message = match (message, missing.is_empty()) {
        (m, true) => m,
        (m, false) => {
            let list: String = missing.iter().collect();
            Some(format!(
                "{}레이아웃에 없는 글자: {list}",
                m.map(|m| m + " · ").unwrap_or_default()
            ))
        }
    };

    let mut app = App::new(layout, words, profile, targets, save_path, message);
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    app.save();
    if let Some(e) = app.save_error {
        eprintln!("{e}");
    }
    result.map_err(|e| e.to_string())
}

fn event_loop(terminal: &mut DefaultTerminal, app: &mut App) -> io::Result<()> {
    let size = terminal.size()?;
    app.set_width(size.width);
    app.regenerate();
    while !app.quit {
        terminal.draw(|f| ui::draw(f, app))?;
        // 실시간 WPM이 갱신되도록 입력이 없어도 주기적으로 다시 그립니다.
        if !event::poll(Duration::from_millis(250))? {
            continue;
        }
        match event::read()? {
            // Windows에서 키를 뗄 때도 이벤트가 오므로 누를 때만 처리.
            Event::Key(key) if key.kind == KeyEventKind::Press => app.on_key(key),
            Event::Resize(w, _) => app.set_width(w),
            _ => {}
        }
    }
    Ok(())
}
