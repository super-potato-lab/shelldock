use crate::{legacy_secret, model::Connection};
use anyhow::{ensure, Result};
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use uuid::Uuid;
use zeroize::Zeroizing;

pub struct ImportItem {
    pub connection: Connection,
    pub password: Option<Zeroizing<String>>,
}
pub struct ImportReport {
    pub items: Vec<ImportItem>,
    pub warnings: Vec<String>,
}
const MAX_FILE: u64 = 8 * 1024 * 1024;

fn text(v: &Value, fields: &[&str]) -> String {
    fields
        .iter()
        .find_map(|f| v.get(*f).and_then(Value::as_str))
        .unwrap_or_default()
        .to_string()
}
fn number(v: &Value, f: &str) -> Option<u64> {
    v.get(f)
        .and_then(|x| x.as_u64().or_else(|| x.as_str()?.parse().ok()))
}

pub fn import_json(data: &str, default_group: &str) -> Result<ImportReport> {
    ensure!(data.len() <= MAX_FILE as usize, "连接文件超过 8 MiB 限制");
    let value: Value = serde_json::from_str(data.trim_start_matches('\u{feff}'))?;
    let native = value.get("format").and_then(Value::as_str) == Some("shelldock");
    let values = if let Some(a) = value.as_array() {
        a.clone()
    } else if let Some(a) = value.get("connections").and_then(Value::as_array) {
        a.clone()
    } else {
        vec![value]
    };
    ensure!(values.len() <= 5000, "一次最多导入 5000 个连接");
    let mut report = ImportReport {
        items: vec![],
        warnings: vec![],
    };
    for (idx, value) in values.into_iter().enumerate() {
        if !value.is_object() || value.get("host").is_none() {
            report
                .warnings
                .push(format!("第 {} 项不是连接配置，已跳过", idx + 1));
            continue;
        }
        let port = number(&value, "port").unwrap_or(22);
        if port == 0 || port > 65535 {
            report
                .warnings
                .push(format!("第 {} 项端口无效，已跳过", idx + 1));
            continue;
        }
        let native_auth = text(&value, &["auth"]);
        let key_id = text(&value, &["secret_key_id"]);
        let key_path = text(&value, &["privateKeyPath", "private_key_path", "key_path"]);
        let is_key = native_auth == "key"
            || number(&value, "authentication_type") == Some(2)
            || !key_id.is_empty()
            || !key_path.is_empty();
        let mut extra = if native {
            value.get("extra").cloned().unwrap_or(json!({}))
        } else {
            value.clone()
        };
        scrub_secrets(&mut extra);
        let protocol = if native {
            text(&value, &["protocol"])
        } else if number(&value, "conection_type").unwrap_or(100) != 100 {
            "unsupported".into()
        } else {
            "ssh".into()
        };
        let mut connection = Connection {
            id: Uuid::new_v4().to_string(),
            name: text(&value, &["name", "title"]),
            host: text(&value, &["host", "hostname"]),
            port: port as u16,
            username: text(&value, &["user_name", "username", "user"]),
            group: text(&value, &["group", "group_path"]),
            auth: if is_key { "key" } else { "password" }.into(),
            private_key_path: (!key_path.is_empty()).then_some(key_path),
            notes: text(&value, &["notes", "description", "remark"]),
            has_password: false,
            protocol: if protocol.is_empty() {
                "ssh".into()
            } else {
                protocol
            },
            extra,
        };
        if connection.name.is_empty() {
            connection.name = connection.host.clone();
        }
        if connection.username.is_empty() {
            connection.username = "root".into();
        }
        if connection.group.is_empty() {
            connection.group = default_group.into();
        }
        if let Err(e) = connection.validate() {
            report.warnings.push(format!("第 {} 项：{}", idx + 1, e));
            continue;
        }
        let password = if native {
            None
        } else if let Some(s) = value
            .get("password")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            match legacy_secret::decode(s) {
                Ok(p) => Some(p),
                Err(_) => {
                    report
                        .warnings
                        .push(format!("{}：密码格式未识别，需要重新输入", connection.name));
                    None
                }
            }
        } else {
            None
        };
        connection.has_password = password.is_some();
        if is_key && connection.private_key_path.is_none() {
            report.warnings.push(format!(
                "{}：私钥引用 {} 需要重新选择本机私钥文件",
                connection.name, key_id
            ));
        }
        let proxy = text(&value, &["proxy_id"]);
        if !proxy.is_empty() && proxy != "0" {
            report.warnings.push(format!(
                "{}：代理引用已保留，配置代理前禁止直接连接",
                connection.name
            ));
        }
        if value
            .get("port_forwarding_list")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty())
        {
            report
                .warnings
                .push(format!("{}：转发规则已保留，目前尚未执行", connection.name));
        }
        if connection.protocol != "ssh" {
            report.warnings.push(format!(
                "{}：连接类型已保留，当前只支持 SSH",
                connection.name
            ));
        }
        report.items.push(ImportItem {
            connection,
            password,
        });
    }
    Ok(report)
}

