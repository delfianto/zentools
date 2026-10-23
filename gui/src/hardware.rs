//! Hardware IPC, independent of Qt. Prefer the system service when installed.
use std::{
    io::{BufRead, BufReader, Read, Write},
    process::{Child, Command, Stdio},
    thread::{self, JoinHandle},
    time::Duration,
};
use zentools::monitor::{HelperCommand, HelperEvent};

#[cfg(target_os = "linux")]
use std::os::unix::net::UnixStream;

#[cfg(target_os = "linux")]
const SOCKET_PATH: &str = "/run/zentools/helper.sock";
const MAX_RESPONSE: u64 = 8 * 1024 * 1024;
const MAX_DIAGNOSTICS: usize = 8192;

pub struct Connection {
    child: Option<Child>,
    input: Option<Box<dyn Write + Send>>,
    reader: Option<JoinHandle<()>>,
    #[cfg(target_os = "linux")]
    socket: Option<UnixStream>,
}

impl Connection {
    pub fn connect(
        report: impl Fn(Result<HelperEvent, String>) + Send + 'static,
    ) -> Result<Self, String> {
        #[cfg(target_os = "linux")]
        match UnixStream::connect(SOCKET_PATH) {
            Ok(socket) => return Self::from_socket(socket, report),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(format!(
                    "Cannot connect to the ZenTools system service: {error}. Check zentools.socket and your zentools group membership (log out and back in after joining)."
                ));
            }
        }

