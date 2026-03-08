pub mod ai;
pub mod pty;
pub mod security;
pub mod sftp;
pub mod ssh;

pub use ai::chat_with_ai;
pub use pty::{close_pty_session, open_pty_session, read_pty_output, send_pty_input};
pub use security::{audit_shell_command, clear_hosts_secure, load_hosts_secure, save_hosts_secure};
pub use sftp::{
    create_remote_dir,
    delete_remote_path,
    download_remote_file,
    list_remote_dir,
    read_remote_file,
    rename_remote_path,
    upload_remote_file,
    write_remote_file,
};
pub use ssh::{run_remote_command, test_ssh_connection};
