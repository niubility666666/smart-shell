use crate::models::{PtyCloseRequest, PtyInputRequest, PtyOutputRequest, PtyOutputResponse, SshProfile};
use crate::ssh::connect_ssh;
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::io::{ErrorKind, Read, Write};
use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone)]
struct PtySessionRuntime {
    input_tx: mpsc::Sender<String>,
    shutdown_tx: mpsc::Sender<()>,
    output_buffer: Arc<Mutex<String>>,
}

static PTY_SESSIONS: Lazy<Mutex<HashMap<String, PtySessionRuntime>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn push_pty_output(output: &Arc<Mutex<String>>, text: &str) {
    if text.is_empty() {
        return;
    }

    if let Ok(mut guard) = output.lock() {
        guard.push_str(text);
    }
}

fn run_pty_loop(
    profile: SshProfile,
    input_rx: mpsc::Receiver<String>,
    shutdown_rx: mpsc::Receiver<()>,
    output: Arc<Mutex<String>>,
) {
    let session = match connect_ssh(&profile) {
        Ok(value) => value,
        Err(err) => {
            push_pty_output(&output, &format!("[PTY] connect failed: {}\n", err));
            return;
        }
    };

    let mut channel = match session.channel_session() {
        Ok(value) => value,
        Err(err) => {
            push_pty_output(&output, &format!("[PTY] open channel failed: {}\n", err));
            return;
        }
    };

    if let Err(err) = channel.request_pty("xterm", None, Some((160, 48, 0, 0))) {
        push_pty_output(&output, &format!("[PTY] request pty failed: {}\n", err));
        return;
    }

    if let Err(err) = channel.shell() {
        push_pty_output(&output, &format!("[PTY] start shell failed: {}\n", err));
        return;
    }

    session.set_blocking(false);
    push_pty_output(&output, "[PTY] connected\n");

    let mut stdout_buffer = [0_u8; 8192];
    let mut stderr_buffer = [0_u8; 4096];

    loop {
        if shutdown_rx.try_recv().is_ok() {
            let _ = channel.write_all(b"exit\n");
            break;
        }

        loop {
            match input_rx.try_recv() {
                Ok(input) => {
                    if let Err(err) = channel.write_all(input.as_bytes()) {
                        push_pty_output(&output, &format!("[PTY] write failed: {}\n", err));
                        break;
                    }
                    let _ = channel.flush();
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }

        match channel.read(&mut stdout_buffer) {
            Ok(size) if size > 0 => {
                let chunk = String::from_utf8_lossy(&stdout_buffer[..size]).to_string();
                push_pty_output(&output, &chunk);
            }
            Ok(_) => {}
            Err(err) => {
                if err.kind() != ErrorKind::WouldBlock {
                    push_pty_output(&output, &format!("[PTY] stdout read failed: {}\n", err));
                    break;
                }
            }
        }

        match channel.stderr().read(&mut stderr_buffer) {
            Ok(size) if size > 0 => {
                let chunk = String::from_utf8_lossy(&stderr_buffer[..size]).to_string();
                push_pty_output(&output, &chunk);
            }
            Ok(_) => {}
            Err(err) => {
                if err.kind() != ErrorKind::WouldBlock {
                    push_pty_output(&output, &format!("[PTY] stderr read failed: {}\n", err));
                    break;
                }
            }
        }

        if channel.eof() {
            push_pty_output(&output, "[PTY] remote session ended\n");
            break;
        }

        thread::sleep(Duration::from_millis(35));
    }

    let _ = channel.close();
    let _ = channel.wait_close();
}

#[tauri::command]
pub async fn open_pty_session(profile: SshProfile) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session_id = Uuid::new_v4().to_string();
        let (input_tx, input_rx) = mpsc::channel::<String>();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
        let output_buffer = Arc::new(Mutex::new(String::new()));

        let thread_profile = profile.clone();
        let thread_output = Arc::clone(&output_buffer);
        thread::spawn(move || run_pty_loop(thread_profile, input_rx, shutdown_rx, thread_output));

        let runtime = PtySessionRuntime {
            input_tx,
            shutdown_tx,
            output_buffer,
        };

        let mut map = PTY_SESSIONS
            .lock()
            .map_err(|_| "PTY session map lock failed".to_string())?;
        map.insert(session_id.clone(), runtime);

        Ok(session_id)
    })
    .await
    .map_err(|err| format!("Open PTY worker failed: {}", err))?
}

#[tauri::command]
pub fn send_pty_input(request: PtyInputRequest) -> Result<String, String> {
    let map = PTY_SESSIONS
        .lock()
        .map_err(|_| "PTY session map lock failed".to_string())?;

    let runtime = map
        .get(&request.session_id)
        .ok_or_else(|| "PTY session not found".to_string())?;

    runtime
        .input_tx
        .send(request.input)
        .map_err(|err| format!("Send PTY input failed: {}", err))?;

    Ok("ok".to_string())
}

#[tauri::command]
pub fn read_pty_output(request: PtyOutputRequest) -> Result<PtyOutputResponse, String> {
    let map = PTY_SESSIONS
        .lock()
        .map_err(|_| "PTY session map lock failed".to_string())?;

    let runtime = map
        .get(&request.session_id)
        .ok_or_else(|| "PTY session not found".to_string())?;

    let mut buffer = runtime
        .output_buffer
        .lock()
        .map_err(|_| "PTY output buffer lock failed".to_string())?;

    let output = buffer.clone();
    buffer.clear();

    Ok(PtyOutputResponse { output })
}

#[tauri::command]
pub fn close_pty_session(request: PtyCloseRequest) -> Result<String, String> {
    let mut map = PTY_SESSIONS
        .lock()
        .map_err(|_| "PTY session map lock failed".to_string())?;

    if let Some(runtime) = map.remove(&request.session_id) {
        let _ = runtime.shutdown_tx.send(());
        Ok("PTY session closed".to_string())
    } else {
        Ok("PTY session not found".to_string())
    }
}
