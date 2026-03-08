#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod models;
mod ssh;

use commands::{
    audit_shell_command,
    chat_with_ai,
    clear_hosts_secure,
    close_pty_session,
    create_remote_dir,
    delete_remote_path,
    download_remote_file,
    list_remote_dir,
    load_hosts_secure,
    open_pty_session,
    read_pty_output,
    read_remote_file,
    rename_remote_path,
    run_remote_command,
    save_hosts_secure,
    send_pty_input,
    test_ssh_connection,
    upload_remote_file,
    write_remote_file,
};

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            test_ssh_connection,
            run_remote_command,
            list_remote_dir,
            read_remote_file,
            write_remote_file,
            upload_remote_file,
            download_remote_file,
            rename_remote_path,
            delete_remote_path,
            create_remote_dir,
            open_pty_session,
            send_pty_input,
            read_pty_output,
            close_pty_session,
            save_hosts_secure,
            load_hosts_secure,
            clear_hosts_secure,
            audit_shell_command,
            chat_with_ai
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri app");
}
