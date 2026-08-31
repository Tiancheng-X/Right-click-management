//! 注册表访问占位模块：只读枚举 + 来源推导。
//!
//! 域模型见 wayfinder「核心域模型与操作语义」；挂载点→场景映射见 CONTEXT.md。
//! 禁用（LegacyDisable / Blocked 键）、删除（断开挂接）、快照等写操作在后续工单接入。

use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use winreg::enums::{
    HKEY_CLASSES_ROOT, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_ALL_ACCESS, KEY_READ, KEY_SET_VALUE,
};
use winreg::RegKey;

#[derive(Serialize, Clone)]
pub struct MenuEntry {
    /// 显示名称：优先 MUIVerb，其次键默认值，最后用键名
    pub name: String,
    /// "verb"（静态动词）| "shellex"（COM 处理器）
    pub kind: String,
    /// 挂载点，如 "*"、"Directory"
    pub mount: String,
    /// 注册表位置（展示用）
    pub reg_path: String,
    /// 静态动词 = 命令行；shellex = CLSID 解析出的 DLL 路径；空 = 由系统实现
    pub command: String,
    /// 来源应用（推导）：动词 = 命令行 exe 名；shellex = DLL 所在目录名，回退挂接键名
    pub source: String,
    /// 真实图标（PNG data URL）；提取失败为 None（前端回退占位图形）
    pub icon: Option<String>,
    /// shellex 的 CLSID（动词为空）
    pub clsid: String,
    /// 当前是否启用（推导）：动词 = 无 LegacyDisable；shellex = CLSID 不在 Blocked 键
    pub enabled: bool,
}

/// HKCU 的 Shell Extensions Blocked 键（免管理员、写值不删键的禁用手法）
const BLOCKED_SUBPATH: &str =
    r"Software\Microsoft\Windows\CurrentVersion\Shell Extensions\Blocked";

/// 自建条目标记值：写在该条目键上，用于区分「我创建的」（可编辑）与他人条目（D6）
pub const MARKER_NAME: &str = "MenuManager";
const MARKER_DATA: &str = "RightClickManager";

/// 「一键切回经典菜单」的 CLSID 覆盖键（T1 调研：HKCU 空字符串默认值 + 重启 explorer 生效；
/// 22H2–25H2 稳定版可用、属非官方手段，系统更新可能清键失效——状态以键为准，T9 Q2）
const CLASSIC_SUBPATH: &str =
    r"Software\Classes\CLSID\{86ca1aa0-34aa-4e8b-a509-50c905bae2a2}\InprocServer32";

/// 经典菜单是否已开启：键存在且默认值为空串 = 开；否则 = 关（系统更新清键后自然为关）
pub fn classic_menu_state() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    hkcu.open_subkey_with_flags(CLASSIC_SUBPATH, KEY_READ)
        .and_then(|k| k.get_value::<String, _>(""))
        .map(|v| v.trim().is_empty())
        .unwrap_or(false)
}

/// 开 = InprocServer32 建**空字符串**默认值；关 = 删除该默认值（保守不删键）。
/// 操作前自动留前像快照（值级），可从快照历史还原
pub fn set_classic_menu(on: bool) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let full = format!(r"HKCU\{CLASSIC_SUBPATH}");
    let before = crate::snapshots::capture_value(&full, "");
    if on {
        let (key, _) = hkcu
            .create_subkey(CLASSIC_SUBPATH)
            .map_err(|e| format!("创建覆盖键失败: {e}"))?;
        key.set_value("", &"")
            .map_err(|e| format!("写入空默认值失败: {e}"))?;
    } else {
        let key = hkcu
            .open_subkey_with_flags(CLASSIC_SUBPATH, KEY_SET_VALUE)
            .map_err(|e| format!("打开覆盖键失败: {e}"))?;
        if let Err(e) = key.delete_value("") {
            if e.kind() != std::io::ErrorKind::NotFound {
                return Err(format!("恢复失败: {e}"));
            }
        }
    }
    let action = if on { "开启经典菜单" } else { "恢复 Win11 新版菜单" };
    crate::snapshots::Snapshot::new("classic", action.to_string(), vec![before], false).save()?;
    Ok(())
}