        let executable = std::env::current_exe()
            .map_err(|e| e.to_string())?
            .with_file_name("zentools-helper");
        if !executable.is_file() {
            return Err(format!(
                "Hardware helper is missing at {}. Build both GUI and helper with just build-gui, or install the system service with just install-service.",
                executable.display()
            ));
        }
        let mut command = Command::new("pkexec");
        command
            .arg("--disable-internal-agent")
            .arg(executable)
            .arg("stream");
        Self::from_process(command, report)
    }

    #[cfg(target_os = "linux")]
    fn from_socket(
        socket: UnixStream,
        report: impl Fn(Result<HelperEvent, String>) + Send + 'static,
    ) -> Result<Self, String> {
        socket
            .set_write_timeout(Some(Duration::from_millis(200)))
            .map_err(|e| e.to_string())?;
        let input = socket.try_clone().map_err(|e| e.to_string())?;
        let output = socket.try_clone().map_err(|e| e.to_string())?;
        let reader = thread::spawn(move || {
            let error = read_events(output, &report);
            report(Err(format!(
                "{error}. Check the ZenTools service journal (journalctl -u 'zentools@*')."
            )));
        });
        Ok(Self {
            child: None,
            input: Some(Box::new(input)),
            reader: Some(reader),
            socket: Some(socket),
        })
    }

    fn from_process(
        mut command: Command,
        report: impl Fn(Result<HelperEvent, String>) + Send + 'static,
    ) -> Result<Self, String> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Cannot open system authorization: {e}"))?;
        let input = child.stdin.take().expect("piped helper input");
        let output = child.stdout.take().expect("piped helper output");
        let diagnostics = child.stderr.take().expect("piped helper diagnostics");
        let reader = thread::spawn(move || {
            // Drain concurrently so a full stderr pipe cannot stall authorization.
            let diagnostics = thread::spawn(move || read_diagnostics(diagnostics));
            let error = read_events(output, &report);
            let diagnostics = diagnostics.join().unwrap_or_default();
            report(Err(if diagnostics.is_empty() {
                format!(
                    "{error}. Authorization may have been cancelled; check that a desktop polkit authentication agent is running."
                )
            } else {
                format!("{error}: {diagnostics}")
            }));
        });
        Ok(Self {
            child: Some(child),
            input: Some(Box::new(input)),
            reader: Some(reader),
            #[cfg(target_os = "linux")]
            socket: None,
        })
    }

    pub fn is_service(&self) -> bool {
        self.child.is_none()
    }

    pub fn send(&mut self, command: &HelperCommand) -> Result<(), String> {
        let input = self.input.as_mut().ok_or("Hardware connection is closed")?;
        serde_json::to_writer(&mut *input, command).map_err(|e| e.to_string())?;
        input
            .write_all(b"\n")
            .and_then(|_| input.flush())
            .map_err(|e| e.to_string())
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(socket) = &self.socket {
            // Wake the reader even if the service has stopped sending data.
            let _ = socket.shutdown(std::net::Shutdown::Both);
        }
        // EOF terminates only this session, including elevated children we cannot signal.
        self.input.take();
        if let Some(child) = self.child.as_mut() {
            for _ in 0..20 {
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn read_events(output: impl Read, report: impl Fn(Result<HelperEvent, String>)) -> String {
    let mut output = BufReader::new(output);
    loop {
        let mut line = Vec::new();
        match output
            .by_ref()
            .take(MAX_RESPONSE + 1)
            .read_until(b'\n', &mut line)
        {
            Ok(0) => return "Hardware connection ended".into(),
            Ok(_) if line.len() as u64 > MAX_RESPONSE => {
                return "Helper response exceeded the size limit".into();
            }
            Ok(_) => match serde_json::from_slice(&line) {
                Ok(event) => report(Ok(event)),
                Err(error) => return format!("Invalid hardware response: {error}"),
            },
            Err(error) => return format!("Cannot read hardware response: {error}"),
        }
    }
}

fn read_diagnostics(mut input: impl Read) -> String {
    let mut tail = Vec::new();
    let mut buffer = [0; 1024];
    loop {
        match input.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                tail.extend_from_slice(&buffer[..count]);
                if tail.len() > MAX_DIAGNOSTICS {
                    tail.drain(..tail.len() - MAX_DIAGNOSTICS);
                }
            }
        }
    }
    String::from_utf8_lossy(&tail).trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    #[cfg(unix)]
    fn authorization_failure_preserves_stderr() {
        let (tx, rx) = mpsc::channel();
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "printf '%131072s' '' >&2; echo 'No authentication agent found' >&2; exit 127",
        ]);
        let connection = Connection::from_process(command, move |event| {
            tx.send(event).unwrap();
        })
        .unwrap();
        let error = rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err();
        assert!(error.contains("No authentication agent found"), "{error}");
        drop(connection);
    }

    #[test]
    fn diagnostics_are_drained_with_bounded_storage() {
        let input = vec![b'x'; MAX_DIAGNOSTICS * 3];
        assert_eq!(read_diagnostics(input.as_slice()).len(), MAX_DIAGNOSTICS);
    }

    #[test]
    fn malformed_and_oversized_events_end_the_connection() {
        let reject = |_| panic!("invalid event must not be published");
        assert!(read_events(&b"invalid\n"[..], reject).contains("Invalid hardware response"));
        let input = vec![b'x'; MAX_RESPONSE as usize + 1];
        assert!(read_events(input.as_slice(), reject).contains("size limit"));
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn socket_protocol_and_disconnect_do_not_require_privilege() {
        let (client, mut server) = UnixStream::pair().unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let (tx, rx) = mpsc::channel();
        let mut connection = Connection::from_socket(client, move |event| {
            let _ = tx.send(event);
        })
        .unwrap();
        assert!(connection.is_service());
        connection.send(&HelperCommand::RefreshMemory {}).unwrap();
        let mut command = String::new();
        BufReader::new(server.try_clone().unwrap())
            .read_line(&mut command)
            .unwrap();
        assert!(matches!(
            serde_json::from_str(&command).unwrap(),
            HelperCommand::RefreshMemory {}
        ));
        server
            .write_all(b"{\"event\":\"result\",\"success\":true,\"message\":\"refreshed\"}\n")
            .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap(),
            HelperEvent::Result { success: true, .. }
        ));
        // The peer remains open and silent: local shutdown must wake the reader.
        drop(connection);
        assert_eq!(server.read(&mut [0; 1]).unwrap(), 0);
    }
}
