use crate::models::SshProfile;
use ssh2::Session;
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

pub fn connect_ssh(profile: &SshProfile) -> Result<Session, String> {
    let address = format!("{}:{}", profile.host, profile.port);
    let tcp = TcpStream::connect(&address)
        .map_err(|err| format!("SSH connect failed {}: {}", address, err))?;

    tcp.set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|err| format!("Set read timeout failed: {}", err))?;
    tcp.set_write_timeout(Some(Duration::from_secs(15)))
        .map_err(|err| format!("Set write timeout failed: {}", err))?;

    let mut session = Session::new().map_err(|err| format!("Create SSH session failed: {}", err))?;
    session.set_tcp_stream(tcp);
    session
        .handshake()
        .map_err(|err| format!("SSH handshake failed: {}", err))?;

    match profile.auth_type.as_str() {
        "password" => {
            let password = profile
                .password
                .clone()
                .ok_or_else(|| "Missing password for SSH auth".to_string())?;
            session
                .userauth_password(&profile.username, &password)
                .map_err(|err| format!("Password auth failed: {}", err))?;
        }
        "key" => {
            let key_path = profile
                .key_path
                .clone()
                .ok_or_else(|| "Missing private key path for SSH auth".to_string())?;
            let passphrase = profile.passphrase.clone().filter(|value| !value.is_empty());
            session
                .userauth_pubkey_file(
                    &profile.username,
                    None,
                    Path::new(&key_path),
                    passphrase.as_deref(),
                )
                .map_err(|err| format!("Key auth failed: {}", err))?;
        }
        other => return Err(format!("Unsupported auth type: {}", other)),
    }

    if !session.authenticated() {
        return Err("SSH authentication rejected by remote".to_string());
    }

    Ok(session)
}
