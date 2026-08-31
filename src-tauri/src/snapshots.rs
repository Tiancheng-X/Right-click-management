//! 快照与时间点还原（T11 / 备份设计 B1–B4）：
//! - 每操作一录全带前像：值级（禁用/启用）与子树级（删除/手动点）统一为 SnapshotEntry
//! - JSON 主存储（每条一个文件），删除类附 .reg（T10 已导出），还原前自动留档可撤销
//! - 还原 = 逐键精确写回 + 外部改动如实报告
//!
//! 值级读写走 winapi 原生（vtype 保持 u32），绕开 winreg 的 RegType 包装类型。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use winapi::shared::minwindef::DWORD;
use winapi::um::winreg::{RegQueryValueExW, RegSetValueExW};
use winreg::enums::{KEY_ALL_ACCESS, KEY_READ, KEY_SET_VALUE};
use winreg::RegKey;

/// 注册表值的可序列化载体
#[derive(Serialize, Deserialize, Clone)]
pub struct RawValue {
    pub vtype: u32,
    pub bytes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct KeyImage {
    pub path: String,
    pub values: Vec<(String, RawValue)>,
    pub children: Vec<KeyImage>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "type")]
pub enum SnapshotEntry {
    /// 值级前像：data = None 表示改动前该值不存在
    Value { path: String, name: String, data: Option<RawValue> },
    /// 子树级前像：image = None 表示改动前该键不存在
    Key { path: String, image: Option<KeyImage> },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Snapshot {
    pub id: String,
    pub when: u64, // unix 毫秒
    pub kind: String, // disable | enable | delete | manual | pre-restore
    pub action: String,
    pub entries: Vec<SnapshotEntry>,
    pub reg_file: Option<String>,
    pub protected: bool, // 删除类/手动点/还原前留档：永不自动清理
}

#[derive(Serialize, Clone)]
pub struct SnapshotMeta {
    pub id: String,
    pub when: u64,
    pub kind: String,
    pub action: String,
    pub keys: usize,
    pub protected: bool,
    pub reg_file: Option<String>,
}

#[derive(Serialize)]
pub struct RestoreReport {
    pub written: usize,
    pub notes: Vec<String>,
    pub undo_id: Option<String>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn dir() -> Result<PathBuf, String> {
    crate::registry::snapshots_dir()
}

fn file_of(id: &str) -> Result<PathBuf, String> {
    Ok(dir()?.join(format!("{id}.json")))
}

impl Snapshot {
    pub fn new(kind: &str, action: String, entries: Vec<SnapshotEntry>, protected: bool) -> Self {
        let when = now_ms();
        Snapshot {
            id: format!("{when}-{kind}"),
            when,
            kind: kind.to_string(),
            action,
            entries,
            reg_file: None,
            protected,
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let dir = dir()?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let json = serde_json::to_string(self).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(format!("{}.json", self.id)), json).map_err(|e| e.to_string())?;
        trim_old()
    }
}

/// 保存后滚动清理：非保护快照仅保留最近 N 条（T6 B2，N 由设置页可调，T15）
fn trim_old() -> Result<(), String> {
    let keep = auto_keep() as usize;
    let dir = dir()?;
    let mut autos: Vec<(u64, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(s) = read_snapshot(&path) {
            if !s.protected {
                autos.push((s.when, path));
            }
        }
    }
    autos.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, path) in autos.iter().skip(keep) {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/* ===== 设置（T15） ===== */

#[derive(Serialize, Deserialize)]
pub struct AppSettings {
    pub auto_keep: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self { auto_keep: 60 }
    }
}

fn settings_file() -> Result<PathBuf, String> {
    Ok(dir()?.join("settings.json"))
}

pub fn load_settings() -> AppSettings {
    settings_file()
        .and_then(|p| std::fs::read_to_string(p).map_err(|e| e.to_string()))
        .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        .unwrap_or_default()
}

pub fn save_settings(s: &AppSettings) -> Result<(), String> {
    let dir = dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string(s).map_err(|e| e.to_string())?;
    std::fs::write(settings_file()?, json).map_err(|e| e.to_string())
}

pub fn auto_keep() -> u32 {
    load_settings().auto_keep
}

pub fn set_auto_keep(n: u32) -> Result<(), String> {
    let n = n.clamp(5, 500);
    save_settings(&AppSettings { auto_keep: n })?;
    trim_old()
}

/// 清空快照：include_protected = false 仅自动记录；true 连手动点与删除备份一起清
pub fn clear(include_protected: bool) -> Result<usize, String> {
    let dir = dir()?;
    let mut removed = 0usize;
    if !dir.exists() {
        return Ok(0);
    }
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let protected = read_snapshot(&path).map(|s| s.protected).unwrap_or(false);
        if include_protected || !protected {
            if std::fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

fn read_snapshot(path: &Path) -> Result<Snapshot, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

/// 全部快照元信息（按时间倒序）
pub fn list() -> Result<Vec<SnapshotMeta>, String> {
    let dir = dir()?;
    let mut out = Vec::new();
    if !dir.exists() {
        return Ok(out);
    }
    for entry in std::fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(s) = read_snapshot(&path) {
            let keys = s.entries.len();
            out.push(SnapshotMeta {
                id: s.id,
                when: s.when,
                kind: s.kind,
                action: s.action,
                keys,
                protected: s.protected,
                reg_file: s.reg_file,
            });
        }
    }
    out.sort_by(|a, b| b.when.cmp(&a.when));
    Ok(out)
}

fn load(id: &str) -> Result<Snapshot, String> {
    read_snapshot(&file_of(id)?)
}

/* ===== 前像捕获 ===== */

/// 拆出 (hive, 其余子键路径)；快照路径永远显式写 HKCU/HKLM，不存 HKCR
fn split_hive(path: &str) -> Result<(winreg::HKEY, String), String> {
    use winreg::enums::*;
    if let Some(rest) = path.strip_prefix("HKCU\\") {
        return Ok((HKEY_CURRENT_USER, rest.to_string()));
    }
    if let Some(rest) = path.strip_prefix("HKLM\\") {
        return Ok((HKEY_LOCAL_MACHINE, rest.to_string()));
    }
    Err(format!("快照路径缺少显式 hive: {path}"))
}

fn open_full(path: &str, access: u32) -> Option<RegKey> {
    let (hive, rest) = split_hive(path).ok()?;
    RegKey::predef(hive).open_subkey_with_flags(rest, access).ok()
}

/// winapi 原生读值（返回 vtype + 原始字节），绕开 RegType 包装
fn raw_read(key: &RegKey, name: &str) -> Option<(u32, Vec<u8>)> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let mut ty: DWORD = 0;
        let mut size: DWORD = 0;
        let rc = RegQueryValueExW(
            key.raw_handle() as *mut _,
            wide.as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            std::ptr::null_mut(),
            &mut size,
        );
        if rc != 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let rc = RegQueryValueExW(
            key.raw_handle() as *mut _,
            wide.as_ptr(),
            std::ptr::null_mut(),
            &mut ty,
            buf.as_mut_ptr(),
            &mut size,
        );
        if rc != 0 {
            return None;
        }
        buf.truncate(size as usize);
        Some((ty, buf))
    }
}

/// winapi 原生写值
fn raw_write(key: &RegKey, name: &str, ty: u32, bytes: &[u8]) -> Result<(), String> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let rc = unsafe {
        RegSetValueExW(
            key.raw_handle() as *mut _,
            wide.as_ptr(),
            0,
            ty,
            bytes.as_ptr(),
            bytes.len() as DWORD,
        )
    };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("写入失败 (RegSetValueExW rc={rc})"))
    }
}

