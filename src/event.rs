use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use anyhow::Result;
use ratatui::crossterm::event::{self, Event, KeyEvent};

use crate::review::Review;

pub enum AppEvent {
    Key(KeyEvent),
    Tick,
    Resize,
    ReviewComplete(usize, Result<Review, String>),
    SubmitComplete(Result<(), String>),
}

pub struct EventHandler {
    rx: mpsc::Receiver<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> Self {
        let (tx, rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));

        let thread = Self::spawn_poll_thread(tx.clone(), stop.clone(), tick_rate);

        Self {
            rx,
            tx,
            stop,
            thread: Some(thread),
            tick_rate,
        }
    }

    fn spawn_poll_thread(
        tx: mpsc::Sender<AppEvent>,
        stop: Arc<AtomicBool>,
        tick_rate: Duration,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if event::poll(tick_rate).unwrap_or(false) {
                    match event::read() {
                        Ok(Event::Key(key)) => {
                            if tx.send(AppEvent::Key(key)).is_err() {
                                break;
                            }
                        }
                        Ok(Event::Resize(_, _)) => {
                            if tx.send(AppEvent::Resize).is_err() {
                                break;
                            }
                        }
                        _ => {}
                    }
                } else if tx.send(AppEvent::Tick).is_err() {
                    break;
                }
            }
        })
    }

    pub fn next(&self) -> Result<AppEvent> {
        Ok(self.rx.recv()?)
    }

    pub fn sender(&self) -> mpsc::Sender<AppEvent> {
        self.tx.clone()
    }

    /// Stop the event polling thread. Call before opening an external process.
    pub fn pause(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        // Drain any queued events
        while self.rx.try_recv().is_ok() {}
    }

    /// Restart the event polling thread. Call after external process exits.
    pub fn resume(&mut self) {
        self.stop.store(false, Ordering::Relaxed);
        self.thread = Some(Self::spawn_poll_thread(
            self.tx.clone(),
            self.stop.clone(),
            self.tick_rate,
        ));
    }
}
