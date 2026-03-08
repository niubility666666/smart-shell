use crate::models::{
    DeletePathRequest,
    DownloadFileResponse,
    FileRequest,
    ListDirRequest,
    MkdirRequest,
    RemoteEntry,
    RenamePathRequest,
    UploadFileRequest,
    WriteFileRequest,
};
use crate::ssh::connect_ssh;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine;
use std::io::{Read, Write};
use std::path::Path;

#[tauri::command]
pub async fn list_remote_dir(request: ListDirRequest) -> Result<Vec<RemoteEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let target = if request.path.trim().is_empty() {
            request
                .profile
                .base_path
                .clone()
                .unwrap_or_else(|| ".".to_string())
        } else {
            request.path
        };

        let entries = sftp
            .readdir(Path::new(&target))
            .map_err(|err| format!("Read remote directory failed: {}", err))?;

        let mut result = Vec::with_capacity(entries.len());
        for (path, stat) in entries {
            let name = path
                .file_name()
                .and_then(|item| item.to_str())
                .unwrap_or_default()
                .to_string();

            if name.is_empty() || name == "." || name == ".." {
                continue;
            }

            let perm = stat.perm.unwrap_or(0);
            let is_dir = (perm & 0o170000) == 0o040000;

            result.push(RemoteEntry {
                name,
                path: path.to_string_lossy().to_string(),
                is_dir,
                size: stat.size.unwrap_or(0),
                modified: stat.mtime.unwrap_or(0),
            });
        }

        Ok(result)
    })
    .await
    .map_err(|err| format!("List directory worker failed: {}", err))?
}

#[tauri::command]
pub async fn read_remote_file(request: FileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .open(Path::new(&request.path))
            .map_err(|err| format!("Open remote file failed: {}", err))?;

        let mut data = Vec::new();
        remote_file
            .read_to_end(&mut data)
            .map_err(|err| format!("Read remote file failed: {}", err))?;

        Ok(String::from_utf8_lossy(&data).to_string())
    })
    .await
    .map_err(|err| format!("Read file worker failed: {}", err))?
}

#[tauri::command]
pub async fn write_remote_file(request: WriteFileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .create(Path::new(&request.path))
            .map_err(|err| format!("Create remote file failed: {}", err))?;

        remote_file
            .write_all(request.content.as_bytes())
            .map_err(|err| format!("Write remote file failed: {}", err))?;

        Ok(format!("Saved: {}", request.path))
    })
    .await
    .map_err(|err| format!("Write file worker failed: {}", err))?
}

#[tauri::command]
pub async fn upload_remote_file(request: UploadFileRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let binary = BASE64_STANDARD
            .decode(request.content_base64.as_bytes())
            .map_err(|err| format!("Decode upload payload failed: {}", err))?;

        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .create(Path::new(&request.remote_path))
            .map_err(|err| format!("Create remote file failed: {}", err))?;

        remote_file
            .write_all(&binary)
            .map_err(|err| format!("Upload write failed: {}", err))?;

        Ok(format!("Uploaded: {}", request.remote_path))
    })
    .await
    .map_err(|err| format!("Upload worker failed: {}", err))?
}

#[tauri::command]
pub async fn download_remote_file(request: FileRequest) -> Result<DownloadFileResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        let mut remote_file = sftp
            .open(Path::new(&request.path))
            .map_err(|err| format!("Open remote file failed: {}", err))?;

        let mut bytes = Vec::new();
        remote_file
            .read_to_end(&mut bytes)
            .map_err(|err| format!("Download read failed: {}", err))?;

        let name = Path::new(&request.path)
            .file_name()
            .and_then(|item| item.to_str())
            .unwrap_or("download.bin")
            .to_string();

        Ok(DownloadFileResponse {
            name,
            data_base64: BASE64_STANDARD.encode(bytes),
        })
    })
    .await
    .map_err(|err| format!("Download worker failed: {}", err))?
}

#[tauri::command]
pub async fn rename_remote_path(request: RenamePathRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        sftp.rename(Path::new(&request.old_path), Path::new(&request.new_path), None)
            .map_err(|err| format!("Rename failed: {}", err))?;

        Ok(format!("Renamed: {} -> {}", request.old_path, request.new_path))
    })
    .await
    .map_err(|err| format!("Rename worker failed: {}", err))?
}

#[tauri::command]
pub async fn delete_remote_path(request: DeletePathRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        if request.is_dir {
            sftp.rmdir(Path::new(&request.path))
                .map_err(|err| format!("Delete directory failed: {}", err))?;
        } else {
            sftp.unlink(Path::new(&request.path))
                .map_err(|err| format!("Delete file failed: {}", err))?;
        }

        Ok(format!("Deleted: {}", request.path))
    })
    .await
    .map_err(|err| format!("Delete worker failed: {}", err))?
}

#[tauri::command]
pub async fn create_remote_dir(request: MkdirRequest) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let session = connect_ssh(&request.profile)?;
        let sftp = session
            .sftp()
            .map_err(|err| format!("Open SFTP channel failed: {}", err))?;

        sftp.mkdir(Path::new(&request.path), 0o755)
            .map_err(|err| format!("Create directory failed: {}", err))?;

        Ok(format!("Directory created: {}", request.path))
    })
    .await
    .map_err(|err| format!("Create directory worker failed: {}", err))?
}
