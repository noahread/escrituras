use std::io::{self, Stderr};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEvent, KeyEventKind, MouseEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use futures_util::StreamExt;
use tokio::sync::mpsc;

pub type Tui = Terminal<CrosstermBackend<Stderr>>;

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppEvent {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize(u16, u16),
    Tick,
}

pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<AppEvent>,
    _tx: mpsc::UnboundedSender<AppEvent>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let _tx = tx.clone();

        // Spawn event reader task
        let tx_events = tx.clone();
        tokio::spawn(async move {
            let mut reader = event::EventStream::new();
            loop {
                if let Some(Ok(evt)) = reader.next().await {
                    let app_event = match evt {
                        Event::Key(key) => {
                            // Only handle key press events, not release
                            if key.kind == KeyEventKind::Press {
                                Some(AppEvent::Key(key))
                            } else {
                                None
                            }
                        }
                        Event::Mouse(mouse) => Some(AppEvent::Mouse(mouse)),
                        Event::Resize(w, h) => Some(AppEvent::Resize(w, h)),
                        _ => None,
                    };

                    if let Some(event) = app_event {
                        if tx_events.send(event).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        // Spawn tick timer for animations (300ms interval)
        let tx_tick = tx.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(300));
            loop {
                interval.tick().await;
                if tx_tick.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        });

        Self { rx, _tx }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}

pub fn init() -> Result<Tui> {
    enable_raw_mode()?;
    execute!(io::stderr(), EnterAlternateScreen)?;

    // Enable mouse capture
    execute!(io::stderr(), crossterm::event::EnableMouseCapture)?;

    let backend = CrosstermBackend::new(io::stderr());
    let terminal = Terminal::new(backend)?;

    Ok(terminal)
}

pub fn restore() -> Result<()> {
    execute!(io::stderr(), crossterm::event::DisableMouseCapture)?;
    execute!(io::stderr(), LeaveAlternateScreen)?;
    disable_raw_mode()?;
    Ok(())
}

/// Install panic hook to restore terminal on panic
pub fn install_panic_hook() {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = restore();
        original_hook(panic_info);
    }));
}
