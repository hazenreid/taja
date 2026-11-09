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
    /// English
    En,
    /// Korean (dubeolsik; follows the layout's letters: q=ㅂ, w=ㅈ, ...)
    Ko,
}

/// Terminal keyboard trainer: start with a few home-row keys and unlock the next key
/// once every open key reaches the target speed and accuracy.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Layout name (<name>.txt in the layouts folder) or a file path
    #[arg(short, long, default_value = "qwerty")]
    layout: String,

    /// Language to practice
    #[arg(long, value_enum, default_value = "en")]
    lang: LangArg,

    /// Per-key target speed to unlock the next key (WPM; Korean shows it x5 as keystrokes per minute)
    #[arg(long, default_value_t = 30.0)]
    target_wpm: f64,

    /// Per-key target accuracy to unlock the next key (%)
    #[arg(long, default_value_t = 95.0)]
    target_acc: f64,

    /// Erase progress for this layout and language and start over
    #[arg(long)]
    reset: bool,

    /// Print the layouts and progress folders and exit
    #[arg(long)]
    paths: bool,

    /// English word list to practice from (one word per line)
    #[arg(long, default_value = words::DEFAULT_DICT)]
    dict: String,
}

fn main() -> ExitCode {
    match run_app(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_app(args: Args) -> Result<(), String> {
    let paths = Paths::detect();
    if args.paths {
        match &paths.layouts {
            Some(dir) => println!("Layouts folder:  {}", dir.display()),
            None => println!("Layouts folder:  (not found; pass a file path instead)"),
        }
        println!("Progress folder: {}", paths.profiles.display());
        return Ok(());
    }
    if !(1.0..=300.0).contains(&args.target_wpm) {
        return Err("--target-wpm must be between 1 and 300".into());
    }
    if !(1.0..=100.0).contains(&args.target_acc) {
        return Err("--target-acc must be between 1 and 100".into());
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
    // 영어는 사전 파일에서만, 한글은 내장 단어 목록에서.
    let words = match lang {
        Lang::En => {
            let text = std::fs::read_to_string(&args.dict).map_err(|e| {
                format!(
                    "cannot read dictionary ({}): {e}\nPass a word list with --dict <path>",
                    args.dict
                )
            })?;
            let words = WordList::english_from(&text);
            if words.words.is_empty() {
                return Err(format!("no usable words in dictionary ({})", args.dict));
            }
            words
        }
        Lang::Ko => WordList::korean(),
    };
    // 진도는 레이아웃과 언어마다 따로.
    let save_path = match lang {
        Lang::En => paths.profile_file(&layout.name),
        Lang::Ko => paths.profile_file(&format!("{}-ko", layout.name)),
    };

    let (profile, message) = match (args.reset, load_profile(&save_path)) {
        (false, Ok(Some(p))) if p.is_valid_for(&layout) => (p, None),
        (false, Ok(Some(_))) => (
            Profile::new(&layout, &make_plan(&layout, &words)),
            Some("Layout changed, starting progress over".to_string()),
        ),
        (false, Err(e)) => {
            return Err(format!(
                "cannot read progress file ({}): {e}",
                save_path.display()
            ));
        }
        _ => {
            let p = Profile::new(&layout, &make_plan(&layout, &words));
            let keys: Vec<String> = p.unlocked_keys().iter().map(|&k| lang.key_label(k)).collect();
            let msg = format!("Start keys: {}", keys.join(" "));
            (p, Some(msg))
        }
    };
    let missing = layout.missing_letters();
    let message = match (message, missing.is_empty()) {
        (m, true) => m,
        (m, false) => {
            let list: String = missing.iter().collect();
            Some(format!(
                "{}Letters missing from layout: {list}",
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
