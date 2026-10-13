use serde::{Deserialize, Serialize};
use shelldock_core::model::Connection;
use std::{
    collections::HashMap,
    io::Write,
    path::{Path, PathBuf},
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Persistent {
    pub version: u32,
    pub connections: Vec<Connection>,
    #[serde(default)]
    pub known_hosts: HashMap<String, String>,
    #[serde(default)]
    pub history: Vec<String>,
    #[serde(default)]
    pub quick_commands: Vec<String>,
}
impl Default for Persistent {
    fn default() -> Self {
        Self {
            version: 1,
            connections: vec![],
            known_hosts: HashMap::new(),
            history: vec![],
            quick_commands: vec![
                "ls -lah".into(),
                "df -h".into(),
                "free -m".into(),
                "top".into(),
                "pwd".into(),
            ],
        }
    }
}
pub struct Store {
    pub data: Persistent,
    path: PathBuf,
}
impl Store {
    pub fn open(path: PathBuf) -> anyhow::Result<Self> {
        let data = if path.exists() {
            serde_json::from_slice(&std::fs::read(&path)?)?
        } else {
            Persistent::default()
        };
        Ok(Self { data, path })
    }
    pub fn save(&self) -> anyhow::Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(&self.data)?)
    }
}
pub fn write_atomic(path: &Path, data: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("无效保存路径"))?;
    std::fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".shelldock-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut f = options.open(&temp)?;
        f.write_all(data)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&temp, path)?;
        Ok::<_, std::io::Error>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result?;
    Ok(())
}
