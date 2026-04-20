//! Event normalisation: crossterm key events + periodic ticks.

use std::time::Duration;

use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tokio::task::JoinHandle;

#[derive(Debug, Clone, Copy)]
pub enum AppEvent {
    Key(KeyEvent),
    /// The terminal was resized — ratatui handles the layout on the next draw.
    Resize,
    /// Periodic tick for clock/refresh of time-based widgets.
    Tick,
}

pub struct Events {
    rx: UnboundedReceiver<AppEvent>,
    #[allow(dead_code)]
    reader: JoinHandle<()>,
    #[allow(dead_code)]
    ticker: JoinHandle<()>,
}

impl Events {
    /// Start the reader + ticker tasks. The reader polls stdin on the blocking
    /// pool so it can't starve the main loop; the ticker sends [`AppEvent::Tick`]
    /// every `tick` duration.
    pub fn start(tick: Duration) -> Self {
        let (tx, rx) = unbounded_channel();
        let reader = spawn_reader(tx.clone());
        let ticker = spawn_ticker(tx, tick);
        Self { rx, reader, ticker }
    }

    pub async fn next(&mut self) -> Option<AppEvent> {
        self.rx.recv().await
    }
}

fn spawn_reader(tx: UnboundedSender<AppEvent>) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        loop {
            match event::read() {
                Ok(Event::Key(key)) if key.kind == KeyEventKind::Press => {
                    if tx.send(AppEvent::Key(key)).is_err() {
                        break;
                    }
                }
                Ok(Event::Resize(_, _)) => {
                    if tx.send(AppEvent::Resize).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(err) => {
                    tracing::warn!("event read failed: {err}");
                    break;
                }
            }
        }
    })
}

fn spawn_ticker(tx: UnboundedSender<AppEvent>, tick: Duration) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick);
        // Skip the immediate first tick so the loop doesn't double-fire on start.
        interval.tick().await;
        loop {
            interval.tick().await;
            if tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    })
}
