use crate::model::{
    Connection, FileEntry, FileRevision, RemoteText, SessionEvent, TransferProgress,
};
use anyhow::{anyhow, ensure, Result};
use russh::{
    client,
    keys::{HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate},
    ChannelMsg, Disconnect,
};
use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

pub struct Credentials {
    pub password: Option<Zeroizing<String>>,
    pub passphrase: Option<Zeroizing<String>>,
}
struct Client {
    expected: Option<String>,
    observed: Arc<Mutex<Option<String>>>,
}
impl client::Handler for Client {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let fingerprint = key.public_key().fingerprint(HashAlg::Sha256).to_string();
        if let Ok(mut observed) = self.observed.lock() {
            *observed = Some(fingerprint.clone());
        }
        // No automatic trust, including for the first connection.
        Ok(self.expected.as_deref() == Some(fingerprint.as_str()))
    }
}

pub enum OpenResult {
    Connected(Arc<RemoteSession>),
    NeedsTrust { fingerprint: String, changed: bool },
}
pub enum TerminalCommand {
    Input(Vec<u8>),
    Resize { columns: u32, rows: u32 },
}
pub struct RemoteSession {
    handle: Arc<client::Handle<Client>>,
    pub cancel: CancellationToken,
    input: mpsc::Sender<TerminalCommand>,
}