/// 扫描的挂载点全景（T2 调研实证清单的主体）；
/// ProgID / SystemFileAssociations 按扩展名动态展开，属后续增强。
pub const MOUNTS: &[&str] = &[
    "*",
    "AllFilesystemObjects",
    "Folder",
    "Directory",
    "Directory\\Background",
    "DesktopBackground",
    "Drive",
    "lnkfile",
];

pub fn list_menu_entries() -> Result<Vec<MenuEntry>, String> {
    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let blocked = blocked_clsids();
    let mut out = Vec::new();
    for mount in MOUNTS {
        let key = hkcr
            .open_subkey_with_flags(mount, KEY_READ)
            .map_err(|e| format!("打开 HKCR\\{mount} 失败: {e}"))?;
        enum_verbs(&key, mount, &mut out);
        enum_shellex(&hkcr, &key, mount, &blocked, &mut out);
    }
    out.sort_by(|a, b| {
        a.mount
            .cmp(&b.mount)
            .then(a.kind.cmp(&b.kind))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    // 新版菜单：MSIX 打包条目（T14；枚举失败静默跳过，不影响经典枚举）
    if let Ok(list) = crate::msix::enumerate() {
        for e in list {
            let enabled = e.clsid.trim().is_empty() || !blocked.contains(&e.clsid);
            out.push(MenuEntry {
                name: format!("{} · {}", e.package_display, e.verb_id),
                kind: "packaged".into(),
                mount: e.item_types.join(" · "),
                reg_path: format!("MSIX 包 · {}", e.package_display),
                command: String::new(),
                source: e.package_display,
                icon: e.icon,
                clsid: e.clsid,
                enabled,
            });
        }
    }
    Ok(out)
}

/// 枚举 `shell\<verb>` 静态动词
fn enum_verbs(mount_key: &RegKey, mount: &str, out: &mut Vec<MenuEntry>) {
    let Ok(shell) = mount_key.open_subkey("shell") else {
        return;
    };
    for name in shell.enum_keys().flatten() {
        let Ok(verb) = shell.open_subkey(&name) else {
            continue;
        };
        let display = verb
            .get_value::<String, _>("MUIVerb")
            .or_else(|_| verb.get_value::<String, _>(""))
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| name.clone());
        let command = verb
            .open_subkey("command")
            .and_then(|c| c.get_value::<String, _>(""))
            .unwrap_or_default();
        let cmd_path = first_token_path(&command);
        let source = cmd_path
            .as_deref()
            .map(file_stem_of)
            .unwrap_or_else(|| "系统内置".to_string());
        // 图标优先取条目自带的 Icon 值（自建条目走这里），否则回退命令行 exe
        let is_custom = verb.get_value::<String, _>(MARKER_NAME).is_ok();
        let kind = if is_custom { "custom" } else { "verb" };
        let icon_pref = verb
            .get_value::<String, _>("Icon")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let icon = match icon_pref.as_deref() {
            Some(p) => crate::icons::data_url(p, 0),
            None => cmd_path.as_deref().and_then(|p| crate::icons::data_url(p, 0)),
        };
        let enabled = verb.get_value::<String, _>("LegacyDisable").is_err();
        out.push(MenuEntry {
            name: display,
            kind: kind.into(),
            mount: mount.to_string(),
            reg_path: format!("HKCR\\{mount}\\shell\\{name}"),
            command,
            source,
            icon,
            clsid: String::new(),
            enabled,
        });
    }
}

/// 取命令行首个 token（尊重引号）作为可执行文件路径
fn first_token_path(cmd: &str) -> Option<String> {
    let t = cmd.trim();
    if t.is_empty() {
        return None;
    }
    Some(if let Some(rest) = t.strip_prefix('"') {
        rest.split('"').next().unwrap_or(rest).to_string()
    } else {
        t.split_whitespace().next().unwrap_or(t).to_string()
    })
}

fn file_stem_of(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// 枚举 `shellex\ContextMenuHandlers`：解析 CLSID → InprocServer32 DLL，
/// 来源取 DLL 所在目录名（如 `...\7-Zip\7-zip.dll` → 7-Zip），解析失败回退挂接键名
fn enum_shellex(
    hkcr: &RegKey,
    mount_key: &RegKey,
    mount: &str,
    blocked: &HashSet<String>,
    out: &mut Vec<MenuEntry>,
) {
    let Ok(handlers) = mount_key.open_subkey(r"shellex\ContextMenuHandlers") else {
        return;
    };
    for name in handlers.enum_keys().flatten() {
        let Ok(handler) = handlers.open_subkey(&name) else {
            continue;
        };
        let clsid: String = handler.get_value("").unwrap_or_default();
        let mut command = String::new();
        let mut source = name.clone();
        let mut icon: Option<String> = None;
        if !clsid.is_empty() {
            if let Ok(dll_key) =
                hkcr.open_subkey_with_flags(format!(r"CLSID\{clsid}\InprocServer32"), KEY_READ)
            {
                if let Ok(dll) = dll_key.get_value::<String, _>("") {
                    if let Some(dir) =
                        Path::new(dll.trim()).parent().and_then(|p| p.file_name())
                    {
                        source = dir.to_string_lossy().to_string();
                    }
                    icon = crate::icons::data_url(dll.trim(), 0);
                    command = dll;
                }
            }
        }
        let enabled = clsid.trim().is_empty() || !blocked.contains(&clsid);
        let reg_path = format!("HKCR\\{mount}\\shellex\\ContextMenuHandlers\\{name}");
        out.push(MenuEntry {
            name,
            kind: "shellex".into(),
            mount: mount.to_string(),
            reg_path,
            command,
            source,
            icon,
            clsid,
            enabled,
        });
    }
}

/* ============ 写操作（T10：条目操作接入） ============ */

fn strip_hkcr(reg_path: &str) -> Result<String, String> {
    reg_path
        .strip_prefix("HKCR\\")
        .map(|s| s.to_string())
        .ok_or_else(|| format!("无法解析注册表路径: {reg_path}"))
}

/// 打开 HKCR 合并视图下某键的**可写**句柄：HKCU 优先（合并视图 HKCU 优先），否则 HKLM。
/// 同时返回显式 hive 的完整路径（快照前像与 reg.exe 用）
fn classes_key_writable(rest: &str) -> Result<(RegKey, String), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(k) = hkcu.open_subkey_with_flags(format!(r"Software\Classes\{rest}"), KEY_SET_VALUE) {
        return Ok((k, format!(r"HKCU\Software\Classes\{rest}")));
    }
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey_with_flags(format!(r"SOFTWARE\Classes\{rest}"), KEY_SET_VALUE)
        .map(|k| (k, format!(r"HKLM\SOFTWARE\Classes\{rest}")))
        .map_err(|e| format!("打开可写键失败（HKCU 与 HKLM 均不可写）: {e}"))
}

/// 禁用/启用：动词/自定义 = LegacyDisable 值；shellex = Blocked 键写/删 CLSID 空串值。
/// 全部「写值不删键」，可逆（域模型 D2）；操作前自动留前像快照（T6 B1）
pub fn set_entry_enabled(
    reg_path: &str,
    kind: &str,
    clsid: &str,
    enabled: bool,
    name: &str,
) -> Result<(), String> {
    match kind {
        "verb" | "custom" => {
            let rest = strip_hkcr(reg_path)?;
            let (key, full) = classes_key_writable(&rest)?;
            let verb_word = if enabled { "启用" } else { "禁用" };
            let before = crate::snapshots::capture_value(&full, "LegacyDisable");
            if enabled {
                if let Err(e) = key.delete_value("LegacyDisable") {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        return Err(format!("恢复失败: {e}"));
                    }
                }
            } else {
                key.set_value("LegacyDisable", &"")
                    .map_err(|e| format!("禁用失败: {e}"))?;
            }
            crate::snapshots::Snapshot::new(
                if enabled { "enable" } else { "disable" },
                format!("{verb_word}「{name}」"),
                vec![before],
                false,
            )
            .save()?;
            Ok(())
        }
        "shellex" | "packaged" => {
            // 打包条目走同一 Blocked 键机制（T1：实测有效但官方未文档化）
            if clsid.trim().is_empty() {
                return Err("该条目缺少 CLSID，无法通过 Blocked 键禁用".into());
            }
            let blocked_full = format!(r"HKCU\{BLOCKED_SUBPATH}");
            let hkcu = RegKey::predef(HKEY_CURRENT_USER);
            let blocked = hkcu
                .open_subkey_with_flags(BLOCKED_SUBPATH, KEY_SET_VALUE)
                .or_else(|_| hkcu.create_subkey(BLOCKED_SUBPATH).map(|(k, _)| k))
                .map_err(|e| format!("打开 Blocked 键失败: {e}"))?;
            let verb_word = if enabled { "启用" } else { "禁用" };
            let before = crate::snapshots::capture_value(&blocked_full, clsid);
            if enabled {
                if let Err(e) = blocked.delete_value(clsid) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        return Err(format!("恢复失败: {e}"));
                    }
                }
            } else {
                blocked
                    .set_value(clsid, &"")
                    .map_err(|e| format!("禁用失败: {e}"))?;
            }
            crate::snapshots::Snapshot::new(
                if enabled { "enable" } else { "disable" },
                format!("{verb_word}「{name}」"),
                vec![before],
                false,
            )
            .save()?;
            Ok(())
        }
        _ => Err(format!("未知条目类型: {kind}")),
    }
}

