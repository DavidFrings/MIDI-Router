mod router;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use log::{LevelFilter, debug, info};
use log::{Metadata, Record};
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::widgets::{Clear, Padding, Wrap};
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::Rect,
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
};
use reqwest::Client;
use router::midi_connection::MidiRouter;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::{process::Command, thread, time::Duration};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    #[clap(short, long, default_value = "APC40 mkII")]
    controller_name: String,

    #[clap(short, long, default_value = "Daslight")]
    software_name: String,
}

struct ForwardLogger {
    tx: Mutex<Sender<String>>,
}

#[derive(Debug)]
pub struct App {
    controller_name: String,
    software_name: String,
    exit: Arc<AtomicBool>,
    restart: Arc<AtomicBool>,
    logs: Vec<String>,
    log_rx: Receiver<String>,
}

impl log::Log for ForwardLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }
    fn log(&self, record: &Record) {
        let msg = format!("{}: {}", record.level(), record.args());
        if let Ok(tx) = self.tx.lock() {
            let _ = tx.send(msg);
        }
    }
    fn flush(&self) {}
}

impl App {
    pub fn new(
        controller_name: String,
        software_name: String,
        exit: Arc<AtomicBool>,
        restart: Arc<AtomicBool>,
        log_rx: Receiver<String>,
    ) -> Self {
        Self {
            controller_name,
            software_name,
            exit,
            restart,
            logs: Vec::new(),
            log_rx,
        }
    }
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit.load(Ordering::SeqCst) {
            while let Ok(line) = self.log_rx.try_recv() {
                self.logs.push(line);
                if self.logs.len() > 1000 {
                    self.logs.drain(0..200);
                }
            }

            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_events(&mut self) -> Result<()> {
        if crossterm::event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                    self.handle_key_event(key_event)
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit_now(),
            KeyCode::Char('r') => {
                self.restart.store(true, Ordering::SeqCst);
                info!("Restart requested from TUI");
            }
            _ => {}
        }
    }

    fn exit_now(&self) {
        self.exit.store(true, Ordering::SeqCst);
    }
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = Line::from("[ MIDI Router ]".bold());
        let instructions = Line::from(vec![
            "[ ".bold(),
            "Quit ".white(),
            "<Q> ".blue().bold(),
            "| Restart router ".white(),
            "<R>".blue().bold(),
            " ]".bold(),
        ]);
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .on_black()
            .border_set(border::THICK)
            .green();

        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
            .split(area);

        let log_area = chunks[0];

        let mut log_lines: Vec<Line> = Vec::with_capacity(self.logs.len());
        for s in &self.logs {
            if let Some(rest) = s.strip_prefix("INFO: ") {
                log_lines.push(Line::from(vec!["INFO: ".green().bold(), rest.white()]));
            } else if let Some(rest) = s.strip_prefix("ERROR: ") {
                log_lines.push(Line::from(vec!["ERROR: ".red().bold(), rest.white()]));
            } else {
                // optional: weitere Level
                if let Some(rest) = s.strip_prefix("WARN: ") {
                    log_lines.push(Line::from(vec!["WARN: ".yellow().bold(), rest.white()]));
                } else if let Some(rest) = s.strip_prefix("DEBUG: ") {
                    log_lines.push(Line::from(vec!["DEBUG: ".blue(), rest.white()]));
                } else {
                    log_lines.push(Line::from(s.as_str()));
                }
            }
        }
        let log_text = Text::from(log_lines);

        Clear.render(log_area, buf);

        let inner_h = log_area.height.saturating_sub(2);
        let total_lines = self.logs.len();
        let scroll_y = total_lines
            .saturating_sub(inner_h as usize)
            .min(u16::MAX as usize) as u16;