pub fn scrub_secrets(v: &mut Value) {
    match v {
        Value::Object(map) => {
            map.retain(|k, _| {
                !matches!(
                    k.to_ascii_lowercase().as_str(),
                    "password" | "passphrase" | "key_data" | "privatekey" | "private_key"
                )
            });
            for value in map.values_mut() {
                scrub_secrets(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                scrub_secrets(value);
            }
        }
        _ => {}
    }
}

fn read_json(path: &Path) -> Result<String> {
    ensure!(
        std::fs::metadata(path)?.len() <= MAX_FILE,
        "文件超过 8 MiB 限制"
    );
    Ok(std::fs::read_to_string(path)?)
}
fn walk(path: &Path, depth: usize, files: &mut Vec<PathBuf>) -> Result<()> {
    ensure!(depth <= 32 && files.len() <= 5000, "配置目录超过导入限制");
    for e in std::fs::read_dir(path)? {
        let e = e?;
        let kind = e.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            walk(&e.path(), depth + 1, files)?;
        } else if e.path().extension().is_some_and(|x| x == "json") {
            files.push(e.path());
        }
    }
    Ok(())
}

pub fn import_path(path: &Path) -> Result<ImportReport> {
    if path.is_file() {
        return import_json(&read_json(path)?, "");
    }
    ensure!(path.is_dir(), "导入路径不存在");
    let root = if path.join("conn").is_dir() {
        path.join("conn")
    } else {
        path.to_path_buf()
    };
    let mut files = vec![];
    walk(&root, 0, &mut files)?;
    files.sort();
    let mut folders: HashMap<String, (String, String)> = HashMap::new();
    let mut warnings = vec![];
    for p in &files {
        if p.file_name().is_some_and(|n| n == "folder.json") {
            if let Ok(v) = read_json(p).and_then(|s| Ok(serde_json::from_str::<Value>(&s)?)) {
                folders.insert(
                    text(&v, &["id"]),
                    (text(&v, &["name"]), text(&v, &["parent_id"])),
                );
            } else {
                warnings.push(format!(
                    "分组文件 {} 无法读取",
                    p.file_name().unwrap_or_default().to_string_lossy()
                ));
            }
        }
    }
    let mut report = ImportReport {
        items: vec![],
        warnings,
    };
    for p in files {
        if !p
            .file_name()
            .is_some_and(|n| n.to_string_lossy().ends_with("_connect_config.json"))
        {
            continue;
        }
        let result = (|| -> Result<ImportReport> {
            let data = read_json(&p)?;
            let v: Value = serde_json::from_str(&data)?;
            let mut id = text(&v, &["parent_id"]);
            let mut names = vec![];
            let mut seen = HashSet::new();
            while !id.is_empty() && id != "root" {
                if !seen.insert(id.clone()) {
                    report.warnings.push("分组循环引用已忽略".into());
                    break;
                }
                if let Some((name, parent)) = folders.get(&id) {
                    names.push(name.clone());
                    id = parent.clone();
                } else {
                    report
                        .warnings
                        .push(format!("分组 {} 缺少 folder.json", id));
                    break;
                }
            }
            names.reverse();
            import_json(&data, &names.join("/"))
        })();
        match result {
            Ok(mut r) => {
                report.items.append(&mut r.items);
                report.warnings.append(&mut r.warnings);
            }
            Err(_) => report.warnings.push(format!(
                "{} 不是有效连接文件",
                p.file_name().unwrap_or_default().to_string_lossy()
            )),
        }
    }
    ensure!(!report.items.is_empty(), "没有发现可导入的连接文件");
    Ok(report)
}