/// 值级前像
pub fn capture_value(path: &str, name: &str) -> SnapshotEntry {
    let data = open_full(path, KEY_READ)
        .and_then(|k| raw_read(&k, name))
        .map(|(vtype, bytes)| RawValue { vtype, bytes });
    SnapshotEntry::Value {
        path: path.to_string(),
        name: name.to_string(),
        data,
    }
}

/// 子树级前像（递归）
pub fn capture_key_tree(path: &str) -> SnapshotEntry {
    SnapshotEntry::Key {
        path: path.to_string(),
        image: capture_key_image(path),
    }
}

/// 仅取子树镜像本体（None = 键不存在）
pub fn capture_key_image(path: &str) -> Option<KeyImage> {
    open_full(path, KEY_READ).map(|k| read_tree(path, &k))
}

fn read_tree(path: &str, k: &RegKey) -> KeyImage {
    let names: Vec<String> = k.enum_values().flatten().map(|(n, _)| n).collect();
    let values = names
        .iter()
        .filter_map(|n| raw_read(k, n).map(|(vtype, bytes)| (n.clone(), RawValue { vtype, bytes })))
        .collect();
    let children = k
        .enum_keys()
        .flatten()
        .filter_map(|c| {
            let child_path = format!(r"{path}\{c}");
            k.open_subkey(&c).ok().map(|ck| read_tree(&child_path, &ck))
        })
        .collect();
    KeyImage {
        path: path.to_string(),
        values,
        children,
    }
}