/// 删除 = 断开挂接（D3）：删挂载子键，不碰 CLSID 本体；删除前强制导出 .reg 备份 + 子树前像快照，失败即中止
pub fn delete_entry(reg_path: &str, kind: &str, name: &str) -> Result<(), String> {
    if kind == "packaged" {
        return Err("打包条目 v1 不支持删除（不卸载应用包）".into());
    }
    let rest = strip_hkcr(reg_path)?;
    let (use_hkcu, full_path) = locate_classes_key(&rest)?;
    let (parent_rel, leaf) = rest
        .rsplit_once('\\')
        .map(|(p, l)| (p.to_string(), l.to_string()))
        .ok_or_else(|| format!("路径不含叶键: {reg_path}"))?;

    // 前像快照（子树）+ 强制 .reg 导出，二者就绪后才动刀
    let before = crate::snapshots::capture_key_tree(&full_path);
    let reg_file = export_backup(&full_path, &leaf)?;

    let hive = if use_hkcu {
        RegKey::predef(HKEY_CURRENT_USER)
    } else {
        RegKey::predef(HKEY_LOCAL_MACHINE)
    };
    let parent = hive
        .open_subkey_with_flags(&parent_rel, KEY_ALL_ACCESS)
        .map_err(|e| format!("打开父键失败: {e}"))?;
    delete_tree(&parent, &leaf)?;

    let mut snap = crate::snapshots::Snapshot::new(
        "delete",
        format!("删除「{name}」"),
        vec![before],
        true,
    );
    snap.reg_file = Some(reg_file);
    snap.save()?;
    let _ = kind;
    Ok(())
}

