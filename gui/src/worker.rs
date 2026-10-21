use crate::{backend::qobject, demo, view::State};
use cxx_qt::CxxQtThread;
use cxx_qt_lib::QString;
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command as Process, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use zentools::{
    aod::AodCapture,
    epp::{EppManager, EppProfile},
    monitor::{HelperCommand, HelperEvent, Session, Snapshot},
};

pub enum Command {
    Pause(bool),
    Refresh,
    Connect,
    Disconnect,
    Apply(i32),
    Baseline,
    ClearBaseline,
    ImportFirmware(PathBuf),
    LoadBaseline(PathBuf),
    Save(PathBuf),
    Stop,
    Remote(u64, Result<HelperEvent, String>),
}
struct Helper {
    child: Child,
    input: Option<ChildStdin>,
    reader: Option<JoinHandle<()>>,
}
impl Helper {
    fn send(&mut self, command: &HelperCommand) -> Result<(), String> {
        let input = self.input.as_mut().ok_or("Hardware connection is closed")?;
        serde_json::to_writer(&mut *input, command).map_err(|e| e.to_string())?;
        input
            .write_all(b"\n")
            .and_then(|_| input.flush())
            .map_err(|e| e.to_string())
    }
}
impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.send(&HelperCommand::Shutdown {});
        self.input.take();
        for _ in 0..20 {
            if self.child.try_wait().ok().flatten().is_some() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}
fn launch(tx: Sender<Command>, generation: u64) -> Result<Helper, String> {
    let executable = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("zentools-helper");
    if !executable.is_file() {
        return Err(format!(
            "Hardware helper is missing at {}. Build both GUI and helper with just build-gui.",
            executable.display()
        ));
    }
    let mut child = Process::new("pkexec")
        .arg("--disable-internal-agent")
        .arg(executable)
        .arg("stream")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Cannot open system authorization: {e}"))?;
    let input = child.stdin.take();
    let output = child.stdout.take().ok_or("Cannot read hardware helper")?;
    let reader = thread::spawn(move || {
        let mut output = BufReader::new(output);
        loop {
            let mut line = Vec::new();
            // A known helper emits bounded snapshots. Also bound the wire parser.
            use std::io::Read;
            match output
                .by_ref()
                .take(8 * 1024 * 1024 + 1)
                .read_until(b'\n', &mut line)
            {
                Ok(0) => {
                    let _ = tx.send(Command::Remote(
                        generation,
                        Err("Hardware access ended or authorization was cancelled".into()),
                    ));
                    break;
                }
                Ok(_) if line.len() > 8 * 1024 * 1024 => {
                    let _ = tx.send(Command::Remote(
                        generation,
                        Err("Helper response exceeded the size limit".into()),
                    ));
                    break;
                }
                Ok(_) => {
                    if tx
                        .send(Command::Remote(
                            generation,
                            serde_json::from_slice(&line)
                                .map_err(|e| format!("Invalid hardware response: {e}")),
                        ))
                        .is_err()
                    {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Command::Remote(generation, Err(e.to_string())));
                    break;
                }
            }
        }
    });
    Ok(Helper {
        child,
        input,
        reader: Some(reader),
    })
}
fn publish(
    state: &State,
    destination: &CxxQtThread<qobject::Backend>,
    pending: &Arc<AtomicBool>,
) -> bool {
    if pending.swap(true, Ordering::AcqRel) {
        return false;
    }
    let json = state.render();
    let flag = Arc::clone(pending);
    if destination
        .queue(move |object| {
            object.set_view_json(QString::from(&json));
            flag.store(false, Ordering::Release);
        })
        .is_err()
    {
        pending.store(false, Ordering::Release);
    }
    true
}

type WorkerHandle = (Sender<Command>, JoinHandle<()>);
static WORKERS: Mutex<Vec<WorkerHandle>> = Mutex::new(Vec::new());

/// Join after the event loop, outside a QObject method and its Rust-state lock.
pub fn shutdown_all() {
    let workers = std::mem::take(&mut *WORKERS.lock().expect("worker registry"));
    for (tx, _) in &workers {
        let _ = tx.send(Command::Stop);
    }
    for (_, handle) in workers {
        let _ = handle.join();
    }
}