        let bottom = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(75)].as_ref())
            .split(chunks[1]);
        let log_api_text =
            Text::from(vec![Line::from("Logs from actrix_web... (Coming soon)")]).white();

        let config_text = Text::from(vec![
            Line::from(vec![
                "Controller:    ".into(),
                self.controller_name.to_string().red().bold(),
            ]),
            Line::from(vec![
                "to Software:   ".into(),
                "to_".blue().bold(),
                self.software_name.to_string().blue().bold(),
            ]),
            Line::from(vec![
                "from Software: ".into(),
                "from_".cyan().bold(),
                self.software_name.to_string().cyan().bold(),
            ]),
        ])
        .white();

        Paragraph::new(log_text.on_black())
            .block(
                Block::bordered()
                    .title("Logs".bold())
                    .padding(Padding::new(1, 1, 0, 0))
                    .on_black()
                    .light_yellow(),
            )
            .wrap(Wrap { trim: true })
            .scroll((scroll_y as u16, 0))
            .render(log_area, buf);
        Paragraph::new(log_api_text)
            .block(
                Block::bordered()
                    .title("API Logs".bold())
                    .padding(Padding::new(1, 1, 0, 0))
                    .on_black()
                    .light_blue(),
            )
            .render(bottom[1], buf);
        Paragraph::new(config_text)
            .block(
                Block::bordered()
                    .title("Config".bold())
                    .padding(Padding::new(1, 1, 0, 0))
                    .on_black()
                    .dark_gray(),
            )
            .render(bottom[0], buf);
    }
}

async fn update() -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    let url = "https://api.github.com/repos/DavidFrings/MIDI-Router/releases/latest";
    let client = Client::new();
    let res: serde_json::Value = client
        .get(url)
        .header("User-Agent", "MIDI-Router-Updater")
        .send()
        .await?
        .json()
        .await?;
    let latest = res["tag_name"].as_str().unwrap();

    if latest != format!("v{}", current_version) {
        let download_url = res["assets"][0]["browser_download_url"].as_str().unwrap();

        info!(
            "New version available: {} (current: v{})",
            latest, current_version
        );

        Command::new("updater.exe")
            .arg(download_url)
            .spawn()
            .expect("Failed to start updater");

        std::process::exit(0);
    }

    info!("Router is up to date (version {})", latest);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let (log_tx, log_rx) = mpsc::channel::<String>();

    let forward = ForwardLogger {
        tx: Mutex::new(log_tx.clone()),
    };
    log::set_boxed_logger(Box::new(forward))?;
    log::set_max_level(LevelFilter::Info);
    // Check for updates
    info!("Checking for updates...");
    update().await.expect("Failed to check for updates"); // Disable for dev!
    let args = Args::parse();

    let restart_flag = Arc::new(AtomicBool::new(false));
    let exit_flag = Arc::new(AtomicBool::new(false));

    // Router-Thread
    {
        let router_restart = restart_flag.clone();
        let router_exit = exit_flag.clone();
        let controller = args.controller_name.clone();
        let software = args.software_name.clone();

        thread::spawn(move || {
            loop {
                if router_exit.load(Ordering::SeqCst) {
                    info!("Router thread exiting");
                    break;
                }
                debug!("Starting MidiRouter::connect...");
                let mut router = MidiRouter::new();
                match MidiRouter::connect(&mut router, &controller, &software) {
                    Ok(_) => info!("Started MIDIRouter..."),
                    Err(e) => log::error!("MidiRouter::connect failed: {:?}", e),
                }
                while !router_restart.load(Ordering::SeqCst) && !router_exit.load(Ordering::SeqCst)
                {
                    thread::sleep(Duration::from_millis(200));
                }
                if router_exit.load(Ordering::SeqCst) {
                    info!("Router thread exiting after wait");
                    break;
                }
                if router_restart.load(Ordering::SeqCst) {
                    router_restart.store(false, Ordering::SeqCst);
                    debug!("Router restart requested, reconnecting...");
                    continue;
                }
            }
        });
    }

    // TUI-Thread
    {
        let mut app = App::new(
            args.controller_name.clone(),
            args.software_name.clone(),
            exit_flag.clone(),
            restart_flag.clone(),
            log_rx,
        );

        let handle = thread::spawn(move || {
            let mut terminal = ratatui::init();
            let res = app.run(&mut terminal);
            ratatui::restore();
            res
        });

        let res = handle.join().expect("UI thread panicked");
        if let Err(e) = res {
            return Err(e);
        }
        exit_flag.store(true, Ordering::SeqCst);
        thread::sleep(Duration::from_millis(200));
    }

    Ok(())
}