/// 判断键位于 HKCU 还是 HKLM，返回 (是否 HKCU, reg.exe 可用的完整路径)
fn locate_classes_key(rest: &str) -> Result<(bool, String), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if hkcu
        .open_subkey_with_flags(format!(r"Software\Classes\{rest}"), KEY_READ)
        .is_ok()
    {
        return Ok((true, format!(r"HKCU\Software\Classes\{rest}")));
    }
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if hklm
        .open_subkey_with_flags(format!(r"SOFTWARE\Classes\{rest}"), KEY_READ)
        .is_ok()
    {
        return Ok((false, format!(r"HKLM\SOFTWARE\Classes\{rest}")));
    }
    Err("未找到目标注册表键（可能已被删除）".into())
}

/// 递归删除子键（先清子键再删自身）
fn delete_tree(parent: &RegKey, name: &str) -> Result<(), String> {
    if let Ok(sub) = parent.open_subkey_with_flags(name, KEY_READ) {
        let children: Vec<String> = sub.enum_keys().flatten().collect();
        for child in children {
            delete_tree(&sub, &child)?;
        }
    }
    parent
        .delete_subkey(name)
        .map_err(|e| format!("删除 {name} 失败: {e}"))
}

/// 删除前备份：reg.exe 导出 .reg（T6 设计：删除类的人工恢复交付物），失败即中止删除
fn export_backup(full_key_path: &str, leaf: &str) -> Result<String, String> {
    let dir = snapshots_dir()?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let safe: String = leaf
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let file = dir.join(format!("delete-{ts}-{safe}.reg"));
    let out = std::process::Command::new("reg")
        .args(["export", full_key_path, &file.to_string_lossy(), "/y"])
        .output()
        .map_err(|e| format!("调用 reg.exe 失败: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "导出删除前备份失败：{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(file.to_string_lossy().to_string())
}

/// 数据目录：便携优先（exe 同目录 data\snapshots），不可写回退 %APPDATA%（T6 B2）
pub fn snapshots_dir() -> Result<PathBuf, String> {
    let (p, _) = data_dir_info();
    Ok(PathBuf::from(p))
}

/// (数据目录路径, 是否便携模式)——设置页展示用
pub fn data_dir_info() -> (String, bool) {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let d = dir.join(r"data\snapshots");
            if std::fs::create_dir_all(&d).is_ok() {
                return (d.to_string_lossy().to_string(), true);
            }
        }
    }
    let appdata = std::env::var("APPDATA").unwrap_or_else(|_| "%APPDATA%".into());
    let d = PathBuf::from(appdata).join(r"RightClickManager\snapshots");
    let _ = std::fs::create_dir_all(&d);
    (d.to_string_lossy().to_string(), false)
}

/* ===== 策略检测（T9 Q4 / T15） ===== */

#[derive(Serialize)]
pub struct PolicyStatus {
    pub menu_disabled: bool,
    pub tray_disabled: bool,
    pub sources: Vec<String>,
}

/// 读组策略键：NoViewContextMenu / NoTrayContextMenu（T2 调研位置），命中 = 右键菜单被策略关闭
pub fn policy_status() -> PolicyStatus {
    let mut st = PolicyStatus {
        menu_disabled: false,
        tray_disabled: false,
        sources: Vec::new(),
    };
    for (hive, label) in [(HKEY_CURRENT_USER, "HKCU"), (HKEY_LOCAL_MACHINE, "HKLM")] {
        let root = RegKey::predef(hive);
        if let Ok(k) = root
            .open_subkey_with_flags(r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer", KEY_READ)
        {
            for (name, target) in [
                ("NoViewContextMenu", &mut st.menu_disabled),
                ("NoTrayContextMenu", &mut st.tray_disabled),
            ] {
                if let Ok(v) = k.get_value::<u32, _>(name) {
                    if v != 0 {
                        *target = true;
                        st.sources.push(format!(r"{label}\...\Explorer\{name}"));
                    }
                }
            }
        }
    }
    st
}

/* ============ 自定义条目（T12） ============ */

fn scene_mount(scene: &str) -> Result<&'static str, String> {
    match scene {
        "file" => Ok("*"),
        "desktop" => Ok(r"Directory\Background"),
        _ => Err(format!("未知场景: {scene}")),
    }
}

/// 生成唯一的 verb 键名（MUIVerb 才是显示名，键名只需唯一）
fn unique_verb_name(shell: &RegKey, name: &str) -> String {
    let mut base: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    if base.is_empty() {
        base = "Custom".into();
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut verb = format!("RCM_{base}{ts}");
    let mut n = 1;
    while shell.open_subkey(&verb).is_ok() {
        verb = format!("{base}_{ts}_{n}");
        n += 1;
    }
    verb
}

fn write_custom_key(shell: &RegKey, verb_name: &str, name: &str, command: &str, icon: &str) -> Result<(), String> {
    let (k, _) = shell
        .create_subkey(verb_name)
        .map_err(|e| format!("创建条目键失败: {e}"))?;
    k.set_value("MUIVerb", &name).map_err(|e| e.to_string())?;
    if !icon.trim().is_empty() {
        k.set_value("Icon", &icon.trim()).map_err(|e| e.to_string())?;
    }
    k.set_value(MARKER_NAME, &MARKER_DATA).map_err(|e| e.to_string())?;
    let (cmd, _) = k.create_subkey("command").map_err(|e| format!("创建 command 失败: {e}"))?;
    cmd.set_value("", &command).map_err(|e| e.to_string())?;
    Ok(())
}

/// 新增自定义条目（写 HKCU，v1 只进经典菜单）；前像 = 「该键当时不存在」
pub fn create_custom_entry(name: &str, command: &str, icon: &str, scene: &str) -> Result<(), String> {
    let mount = scene_mount(scene)?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (shell, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{mount}\shell"))
        .map_err(|e| format!("打开 shell 键失败: {e}"))?;
    let verb_name = unique_verb_name(&shell, name);
    let full = format!(r"HKCU\Software\Classes\{mount}\shell\{verb_name}");
    let before = crate::snapshots::capture_key_tree(&full);
    write_custom_key(&shell, &verb_name, name, command, icon)?;
    crate::snapshots::Snapshot::new(
        "create",
        format!("新增自定义项「{name}」"),
        vec![before],
        false,
    )
    .save()?;
    Ok(())
}

/// 编辑自定义条目：场景可能变化 = 旧位置删、新位置建；前像同时记录两侧，还原完全可逆
pub fn update_custom_entry(
    reg_path: &str,
    name: &str,
    command: &str,
    icon: &str,
    scene: &str,
) -> Result<(), String> {
    let rest = strip_hkcr(reg_path)?;
    let (_, old_full) = locate_classes_key(&rest)?;
    let old_tree = crate::snapshots::capture_key_image(&old_full);

    let mount = scene_mount(scene)?;
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (shell, _) = hkcu
        .create_subkey(format!(r"Software\Classes\{mount}\shell"))
        .map_err(|e| format!("打开 shell 键失败: {e}"))?;
    let verb_name = unique_verb_name(&shell, name);
    let new_full = format!(r"HKCU\Software\Classes\{mount}\shell\{verb_name}");
    let new_before = crate::snapshots::capture_key_image(&new_full); // None = 当时不存在

    write_custom_key(&shell, &verb_name, name, command, icon)?;

    // 删旧键（自建键必在 HKCU，但稳妥按 locate 结果）
    let (old_parent_rel, old_leaf) = rest
        .rsplit_once('\\')
        .map(|(p, l)| (p.to_string(), l.to_string()))
        .ok_or_else(|| format!("路径不含叶键: {reg_path}"))?;
    let hive = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(parent) = hive.open_subkey_with_flags(&old_parent_rel, KEY_ALL_ACCESS) {
        delete_tree(&parent, &old_leaf)?;
    }

    crate::snapshots::Snapshot::new(
        "update",
        format!("更新自定义项「{name}」"),
        vec![
            crate::snapshots::SnapshotEntry::Key { path: new_full, image: new_before },
            crate::snapshots::SnapshotEntry::Key { path: old_full, image: old_tree },
        ],
        false,
    )
    .save()?;
    Ok(())
}

/// HKCU Blocked 键中已被屏蔽的 CLSID 集合
fn blocked_clsids() -> HashSet<String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let mut set = HashSet::new();
    if let Ok(k) = hkcu.open_subkey_with_flags(BLOCKED_SUBPATH, KEY_READ) {
        for (name, _) in k.enum_values().flatten() {
            set.insert(name);
        }
    }
    set
}
