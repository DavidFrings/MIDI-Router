use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use log::info;
use ratatui::{
    DefaultTerminal, Frame,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Clear, Padding, Paragraph, Widget, Wrap},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
    },
    time::Duration,
};

#[derive(Debug)]
pub struct App {
    controller_name: String,
    software_name: String,
    exit: Arc<AtomicBool>,
    restart: Arc<AtomicBool>,
    logs_router: Vec<String>,
    logs_api: Vec<String>,
    log_rx_router: Receiver<String>,
    log_rx_api: Receiver<String>,
}

impl App {
    pub fn new(
        controller_name: String,
        software_name: String,
        exit: Arc<AtomicBool>,
        restart: Arc<AtomicBool>,
        log_rx_router: Receiver<String>,
        log_rx_api: Receiver<String>,
    ) -> Self {
        Self {
            controller_name,
            software_name,
            exit,
            restart,
            logs_router: Vec::new(),
            logs_api: Vec::new(),
            log_rx_router,
            log_rx_api,
        }
    }
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        while !self.exit.load(Ordering::SeqCst) {
            while let Ok(line) = self.log_rx_router.try_recv() {
                self.logs_router.push(line);
                if self.logs_router.len() > 1000 {
                    self.logs_router.drain(0..200);
                }
            }

            while let Ok(line) = self.log_rx_api.try_recv() {
                self.logs_api.push(line);
                if self.logs_api.len() > 1000 {
                    self.logs_api.drain(0..200);
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
        if event::poll(Duration::from_millis(50))? {
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
            "| ".green().bold(),
            "Reload router ".white(),
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

        let bottom = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(25), Constraint::Percentage(75)].as_ref())
            .split(chunks[1]);

        let log_router_area = chunks[0];
        let log_api_area = bottom[1];

        let log_router_lines: Vec<Line> = self
            .logs_router
            .iter()
            .map(|s| format_log_line(s))
            .collect();
        let log_api_lines: Vec<Line> = self.logs_api.iter().map(|s| format_log_line(s)).collect();

        let log_router_text = Text::from(log_router_lines);
        let log_api_text = Text::from(log_api_lines);

        Clear.render(log_router_area, buf);
        Clear.render(log_api_area, buf);

        let router_inner_h = log_router_area.height.saturating_sub(2);
        let api_inner_h = log_api_area.height.saturating_sub(2);

        let router_total_lines = self.logs_router.len();
        let api_total_lines = self.logs_api.len();

        let router_scroll_y = router_total_lines
            .saturating_sub(router_inner_h as usize)
            .min(u16::MAX as usize) as u16;
        let api_scroll_y = api_total_lines
            .saturating_sub(api_inner_h as usize)
            .min(u16::MAX as usize) as u16;

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

        Paragraph::new(log_router_text.on_black())
            .block(
                Block::bordered()
                    .title("Logs".bold())
                    .padding(Padding::new(1, 1, 0, 0))
                    .on_black()
                    .light_yellow(),
            )
            .wrap(Wrap { trim: true })
            .scroll((router_scroll_y, 0))
            .render(log_router_area, buf);
        Paragraph::new(log_api_text.on_black())
            .block(
                Block::bordered()
                    .title("API Logs".bold())
                    .padding(Padding::new(1, 1, 0, 0))
                    .on_black()
                    .light_blue(),
            )
            .wrap(Wrap { trim: true })
            .scroll((api_scroll_y, 0))
            .render(log_api_area, buf);
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

fn format_log_line(s: &str) -> Line {
    if let Some(rest) = s.strip_prefix("INFO: ") {
        Line::from(vec!["INFO:  ".green().bold(), rest.white()])
    } else if let Some(rest) = s.strip_prefix("ERROR: ") {
        Line::from(vec!["ERROR: ".red().bold(), rest.white()])
    } else if let Some(rest) = s.strip_prefix("WARN: ") {
        Line::from(vec!["WARN:  ".yellow().bold(), rest.white()])
    } else if let Some(rest) = s.strip_prefix("DEBUG: ") {
        Line::from(vec!["DEBUG: ".blue(), rest.white()])
    } else {
        Line::from(s)
    }
}
