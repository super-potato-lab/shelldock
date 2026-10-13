use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(default)]
    pub group: String,
    #[serde(default = "default_auth")]
    pub auth: String,
    #[serde(default)]
    pub private_key_path: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default)]
    pub extra: Value,
}

fn default_auth() -> String {
    "password".into()
}
fn default_protocol() -> String {
    "ssh".into()
}

impl Connection {
    pub fn validate(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!self.name.trim().is_empty(), "连接名称不能为空");
        anyhow::ensure!(!self.host.trim().is_empty(), "主机地址不能为空");
        anyhow::ensure!(
            !self.host.chars().any(char::is_control),
            "主机地址包含控制字符"
        );
        anyhow::ensure!(self.port != 0, "端口必须在 1–65535 之间");
        anyhow::ensure!(!self.username.trim().is_empty(), "用户名不能为空");
        anyhow::ensure!(
            matches!(self.auth.as_str(), "password" | "key"),
            "未知认证方式"
        );
        Ok(())
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: u32,
    pub permissions: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileRevision {
    pub size: u64,
    pub modified: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteText {
    pub path: String,
    pub content: String,
    pub revision: FileRevision,
}

#[derive(Clone, Serialize)]
#[serde(tag = "event", content = "data", rename_all = "camelCase")]
pub enum SessionEvent {
    Output(Vec<u8>),
    Closed,
    Error(String),
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferProgress {
    pub id: String,
    pub name: String,
    pub direction: String,
    pub transferred: u64,
    pub total: u64,
    pub status: String,
    pub error: Option<String>,
}
