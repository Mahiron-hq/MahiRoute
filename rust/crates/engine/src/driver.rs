// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Lev Burmistrov (Mahiron-hq)

//! Threads, timers and I/O around the pure [`Core`].

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use mahiroute_core::Settings;
use mahiroute_core::protocol::encode_line;

use crate::core::{Command, Core, Effect, HelperTarget, Timer};
use crate::io::{Connector, LineSource, Transport};
use crate::state::UiState;

/// Receives every new [`UiState`].
pub trait Listener: Send + Sync + 'static {
    /// Called on the engine thread; implementations must not block.
    fn state_changed(&self, state: &UiState);
}

enum Msg {
    Command(Command),
    Connected(u64, Box<dyn Transport>),
    ConnectFailed(u64, String),
    Line(u64, String),
    Closed(u64, String),
    Stop,
}

/// Handle to the running engine thread; dropping it stops the thread.
pub struct Engine {
    sender: Sender<Msg>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// Loads the settings from `settings_path` and starts the engine thread.
    pub fn start(
        settings_path: PathBuf,
        connector: Arc<dyn Connector>,
        listener: Arc<dyn Listener>,
    ) -> Self {
        let (settings, load_problem) = Settings::load_or_quarantine(&settings_path);
        let mut core = Core::new(settings);
        if let Some(problem) = load_problem {
            core.warn(format!("settings were reset: {problem}"));
        }
        let (sender, receiver) = mpsc::channel();
        let driver = Driver {
            core,
            settings_path,
            connector,
            listener,
            inbox: sender.clone(),
            transport: None,
            generation: 0,
            timers: HashMap::new(),
        };
        let thread = thread::Builder::new()
            .name("mahiroute-engine".to_owned())
            .spawn(move || driver.run(&receiver))
            .ok();
        Self { sender, thread }
    }

    /// Queues a command for the engine.
    pub fn command(&self, command: Command) {
        // A closed channel means the engine already stopped; the command has no one to serve.
        let _ = self.sender.send(Msg::Command(command));
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Driver {
    core: Core,
    settings_path: PathBuf,
    connector: Arc<dyn Connector>,
    listener: Arc<dyn Listener>,
    inbox: Sender<Msg>,
    transport: Option<Box<dyn Transport>>,
    generation: u64,
    timers: HashMap<Timer, Instant>,
}

impl Driver {
    fn run(mut self, receiver: &Receiver<Msg>) {
        self.listener.state_changed(&self.core.ui_state());
        loop {
            let message = match self.next_deadline() {
                Some(deadline) => match receiver
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                {
                    Ok(message) => Some(message),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                None => match receiver.recv() {
                    Ok(message) => Some(message),
                    Err(_) => break,
                },
            };
            let mut effects = Vec::new();
            match message {
                Some(Msg::Stop) => break,
                Some(message) => self.dispatch(message, &mut effects),
                None => self.fire_due_timers(&mut effects),
            }
            self.execute(effects);
        }
        if let Some(mut transport) = self.transport.take() {
            transport.close();
        }
    }

    fn dispatch(&mut self, message: Msg, effects: &mut Vec<Effect>) {
        match message {
            Msg::Command(command) => effects.extend(self.core.command(command)),
            Msg::Connected(generation, transport) if generation == self.generation => {
                self.transport = Some(transport);
                effects.extend(self.core.connected());
            }
            Msg::ConnectFailed(generation, reason) if generation == self.generation => {
                effects.extend(self.core.connect_failed(&reason));
            }
            Msg::Line(generation, line) if generation == self.generation => {
                effects.extend(self.core.incoming(&line));
            }
            Msg::Closed(generation, reason) if generation == self.generation => {
                effects.extend(self.core.closed(&reason));
            }
            // A late message of a connection that has been replaced; its transport is already closed.
            Msg::Connected(_, mut stale) => stale.close(),
            Msg::ConnectFailed(..) | Msg::Line(..) | Msg::Closed(..) | Msg::Stop => {}
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.timers.values().min().copied()
    }

    fn fire_due_timers(&mut self, effects: &mut Vec<Effect>) {
        let now = Instant::now();
        let due: Vec<Timer> = self
            .timers
            .iter()
            .filter(|(_, deadline)| **deadline <= now)
            .map(|(timer, _)| *timer)
            .collect();
        for timer in due {
            self.timers.remove(&timer);
            effects.extend(self.core.timer(timer));
        }
    }

    /// Runs effects, including the follow-ups that failures produce, then publishes the state once.
    fn execute(&mut self, effects: Vec<Effect>) {
        let mut queue = std::collections::VecDeque::from(effects);
        let mut emit = false;
        while let Some(effect) = queue.pop_front() {
            match effect {
                Effect::Emit => emit = true,
                Effect::Persist => {
                    if let Err(error) = self.core.settings().save(&self.settings_path) {
                        self.core.warn(format!("settings not saved: {error}"));
                        emit = true;
                    }
                }
                Effect::Schedule(timer, delay) => {
                    self.timers.insert(timer, Instant::now() + delay);
                }
                Effect::Cancel(timer) => {
                    self.timers.remove(&timer);
                }
                Effect::Close => {
                    self.generation += 1;
                    if let Some(mut transport) = self.transport.take() {
                        transport.close();
                    }
                }
                Effect::Connect(target) => self.spawn_connect(target),
                Effect::Send(request) => {
                    if let Err(reason) = self.send(&request) {
                        queue.extend(self.core.closed(&reason));
                    }
                }
            }
        }
        if emit {
            self.listener.state_changed(&self.core.ui_state());
        }
    }

    fn send(&mut self, request: &mahiroute_core::protocol::Request) -> Result<(), String> {
        let line = encode_line(request).map_err(|error| error.to_string())?;
        let Some(transport) = self.transport.as_mut() else {
            return Err("not connected to the helper".to_owned());
        };
        transport
            .send_line(&line)
            .map_err(|error| format!("writing to the helper failed: {error}"))
    }

    fn spawn_connect(&mut self, target: HelperTarget) {
        self.generation += 1;
        let generation = self.generation;
        let connector = Arc::clone(&self.connector);
        let inbox = self.inbox.clone();
        let spawned = thread::Builder::new()
            .name("mahiroute-connect".to_owned())
            .spawn(move || match connector.connect(&target) {
                Ok((transport, source)) => {
                    if inbox.send(Msg::Connected(generation, transport)).is_ok() {
                        read_loop(generation, source, &inbox);
                    }
                }
                Err(error) => {
                    let _ = inbox.send(Msg::ConnectFailed(
                        generation,
                        format!("cannot reach the helper: {error}"),
                    ));
                }
            });
        if spawned.is_err() {
            let _ = self.inbox.send(Msg::ConnectFailed(
                generation,
                "cannot start a connection thread".to_owned(),
            ));
        }
    }
}

fn read_loop(generation: u64, mut source: Box<dyn LineSource>, inbox: &Sender<Msg>) {
    loop {
        match source.next_line() {
            Ok(Some(line)) => {
                if inbox.send(Msg::Line(generation, line)).is_err() {
                    return;
                }
            }
            Ok(None) => {
                let _ = inbox.send(Msg::Closed(
                    generation,
                    "the helper closed the connection".to_owned(),
                ));
                return;
            }
            Err(error) => {
                let _ = inbox.send(Msg::Closed(
                    generation,
                    format!("reading from the helper failed: {error}"),
                ));
                return;
            }
        }
    }
}