pub fn export_finalshell(connection: &Connection, password: Option<&str>) -> Result<Value> {
    let mut v = if connection.extra.is_object() {
        connection.extra.clone()
    } else {
        json!({})
    };
    scrub_secrets(&mut v);
    v["id"] = json!(connection.id);
    v["name"] = json!(connection.name);
    v["host"] = json!(connection.host);
    v["port"] = json!(connection.port);
    v["user_name"] = json!(connection.username);
    v["authentication_type"] = json!(if connection.auth == "key" { 2 } else { 1 });
    if v.get("conection_type").is_none() {
        v["conection_type"] = json!(100);
    }
    // Standalone exports cannot safely refer to another app's folder identifiers.
    v["parent_id"] = json!("root");
    v["group_path"] = json!(connection.group);
    v["password"] = json!(match password {
        Some(p) => legacy_secret::encode(p)?,
        None => String::new(),
    });
    Ok(v)
}

pub fn export_native(connections: &[Connection]) -> Value {
    let mut safe = connections.to_vec();
    for c in &mut safe {
        c.has_password = false;
        scrub_secrets(&mut c.extra);
    }
    json!({"format":"shelldock","version":1,"connections":safe})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_real_field_names_without_leaking_secret() {
        let r=import_json(r#"{"name":"测试","host":"example.test","port":"2200","user_name":"ops","conection_type":100,"authentication_type":1,"password":"AQIDBAUGBwjPeQvDruVvdza4kcHpV3CH","custom":{"enabled":true}}"#,"生产").unwrap();
        assert_eq!(r.items[0].connection.port, 2200);
        assert_eq!(r.items[0].connection.username, "ops");
        assert_eq!(
            r.items[0].password.as_deref().map(|p| p.as_str()),
            Some("vector-password")
        );
        let public = serde_json::to_string(&r.items[0].connection).unwrap();
        assert!(!public.contains("PeQv"));
        assert!(!public.contains("vector-password"));
        assert!(r.items[0].connection.extra["custom"]["enabled"]
            .as_bool()
            .unwrap());
    }
    #[test]
    fn compatibility_export_round_trip() {
        let r = import_json(r#"{"host":"example.test","user_name":"root"}"#, "").unwrap();
        let v = export_finalshell(&r.items[0].connection, Some("测试-password")).unwrap();
        let imported = import_json(&v.to_string(), "").unwrap();
        assert_eq!(
            &**imported.items[0].password.as_ref().unwrap(),
            "测试-password"
        );
        let native = export_native(&[r.items[0].connection.clone()]);
        assert!(import_json(&native.to_string(), "").unwrap().items[0]
            .password
            .is_none());
    }
    #[test]
    fn invalid_port_is_not_wrapped() {
        assert!(import_json(r#"{"host":"x","port":65536}"#, "")
            .unwrap()
            .items
            .is_empty());
    }
    #[test]
    fn directory_import_restores_group_tree() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("conn")).unwrap();
        std::fs::write(
            d.path().join("conn/folder.json"),
            r#"{"id":"team","name":"生产","parent_id":"root"}"#,
        )
        .unwrap();
        std::fs::write(
            d.path().join("conn/test_connect_config.json"),
            r#"{"host":"example.test","user_name":"ops","parent_id":"team"}"#,
        )
        .unwrap();
        assert_eq!(
            import_path(d.path()).unwrap().items[0].connection.group,
            "生产"
        );
    }
}