impl RemoteSession {
    pub async fn open(
        profile: &Connection,
        creds: Credentials,
        expected: Option<String>,
        columns: u32,
        rows: u32,
        events: mpsc::Sender<SessionEvent>,
    ) -> Result<OpenResult> {
        profile.validate()?;
        ensure!(profile.protocol == "ssh", "此连接类型尚未实现，只支持 SSH");
        let proxy = profile
            .extra
            .get("proxy_id")
            .and_then(|x| x.as_str())
            .unwrap_or_default();
        ensure!(
            proxy.is_empty() || proxy == "0",
            "此连接引用了代理，请先在连接设置中移除未迁移的代理引用"
        );
        let observed = Arc::new(Mutex::new(None));
        let handler = Client {
            expected: expected.clone(),
            observed: observed.clone(),
        };
        let config = Arc::new(client::Config {
            keepalive_interval: Some(Duration::from_secs(20)),
            keepalive_max: 3,
            ..Default::default()
        });
        let attempt = tokio::time::timeout(
            Duration::from_secs(20),
            client::connect(config, (profile.host.as_str(), profile.port), handler),
        )
        .await?;
        let mut handle = match attempt {
            Ok(h) => h,
            Err(e) => {
                let seen = observed
                    .lock()
                    .map_err(|_| anyhow!("主机校验状态不可用"))?
                    .clone();
                if let Some(fingerprint) = seen {
                    if expected.as_deref() != Some(&fingerprint) {
                        return Ok(OpenResult::NeedsTrust {
                            fingerprint,
                            changed: expected.is_some(),
                        });
                    }
                }
                return Err(e.into());
            }
        };
        let authentication = tokio::time::timeout(Duration::from_secs(30), async {
            if profile.auth == "key" {
                let path = profile
                    .private_key_path
                    .as_ref()
                    .ok_or_else(|| anyhow!("请为此连接选择私钥文件"))?;
                let key = russh::keys::load_secret_key(
                    path,
                    creds.passphrase.as_deref().map(|p| p.as_str()),
                )?;
                let hash = handle.best_supported_rsa_hash().await?.flatten();
                Ok::<_, anyhow::Error>(
                    handle
                        .authenticate_publickey(
                            &profile.username,
                            PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                        )
                        .await?
                        .success(),
                )
            } else {
                let password = creds.password.as_deref().map(|p| p.as_str()).unwrap_or("");
                Ok(handle
                    .authenticate_password(&profile.username, password)
                    .await?
                    .success())
            }
        })
        .await??;
        ensure!(authentication, "认证失败，请检查用户名、密码或私钥");
        let handle = Arc::new(handle);
        let channel = handle.channel_open_session().await?;
        channel
            .request_pty(
                true,
                "xterm-256color",
                columns.max(1),
                rows.max(1),
                0,
                0,
                &[],
            )
            .await?;
        channel.request_shell(true).await?;
        let cancel = CancellationToken::new();
        let task_cancel = cancel.clone();
        let (input, commands) = mpsc::channel(64);
        tokio::spawn(async move {
            let outcome =
                terminal_loop(channel, commands, events.clone(), task_cancel.clone()).await;
            if let Err(e) = outcome {
                let _ = events.send(SessionEvent::Error(e.to_string())).await;
            }
            task_cancel.cancel();
            let _ = events.send(SessionEvent::Closed).await;
        });
        Ok(OpenResult::Connected(Arc::new(Self {
            handle,
            cancel,
            input,
        })))
    }
    pub async fn input(&self, data: Vec<u8>) -> Result<()> {
        ensure!(data.len() <= 65536, "单次终端输入超过限制");
        self.input.send(TerminalCommand::Input(data)).await?;
        Ok(())
    }
    pub async fn resize(&self, columns: u32, rows: u32) -> Result<()> {
        ensure!(
            (1..=1000).contains(&columns) && (1..=1000).contains(&rows),
            "终端尺寸无效"
        );
        self.input
            .send(TerminalCommand::Resize { columns, rows })
            .await?;
        Ok(())
    }
    pub async fn close(&self) -> Result<()> {
        self.cancel.cancel();
        let _ = self
            .handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
        Ok(())
    }
    pub async fn exec(&self, command: &str) -> Result<String> {
        let mut channel = self.handle.channel_open_session().await?;
        channel.exec(true, command).await?;
        let read = async {
            let mut out = Vec::new();
            let mut stderr = Vec::new();
            let mut exit = 0;
            loop {
                tokio::select! {_ = self.cancel.cancelled()=>return Err(anyhow!("连接已关闭")), msg=channel.wait()=>match msg{
                    Some(ChannelMsg::Data{data})=>{ensure!(out.len()+data.len()<=256*1024,"命令输出超过限制");out.extend_from_slice(&data);},
                    Some(ChannelMsg::ExtendedData{data,..})=>{if stderr.len()<4096{stderr.extend_from_slice(&data[..data.len().min(4096-stderr.len())]);}},
                    Some(ChannelMsg::ExitStatus{exit_status})=>exit=exit_status,
                    Some(ChannelMsg::Failure)=>return Err(anyhow!("服务器拒绝 exec 通道请求")),
                    None|Some(ChannelMsg::Close)=>break,_=>{}
                }}
            }
            ensure!(
                exit == 0,
                "远端命令失败：{}",
                String::from_utf8_lossy(&stderr)
            );
            Ok(String::from_utf8(out)?)
        };
        let result = tokio::time::timeout(Duration::from_secs(12), read).await;
        let _ = channel.close().await;
        result?
    }
    async fn sftp(&self) -> Result<SftpSession> {
        ensure!(!self.cancel.is_cancelled(), "连接已关闭");
        let channel = self.handle.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        Ok(SftpSession::new(channel.into_stream()).await?)
    }
    pub async fn canonicalize(&self, path: &str) -> Result<String> {
        let s = self.sftp().await?;
        let result = s.canonicalize(path).await?;
        s.close().await?;
        Ok(result)
    }
    pub async fn list(&self, path: &str) -> Result<Vec<FileEntry>> {
        let s = self.sftp().await?;
        let mut out = vec![];
        for entry in s.read_dir(path).await? {
            let m = entry.metadata();
            out.push(FileEntry {
                name: entry.file_name(),
                path: entry.path(),
                is_dir: m.is_dir(),
                is_symlink: m.is_symlink(),
                size: m.size.unwrap_or(0),
                modified: m.mtime.unwrap_or(0),
                permissions: m.permissions,
                uid: m.uid,
                gid: m.gid,
            });
            ensure!(out.len() <= 50000, "目录过大，请选择更具体的路径");
        }
        s.close().await?;
        out.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    }
    pub async fn mkdir(&self, path: &str) -> Result<()> {
        let s = self.sftp().await?;
        s.create_dir(path).await?;
        s.close().await?;
        Ok(())
    }
    pub async fn rename(&self, from: &str, to: &str) -> Result<()> {
        let s = self.sftp().await?;
        s.rename(from, to).await?;
        s.close().await?;
        Ok(())
    }
    pub async fn delete(&self, path: &str) -> Result<()> {
        let s = self.sftp().await?;
        let m = s.symlink_metadata(path).await?;
        if m.is_dir() {
            s.remove_dir(path).await?;
        } else {
            s.remove_file(path).await?;
        }
        s.close().await?;
        Ok(())
    }
    pub async fn chmod(&self, path: &str, mode: u32) -> Result<()> {
        ensure!(mode <= 0o7777, "权限值无效");
        let s = self.sftp().await?;
        let mut m = s.metadata(path).await?;
        m.permissions = Some(mode);
        s.set_metadata(path, m).await?;
        s.close().await?;
        Ok(())
    }
    pub async fn read_text(&self, path: &str) -> Result<RemoteText> {
        let s = self.sftp().await?;
        let m = s.metadata(path).await?;
        ensure!(
            m.size.unwrap_or(0) <= 4 * 1024 * 1024,
            "内置编辑器最多打开 4 MiB 文本"
        );
        let mut file = s.open(path).await?;
        let mut bytes = vec![];
        (&mut file)
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .await?;
        ensure!(bytes.len() <= 4 * 1024 * 1024, "文件超过编辑大小限制");
        let content = String::from_utf8(bytes)
            .map_err(|_| anyhow!("文件不是 UTF-8 文本，请下载后使用外部编辑器"))?;
        file.close().await?;
        s.close().await?;
        Ok(RemoteText {
            path: path.into(),
            content,
            revision: FileRevision {
                size: m.size.unwrap_or(0),
                modified: m.mtime.unwrap_or(0),
            },
        })
    }
    pub async fn save_text(&self, text: &RemoteText) -> Result<FileRevision> {
        ensure!(text.content.len() <= 4 * 1024 * 1024, "文本超过 4 MiB 限制");
        let s = self.sftp().await?;
        let old = s.metadata(&text.path).await?;
        ensure!(
            FileRevision {
                size: old.size.unwrap_or(0),
                modified: old.mtime.unwrap_or(0)
            } == text.revision,
            "远端文件已改变，请重新打开或另存，避免覆盖他人修改"
        );
        // Write the same file after checking its revision; this preserves its mode and symlink target.
        // SFTP v3 lacks a portable atomic-overwrite operation: document the remaining race.
        let mut file = s
            .open_with_flags(&text.path, OpenFlags::WRITE | OpenFlags::TRUNCATE)
            .await?;
        file.write_all(text.content.as_bytes()).await?;
        file.flush().await?;
        file.close().await?;
        let m = s.metadata(&text.path).await?;
        s.close().await?;
        Ok(FileRevision {
            size: m.size.unwrap_or(0),
            modified: m.mtime.unwrap_or(0),
        })
    }
    pub async fn transfer(
        &self,
        id: String,
        direction: &str,
        local: &Path,
        remote: &str,
        cancel: CancellationToken,
        progress: mpsc::Sender<TransferProgress>,
    ) -> Result<()> {
        let s = self.sftp().await?;
        let upload = direction == "upload";
        let name = local
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let total = if upload {
            tokio::fs::metadata(local).await?.len()
        } else {
            s.metadata(remote).await?.size.unwrap_or(0)
        };
        let mut done = 0u64;
        let mut buffer = vec![0u8; 64 * 1024];
        let mut local_file = if upload {
            tokio::fs::File::open(local).await?
        } else {
            tokio::fs::File::create(local).await?
        };
        let mut remote_file = if upload {
            s.open_with_flags(
                remote,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::EXCLUDE,
            )
            .await?
        } else {
            s.open(remote).await?
        };
        let mut last = tokio::time::Instant::now();
        let outcome:Result<()>=async{loop{let count=tokio::select!{_ = cancel.cancelled()=>return Err(anyhow!("传输已取消")),_ = self.cancel.cancelled()=>return Err(anyhow!("连接已关闭")),r=async {if upload{local_file.read(&mut buffer).await}else{remote_file.read(&mut buffer).await}}=>r?};if count==0{break;}tokio::select!{_ = cancel.cancelled()=>return Err(anyhow!("传输已取消")),r=async {if upload{remote_file.write_all(&buffer[..count]).await}else{local_file.write_all(&buffer[..count]).await}}=>r?};done+=count as u64;if last.elapsed()>=Duration::from_millis(100){let _=progress.try_send(TransferProgress{id:id.clone(),name:name.clone(),direction:direction.into(),transferred:done,total,status:"running".into(),error:None});last=tokio::time::Instant::now();}}if upload{remote_file.flush().await?;}else{local_file.flush().await?;}Ok(())}.await;
        let _ = remote_file.close().await;
        let _ = s.close().await;
        let error = outcome.as_ref().err().map(|e| e.to_string());
        let _ = progress
            .send(TransferProgress {
                id,
                name,
                direction: direction.into(),
                transferred: done,
                total,
                status: if outcome.is_ok() {
                    "completed"
                } else if cancel.is_cancelled() {
                    "cancelled"
                } else {
                    "failed"
                }
                .into(),
                error,
            })
            .await;
        outcome
    }
}

async fn terminal_loop(
    mut channel: russh::Channel<client::Msg>,
    mut commands: mpsc::Receiver<TerminalCommand>,
    events: mpsc::Sender<SessionEvent>,
    cancel: CancellationToken,
) -> Result<()> {
    loop {
        tokio::select! {_ = cancel.cancelled()=>{let _=channel.close().await;break;}, command=commands.recv()=>match command{Some(TerminalCommand::Input(data))=>channel.data(&data[..]).await?,Some(TerminalCommand::Resize{columns,rows})=>channel.window_change(columns,rows,0,0).await?,None=>break},message=channel.wait()=>match message{Some(ChannelMsg::Data{data})|Some(ChannelMsg::ExtendedData{data,..})=>{tokio::select!{_ = cancel.cancelled()=>break,r=events.send(SessionEvent::Output(data.to_vec()))=>{if r.is_err(){break;}}}},Some(ChannelMsg::Failure)=>return Err(anyhow!("服务器拒绝终端请求")),None|Some(ChannelMsg::Close)=>break,_=>{}}}
    }
    Ok(())
}