/* ===== 还原 ===== */

fn capture_current(e: &SnapshotEntry) -> SnapshotEntry {
    match e {
        SnapshotEntry::Value { path, name, .. } => capture_value(path, name),
        SnapshotEntry::Key { path, .. } => capture_key_tree(path),
    }
}

fn rebuild(img: &KeyImage) -> Result<usize, String> {
    let (hive, rest) = split_hive(&img.path)?;
    let root = RegKey::predef(hive);
    let (k, _) = root.create_subkey(&rest).map_err(|e| format!("重建键 {} 失败: {e}", img.path))?;
    let mut n = 1 + img.values.len();
    for (name, raw) in &img.values {
        raw_write(&k, name, raw.vtype, &raw.bytes)?;
    }
    for c in &img.children {
        n += rebuild(c)?;
    }
    Ok(n)
}

fn apply(entries: &[SnapshotEntry]) -> Result<(usize, Vec<String>), String> {
    let mut written = 0usize;
    let mut notes: Vec<String> = Vec::new();
    // 逆序应用：先恢复叶子再恢复父级语义更贴近原状
    for e in entries.iter().rev() {
        match e {
            SnapshotEntry::Value { path, name, data } => {
                let key = open_full(path, KEY_SET_VALUE)
                    .ok_or_else(|| format!("键不可写: {path}"))?;
                match data {
                    Some(v) => {
                        raw_write(&key, name, v.vtype, &v.bytes)?;
                        written += 1;
                    }
                    None => match key.delete_value(name) {
                        Ok(_) => written += 1,
                        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                        Err(err) => return Err(format!("移除 {name} 失败: {err}")),
                    },
                }
            }
            SnapshotEntry::Key { path, image } => match image {
                Some(img) => {
                    if open_full(path, KEY_READ).is_some() {
                        notes.push(format!("键在快照后已被重建，已按前像合并写回：{path}"));
                    }
                    written += rebuild(img)?;
                }
                None => {
                    // 当时不存在 → 删除现在存在的
                    let (hive, rest) = split_hive(path)?;
                    if let Some((parent_rel, leaf)) = rest.rsplit_once('\\') {
                        let root = RegKey::predef(hive);
                        if let Ok(parent) = root.open_subkey_with_flags(parent_rel, KEY_ALL_ACCESS) {
                            if parent.delete_subkey(leaf).is_ok() {
                                written += 1;
                            }
                        }
                    }
                }
            },
        }
    }
    Ok((written, notes))
}

/// 还原：先拍「还原前留档」（可撤销），再逐键写回，报告外部改动
pub fn restore(id: &str) -> Result<RestoreReport, String> {
    let snap = load(id)?;
    let undo_entries: Vec<SnapshotEntry> = snap.entries.iter().map(capture_current).collect();
    let undo_id = format!("{}-pre-restore", now_ms());
    let undo = Snapshot {
        id: undo_id.clone(),
        when: now_ms(),
        kind: "pre-restore".into(),
        action: format!("还原前留档（对应 {}）", snap.action),
        entries: undo_entries,
        reg_file: None,
        protected: true,
    };
    undo.save()?;
    let (written, notes) = apply(&snap.entries)?;
    Ok(RestoreReport {
        written,
        notes,
        undo_id: Some(undo_id),
    })
}

/// 手动全量快照点：扫全部挂载点的 shell 与 shellex 子树（HKCU/HKLM 两侧各取现存者）
pub fn create_manual() -> Result<(), String> {
    let mut entries = Vec::new();
    for mount in crate::registry::MOUNTS {
        for hive_prefix in ["HKCU", "HKLM"] {
            for sub in [
                format!(r"Software\Classes\{mount}\shell"),
                format!(r"Software\Classes\{mount}\shellex\ContextMenuHandlers"),
            ] {
                let full = format!(r"{hive_prefix}\{sub}");
                if open_full(&full, KEY_READ).is_some() {
                    entries.push(capture_key_tree(&full));
                }
            }
        }
    }
    let snap = Snapshot::new("manual", "手动快照（全量受管键）".into(), entries, true);
    snap.save()
}
