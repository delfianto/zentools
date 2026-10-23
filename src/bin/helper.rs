//! Headless, connection-scoped helper (pipes or socket on stdin/stdout).
//! No paths or arbitrary register writes are accepted.
use std::{
    io::{self, BufRead, Read, Write},
    sync::mpsc,
    time::{Duration, Instant},
};
use zentools::{
    epp::EppManager,
    monitor::{self, HelperCommand, HelperEvent, Session},
};

fn emit(event: &HelperEvent) -> io::Result<()> {
    let stdout = io::stdout();
    emit_to(&mut stdout.lock(), event)
}
fn emit_to(output: &mut impl Write, event: &HelperEvent) -> io::Result<()> {
    serde_json::to_writer(&mut *output, event)?;
    output.write_all(b"\n")?;
    output.flush()
}
fn connection_closed(error: &(dyn std::error::Error + 'static)) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|e| {
        matches!(
            e.kind(),
            io::ErrorKind::BrokenPipe | io::ErrorKind::ConnectionReset
        )
    })
}
fn main() {
    if let Err(error) = run() {
        // A client can close its socket while a snapshot is being serialized.
        // This ends that session normally, without a failed systemd unit.
        if connection_closed(error.as_ref()) {
            return;
        }
        eprintln!("ZenTools helper: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args != ["stream"] && args != ["--once"] {
        return Err("usage: zentools-helper stream | --once".into());
    }
    let mut session = Session::new();
    let firmware = if std::path::Path::new("/proc/zentools_aod_probe").exists() {
        Some(monitor::cached_firmware())
    } else {
        None
    };
    let sample = |session: &mut Session| {
        let mut snapshot = session.sample();
        if let Some(result) = &firmware {
            match result {
                Ok(f) => snapshot.firmware = Some(f.clone()),
                Err(e) => snapshot.firmware_error = Some(e.clone()),
            }
        }
        snapshot
    };
    emit(&HelperEvent::Snapshot {
        snapshot: Box::new(sample(&mut session)),
        memory_refreshed: true,
    })?;
    if args == ["--once"] {
        return Ok(());
    }
    let (tx, rx) = mpsc::sync_channel(16);
    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut input = stdin.lock();
        loop {
            let mut line = Vec::new();
            match input.by_ref().take(4097).read_until(b'\n', &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > 4096 => break,
                Ok(_) => {
                    if tx
                        .send(
                            serde_json::from_slice::<HelperCommand>(&line)
                                .map_err(|e| e.to_string()),
                        )
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });
    let interval = Duration::from_secs(1);
    let mut next = Instant::now() + interval;
    loop {
        match rx.recv_timeout(next.saturating_duration_since(Instant::now())) {
            Ok(Ok(HelperCommand::Shutdown {})) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Ok(HelperCommand::RefreshMemory {})) => {
                session.refresh_memory();
                emit(&HelperEvent::Snapshot {
                    snapshot: Box::new(sample(&mut session)),
                    memory_refreshed: true,
                })?;
            }
            Ok(Ok(HelperCommand::ApplyEpp { profile })) => {
                let result = EppManager::new().and_then(|m| m.apply_profile(profile));
                let success = result.is_ok();
                let message = match result {
                    Ok(()) => format!("Applied and verified {} on all CPUs", profile.as_str()),
                    Err(e) => e.to_string(),
                };
                emit(&HelperEvent::Result { success, message })?;
                emit(&HelperEvent::Snapshot {
                    snapshot: Box::new(sample(&mut session)),
                    memory_refreshed: false,
                })?;
            }
            Ok(Err(message)) => emit(&HelperEvent::Result {
                success: false,
                message,
            })?,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if Instant::now() >= next {
            emit(&HelperEvent::Snapshot {
                snapshot: Box::new(sample(&mut session)),
                memory_refreshed: false,
            })?;
            next = Instant::now() + interval;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn socket_disconnect_during_emit_is_a_normal_session_end() {
        let (mut writer, client) = std::os::unix::net::UnixStream::pair().unwrap();
        drop(client);
        let error = emit_to(
            &mut writer,
            &HelperEvent::Result {
                success: true,
                message: "synthetic result".into(),
            },
        )
        .unwrap_err();
        assert!(connection_closed(&error), "{error}");
        assert!(!connection_closed(&io::Error::from(
            io::ErrorKind::PermissionDenied
        )));
    }
}
