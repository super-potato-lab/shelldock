mod storage;
use serde::Serialize;
use shelldock_core::{
    compatibility,
    metrics::{Metrics, Tracker},
    model::{Connection, FileEntry, FileRevision, RemoteText, SessionEvent, TransferProgress},
    ssh::{Credentials, OpenResult, RemoteSession},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use storage::{Persistent, Store};
use tauri::{ipc::Channel, AppHandle, Emitter, Manager, State};
use tokio::sync::{Mutex as AsyncMutex, Notify, Semaphore};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

struct Bridge {
    pending: AtomicUsize,
    notify: Notify,
}
struct Live {
    remote: Arc<RemoteSession>,
    tracker: AsyncMutex<Tracker>,
    bridge: Arc<Bridge>,
}
struct AppState {
    store: Mutex<Store>,
    sessions: AsyncMutex<HashMap<String, Arc<Live>>>,
    secrets: Mutex<HashMap<String, Zeroizing<String>>>,
    transfers: AsyncMutex<HashMap<String, CancellationToken>>,
    transfer_slots: Arc<Semaphore>,
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn profile(state: &AppState, id: &str) -> Result<Connection, String> {
    state
        .store
        .lock()
        .map_err(err)?
        .data
        .connections
        .iter()
        .find(|p| p.id == id)
        .cloned()
        .ok_or_else(|| "连接不存在".into())
}
async fn live(state: &AppState, id: &str) -> Result<Arc<Live>, String> {
    state
        .sessions
        .lock()
        .await
        .get(id)
        .cloned()
        .ok_or_else(|| "会话已关闭".into())
}
fn secret(state: &AppState, id: &str) -> Option<Zeroizing<String>> {
    if let Ok(map) = state.secrets.lock() {
        if let Some(s) = map.get(id) {
            return Some(s.clone());
        }
    }
    keyring::Entry::new("ShellDock", id)
        .ok()?
        .get_password()
        .ok()
        .map(Zeroizing::new)
}
fn remember_secret(
    state: &AppState,
    id: &str,
    password: String,
    remember: bool,
) -> Result<(), String> {
    if remember {
        keyring::Entry::new("ShellDock", id)
            .map_err(err)?
            .set_password(&password)
            .map_err(|_| "系统凭据服务不可用。取消“保存密码”后可仅在本次运行使用。".to_string())?;
    }
    state
        .secrets
        .lock()
        .map_err(err)?
        .insert(id.into(), Zeroizing::new(password));
    Ok(())
}

#[tauri::command]
fn bootstrap(state: State<AppState>) -> Result<Persistent, String> {
    let mut d = state.store.lock().map_err(err)?.data.clone();
    for c in &mut d.connections {
        c.has_password = secret(&state, &c.id).is_some();
    }
    Ok(d)
}
#[tauri::command]
fn save_connection(
    state: State<AppState>,
    mut connection: Connection,
    password: Option<String>,
    remember: bool,
) -> Result<Connection, String> {
    connection.validate().map_err(err)?;
    compatibility::scrub_secrets(&mut connection.extra);
    if connection.id.is_empty() {
        connection.id = uuid::Uuid::new_v4().to_string();
    }
    if let Some(password) = password {
        remember_secret(&state, &connection.id, password, remember)?;
    }
    connection.has_password = secret(&state, &connection.id).is_some();
    let mut store = state.store.lock().map_err(err)?;
    if let Some(old) = store
        .data
        .connections
        .iter_mut()
        .find(|x| x.id == connection.id)
    {
        *old = connection.clone();
    } else {
        store.data.connections.push(connection.clone());
    }
    store.save().map_err(err)?;
    Ok(connection)
}
#[tauri::command]
fn delete_connection(state: State<AppState>, id: String) -> Result<(), String> {
    let mut store = state.store.lock().map_err(err)?;
    store.data.connections.retain(|x| x.id != id);
    store.save().map_err(err)?;
    if let Ok(mut s) = state.secrets.lock() {
        s.remove(&id);
    }
    if let Ok(e) = keyring::Entry::new("ShellDock", &id) {
        let _ = e.delete_credential();
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResponse {
    connections: Vec<Connection>,
    warnings: Vec<String>,
    imported: usize,
}
#[tauri::command]
async fn import_connections(app: AppHandle, paths: Vec<String>) -> Result<ImportResponse, String> {
    tokio::task::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let mut connections = vec![];
        let mut warnings = vec![];
        for path in paths {
            match compatibility::import_path(&PathBuf::from(path)) {
                Ok(report) => {
                    warnings.extend(report.warnings);
                    for mut item in report.items {
                        if let Some(password) = item.password {
                            let id = item.connection.id.clone();
                            if remember_secret(&state, &id, password.to_string(), true).is_err() {
                                remember_secret(&state, &id, password.to_string(), false)?;
                                warnings.push(format!(
                                    "{}：密码仅保留到本次退出，系统凭据服务不可用",
                                    item.connection.name
                                ));
                            }
                            item.connection.has_password = true;
                        }
                        connections.push(item.connection);
                    }
                }
                Err(e) => warnings.push(e.to_string()),
            }
        }
        if connections.is_empty() {
            return Err(warnings.join("；"));
        }
        let imported = connections.len();
        let mut store = state.store.lock().map_err(err)?;
        store.data.connections.extend(connections.clone());
        store.save().map_err(err)?;
        Ok(ImportResponse {
            connections,
            warnings,
            imported,
        })
    })
    .await
    .map_err(err)?
}
#[tauri::command]
async fn export_connections(
    app: AppHandle,
    path: String,
    format: String,
    ids: Vec<String>,
    include_passwords: bool,
) -> Result<usize, String> {
    tokio::task::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let selected: Vec<Connection> = state
            .store
            .lock()
            .map_err(err)?
            .data
            .connections
            .iter()
            .filter(|c| ids.is_empty() || ids.contains(&c.id))
            .cloned()
            .collect();
        if selected.is_empty() {
            return Err("没有可导出的连接".into());
        }
        let value = if format == "finalshell" {
            if selected.len() != 1 {
                return Err("FinalShell JSON 每次导出一个连接，请先选中连接".into());
            }
            let password = if include_passwords {
                secret(&state, &selected[0].id)
            } else {
                None
            };
            compatibility::export_finalshell(&selected[0], password.as_deref().map(|p| p.as_str()))
                .map_err(err)?
        } else if format == "shelldock" {
            compatibility::export_native(&selected)
        } else {
            return Err("未知导出格式".into());
        };
        storage::write_atomic(
            &PathBuf::from(path),
            &serde_json::to_vec_pretty(&value).map_err(err)?,
        )
        .map_err(err)?;
        Ok(selected.len())
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
#[serde(
    tag = "status",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ConnectResponse {
    Connected { session_id: String, path: String },
    NeedsTrust { fingerprint: String, changed: bool },
}
#[tauri::command]
async fn connect(
    app: AppHandle,
    id: String,
    password: Option<String>,
    columns: u32,
    rows: u32,
    on_event: Channel<SessionEvent>,
) -> Result<ConnectResponse, String> {
    let state = app.state::<AppState>();
    let profile = profile(&state, &id)?;
    let expected = state
        .store
        .lock()
        .map_err(err)?
        .data
        .known_hosts
        .get(&format!("{}:{}", profile.host, profile.port))
        .cloned();
    let pass = if let Some(p) = password {
        Some(Zeroizing::new(p))
    } else {
        let app = app.clone();
        let id = id.clone();
        tokio::task::spawn_blocking(move || secret(&app.state::<AppState>(), &id))
            .await
            .map_err(err)?
    };
    let creds = if profile.auth == "key" {
        Credentials {
            password: None,
            passphrase: pass,
        }
    } else {
        Credentials {
            password: pass,
            passphrase: None,
        }
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(32);
    match RemoteSession::open(&profile, creds, expected, columns, rows, tx)
        .await
        .map_err(err)?
    {
        OpenResult::NeedsTrust {
            fingerprint,
            changed,
        } => Ok(ConnectResponse::NeedsTrust {
            fingerprint,
            changed,
        }),
        OpenResult::Connected(remote) => {
            let session_id = uuid::Uuid::new_v4().to_string();
            let path = remote
                .canonicalize(".")
                .await
                .unwrap_or_else(|_| "/".into());
            let bridge = Arc::new(Bridge {
                pending: AtomicUsize::new(0),
                notify: Notify::new(),
            });
            state.sessions.lock().await.insert(
                session_id.clone(),
                Arc::new(Live {
                    remote: remote.clone(),
                    tracker: AsyncMutex::new(Tracker::default()),
                    bridge: bridge.clone(),
                }),
            );
            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    if let SessionEvent::Output(ref data) = event {
                        while bridge.pending.load(Ordering::Relaxed) > 1024 * 1024 {
                            tokio::select! {_ = remote.cancel.cancelled()=>{let _=on_event.send(SessionEvent::Closed);return;},r=tokio::time::timeout(std::time::Duration::from_secs(20),bridge.notify.notified())=>if r.is_err(){let _=on_event.send(SessionEvent::Error("终端消费超时，会话已停止".into()));let _=remote.close().await;return;}}
                        }
                        bridge.pending.fetch_add(data.len(), Ordering::Relaxed);
                    }
                    if on_event.send(event).is_err() {
                        let _ = remote.close().await;
                        break;
                    }
                }
            });
            Ok(ConnectResponse::Connected { session_id, path })
        }
    }
}
#[tauri::command]
fn trust_host(state: State<AppState>, id: String, fingerprint: String) -> Result<(), String> {
    let p = profile(&state, &id)?;
    if !fingerprint.starts_with("SHA256:") {
        return Err("无效主机指纹".into());
    }
    let mut store = state.store.lock().map_err(err)?;
    store
        .data
        .known_hosts
        .insert(format!("{}:{}", p.host, p.port), fingerprint);
    store.save().map_err(err)
}
#[tauri::command]
async fn terminal_input(
    state: State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    live(&state, &session_id)
        .await?
        .remote
        .input(data)
        .await
        .map_err(err)
}
#[tauri::command]
async fn terminal_resize(
    state: State<'_, AppState>,
    session_id: String,
    columns: u32,
    rows: u32,
) -> Result<(), String> {
    live(&state, &session_id)
        .await?
        .remote
        .resize(columns, rows)
        .await
        .map_err(err)
}
#[tauri::command]
async fn terminal_ack(
    state: State<'_, AppState>,
    session_id: String,
    bytes: usize,
) -> Result<(), String> {
    let l = live(&state, &session_id).await?;
    let _ = l
        .bridge
        .pending
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |p| {
            Some(p.saturating_sub(bytes))
        });
    l.bridge.notify.notify_one();
    Ok(())
}
#[tauri::command]
async fn disconnect(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    let l = state.sessions.lock().await.remove(&session_id);
    if let Some(l) = l {
        l.remote.close().await.map_err(err)?;
    }
    Ok(())
}
#[tauri::command]
async fn list_files(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<Vec<FileEntry>, String> {
    live(&state, &session_id)
        .await?
        .remote
        .list(&path)
        .await
        .map_err(err)
}
#[tauri::command]
async fn file_action(
    state: State<'_, AppState>,
    session_id: String,
    action: String,
    path: String,
    target: Option<String>,
) -> Result<(), String> {
    let r = live(&state, &session_id).await?.remote.clone();
    match action.as_str() {
        "mkdir" => r.mkdir(&path).await,
        "rename" => {
            r.rename(&path, target.as_deref().ok_or("缺少目标名称")?)
                .await
        }
        "delete" => r.delete(&path).await,
        "chmod" => {
            r.chmod(
                &path,
                u32::from_str_radix(target.as_deref().unwrap_or_default(), 8).map_err(err)?,
            )
            .await
        }
        _ => return Err("未知文件操作".into()),
    }
    .map_err(err)
}
#[tauri::command]
async fn read_remote_text(
    state: State<'_, AppState>,
    session_id: String,
    path: String,
) -> Result<RemoteText, String> {
    live(&state, &session_id)
        .await?
        .remote
        .read_text(&path)
        .await
        .map_err(err)
}
#[tauri::command]
async fn save_remote_text(
    state: State<'_, AppState>,
    session_id: String,
    text: RemoteText,
) -> Result<FileRevision, String> {
    live(&state, &session_id)
        .await?
        .remote
        .save_text(&text)
        .await
        .map_err(err)
}
#[tauri::command]
async fn sample_metrics(state: State<'_, AppState>, session_id: String) -> Result<Metrics, String> {
    let l = live(&state, &session_id).await?;
    let mut tracker = l.tracker.lock().await;
    let output = l
        .remote
        .exec(shelldock_core::metrics::COMMAND)
        .await
        .map_err(err)?;
    tracker.sample(&output).map_err(err)
}

#[tauri::command]
async fn start_transfer(
    app: AppHandle,
    session_id: String,
    direction: String,
    local: String,
    remote: String,
) -> Result<String, String> {
    if !matches!(direction.as_str(), "upload" | "download") {
        return Err("未知传输方向".into());
    }
    let state = app.state::<AppState>();
    let session = live(&state, &session_id).await?.remote.clone();
    let id = uuid::Uuid::new_v4().to_string();
    let cancel = CancellationToken::new();
    state
        .transfers
        .lock()
        .await
        .insert(id.clone(), cancel.clone());
    let slots = state.transfer_slots.clone();
    let task_id = id.clone();
    let queued = TransferProgress {
        id: id.clone(),
        name: PathBuf::from(&local)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into(),
        direction: direction.clone(),
        transferred: 0,
        total: 0,
        status: "queued".into(),
        error: None,
    };
    let _ = app.emit_to("main", "transfer-progress", queued);
    tokio::spawn(async move {
        let permit = tokio::select! {_ = cancel.cancelled()=>None,p=slots.acquire_owned()=>p.ok()};
        if let Some(_permit) = permit {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<TransferProgress>(32);
            let event_app = app.clone();
            let progress_task = tokio::spawn(async move {
                let mut finalized = false;
                while let Some(event) = rx.recv().await {
                    finalized |=
                        matches!(event.status.as_str(), "completed" | "failed" | "cancelled");
                    let _ = event_app.emit_to("main", "transfer-progress", event);
                }
                finalized
            });
            let outcome = session
                .transfer(
                    task_id.clone(),
                    &direction,
                    &PathBuf::from(local),
                    &remote,
                    cancel.clone(),
                    tx,
                )
                .await;
            let finalized = progress_task.await.unwrap_or(false);
            if let Some(e) = outcome.err().filter(|_| !finalized) {
                let _ = app.emit_to(
                    "main",
                    "transfer-progress",
                    TransferProgress {
                        id: task_id.clone(),
                        name: remote,
                        direction,
                        transferred: 0,
                        total: 0,
                        status: if cancel.is_cancelled() {
                            "cancelled"
                        } else {
                            "failed"
                        }
                        .into(),
                        error: Some(e.to_string()),
                    },
                );
            }
        } else {
            let _ = app.emit_to(
                "main",
                "transfer-progress",
                TransferProgress {
                    id: task_id.clone(),
                    name: remote,
                    direction,
                    transferred: 0,
                    total: 0,
                    status: "cancelled".into(),
                    error: None,
                },
            );
        }
        app.state::<AppState>()
            .transfers
            .lock()
            .await
            .remove(&task_id);
    });
    Ok(id)
}
#[tauri::command]
async fn cancel_transfer(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if let Some(c) = state.transfers.lock().await.get(&id) {
        c.cancel();
    }
    Ok(())
}
#[tauri::command]
fn record_command(state: State<AppState>, command: String) -> Result<(), String> {
    if command.trim().is_empty() {
        return Ok(());
    }
    let mut store = state.store.lock().map_err(err)?;
    store.data.history.retain(|c| c != &command);
    store.data.history.insert(0, command);
    store.data.history.truncate(500);
    store.save().map_err(err)
}
#[tauri::command]
fn save_quick_commands(state: State<AppState>, commands: Vec<String>) -> Result<(), String> {
    let mut store = state.store.lock().map_err(err)?;
    store.data.quick_commands = commands;
    store.save().map_err(err)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let store = Store::open(app.path().app_data_dir()?.join("connections.json"))?;
            app.manage(AppState {
                store: Mutex::new(store),
                sessions: AsyncMutex::new(HashMap::new()),
                secrets: Mutex::new(HashMap::new()),
                transfers: AsyncMutex::new(HashMap::new()),
                transfer_slots: Arc::new(Semaphore::new(3)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            save_connection,
            delete_connection,
            import_connections,
            export_connections,
            connect,
            trust_host,
            terminal_input,
            terminal_resize,
            terminal_ack,
            disconnect,
            list_files,
            file_action,
            read_remote_text,
            save_remote_text,
            sample_metrics,
            start_transfer,
            cancel_transfer,
            record_command,
            save_quick_commands
        ])
        .build(tauri::generate_context!())
        .expect("failed to build ShellDock")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let state = app.state::<AppState>();
                if let Ok(live) = state.sessions.try_lock() {
                    for s in live.values() {
                        s.remote.cancel.cancel();
                    }
                }
                if let Ok(transfers) = state.transfers.try_lock() {
                    for c in transfers.values() {
                        c.cancel();
                    }
                };
            }
        });
}