pub fn spawn(destination: CxxQtThread<qobject::Backend>, demo_mode: bool) -> Sender<Command> {
    let (tx, rx) = mpsc::channel();
    let sender = tx.clone();
    let handle = thread::spawn(move || run(destination, demo_mode, sender, rx));
    WORKERS
        .lock()
        .expect("worker registry")
        .push((tx.clone(), handle));
    tx
}
fn run(
    destination: CxxQtThread<qobject::Backend>,
    demo_mode: bool,
    tx: Sender<Command>,
    rx: Receiver<Command>,
) {
    let mut state = State {
        demo: demo_mode,
        ..State::default()
    };
    let mut session = (!demo_mode).then(Session::new);
    let mut helper: Option<Helper> = None;
    let mut generation = 0;
    let mut refresh_pending = false;
    let mut force_snapshot = false;
    let mut tick = 0;
    let mut next = Instant::now();
    let mut dirty = true;
    let mut imported: Option<AodCapture> = None;
    if demo_mode {
        for i in 0..120 {
            let mut preview = demo::snapshot(i);
            preview.timestamp_ms = zentools::monitor::now_ms().saturating_sub((119 - i) * 1000);
            state.accept(preview);
        }
        tick = 120;
    }
    let pending = Arc::new(AtomicBool::new(false));
    loop {
        if destination.is_destroyed() {
            break;
        }
        if Instant::now() >= next {
            if !state.paused && !state.elevated {
                let mut snapshot = if let Some(s) = session.as_mut() {
                    s.sample()
                } else {
                    demo::snapshot(tick)
                };
                if let Some(capture) = &imported {
                    snapshot.firmware = Some(capture.clone());
                }
                state.accept(snapshot);
                tick += 1;
                dirty = true;
            }
            next = Instant::now() + Duration::from_secs(1);
        }
        if dirty {
            dirty = !publish(&state, &destination, &pending);
        }
        let command = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(c) => c,
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        dirty = true;
        match command {
            Command::Stop => break,
            Command::Pause(value) => {
                state.paused = value;
                state.message(
                    if value {
                        "Display updates paused"
                    } else {
                        "Live updates resumed"
                    },
                    false,
                );
                if !value {
                    next = Instant::now();
                }
            }
            Command::Connect if !demo_mode => {
                if helper.is_some() {
                    continue;
                }
                generation += 1;
                match launch(tx.clone(), generation) {
                    Ok(connection) => {
                        helper = Some(connection);
                        state.authorizing = true;
                        state.message("Waiting for system authorization…", false);
                    }
                    Err(e) => state.message(e, true),
                }
            }
            Command::Disconnect => {
                generation += 1;
                helper = None;
                state.elevated = false;
                state.authorizing = false;
                next = Instant::now();
                state.message("Hardware access disconnected", false);
            }
            Command::Refresh => {
                if let Some(h) = helper.as_mut() {
                    if let Err(e) = h.send(&HelperCommand::RefreshMemory {}) {
                        state.message(e, true);
                    } else {
                        refresh_pending = true;
                    }
                } else if let Some(s) = session.as_mut() {
                    s.refresh_memory();
                    let mut snapshot = s.sample();
                    if let Some(f) = &imported {
                        snapshot.firmware = Some(f.clone());
                    }
                    state.accept(snapshot);
                } else {
                    state.accept(demo::snapshot(tick));
                    tick += 1;
                }
            }
            Command::Apply(level) => {
                if demo_mode {
                    state.message("Preview mode does not change your hardware", false);
                    continue;
                }
                let Some(profile) = u8::try_from(level).ok().and_then(EppProfile::from_level)
                else {
                    state.message("Unknown EPP profile", true);
                    continue;
                };
                if let Some(h) = helper.as_mut() {
                    if let Err(e) = h.send(&HelperCommand::ApplyEpp { profile }) {
                        state.message(e, true);
                    }
                } else {
                    let result = EppManager::new().and_then(|m| m.apply_profile(profile));
                    match result {
                        Ok(()) => {
                            state.message(
                                format!("Applied and verified {}", profile.as_str()),
                                false,
                            );
                            next = Instant::now();
                        }
                        Err(e) => state.message(
                            format!("{e}. Enable hardware access to authorize profile changes."),
                            true,
                        ),
                    }
                }
            }
            Command::Baseline => {
                if let Some(snapshot) = &state.latest {
                    state.baseline = Some(snapshot.clone());
                    state.message(
                        "Baseline captured. Compare it with subsequent readings below.",
                        false,
                    );
                }
            }
            Command::ClearBaseline => {
                state.baseline = None;
                state.message("Baseline cleared", false);
            }
            Command::Save(path) => {
                if let Some(snapshot) = &state.latest {
                    match snapshot.write_to(&path) {
                        Ok(()) => {
                            state.message(format!("Snapshot saved to {}", path.display()), false)
                        }
                        Err(e) => state.message(e, true),
                    }
                }
            }
            Command::ImportFirmware(path) => match AodCapture::from_path(&path) {
                Ok(capture) => {
                    imported = Some(capture.clone());
                    if let Some(snapshot) = &mut state.latest {
                        snapshot.firmware = Some(capture);
                    }
                    state.message("Firmware capture imported. These are historical settings, not live voltage readings.",false);
                }
                Err(e) => state.message(e.to_string(), true),
            },
            Command::LoadBaseline(path) => match Snapshot::from_path(&path) {
                Ok(snapshot) => {
                    if let Some(current) = &state.latest
                        && let Err(e) = zentools::monitor::compare(&snapshot, current)
                    {
                        state.message(e, true);
                        continue;
                    }
                    state.baseline = Some(snapshot);
                    state.message("Saved baseline loaded", false);
                }
                Err(e) => state.message(e, true),
            },
            Command::Remote(id, event) if id == generation => match event {
                Ok(HelperEvent::Snapshot {
                    mut snapshot,
                    memory_refreshed,
                }) => {
                    state.elevated = true;
                    state.authorizing = false;
                    if let Some(f) = &imported {
                        snapshot.firmware = Some(f.clone());
                    }
                    if !state.paused
                        || state.latest.is_none()
                        || force_snapshot
                        || (refresh_pending && memory_refreshed)
                    {
                        state.accept(*snapshot);
                        force_snapshot = false;
                        if memory_refreshed {
                            refresh_pending = false;
                        }
                    }
                }
                Ok(HelperEvent::Result { success, message }) => {
                    state.message(message, !success);
                    force_snapshot = true;
                }
                Err(e) => {
                    generation += 1;
                    helper = None;
                    state.elevated = false;
                    state.authorizing = false;
                    next = Instant::now();
                    state.message(e, true);
                }
            },
            _ => {}
        }
    }
    drop(helper);
}
