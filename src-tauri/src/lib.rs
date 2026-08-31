mod icons;
mod msix;
mod registry;
mod snapshots;

use serde::Serialize;

/// 只读枚举：经典菜单条目（含状态与来源推导）
#[tauri::command]
fn list_menu_entries() -> Result<Vec<registry::MenuEntry>, String> {
    registry::list_menu_entries()
}

/// 禁用/启用（写值不删键，可逆；操作前自动留前像快照）
#[tauri::command]
fn set_entry_enabled(
    reg_path: String,
    kind: String,
    clsid: String,
    enabled: bool,
    name: String,
) -> Result<(), String> {
    registry::set_entry_enabled(&reg_path, &kind, &clsid, enabled, &name)
}

/// 删除（断开挂接，删除前强制导出 .reg + 子树前像快照）
#[tauri::command]
fn delete_entry(reg_path: String, kind: String, name: String) -> Result<(), String> {
    registry::delete_entry(&reg_path, &kind, &name)
}

/// 重启资源管理器（待生效改动落地）
#[tauri::command]
fn restart_explorer() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    std::process::Command::new("cmd")
        .args(["/C", "taskkill /f /im explorer.exe & start explorer.exe"])
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .map_err(|e| e.to_string())
        .and_then(|s| {
            if s.success() {
                Ok(())
            } else {
                Err("重启资源管理器失败".into())
            }
        })
}

/// 快照历史（按时间倒序的元信息）
#[tauri::command]
fn list_snapshots() -> Result<Vec<snapshots::SnapshotMeta>, String> {
    snapshots::list()
}

/// 还原至某时间点（还原前自动留档，可撤销；逐键写回 + 外部改动报告）
#[tauri::command]
fn restore_snapshot(id: String) -> Result<snapshots::RestoreReport, String> {
    snapshots::restore(&id)
}

/// 手动全量快照点
#[tauri::command]
fn create_manual_snapshot() -> Result<(), String> {
    snapshots::create_manual().map(|_| ())
}

/// 新增自定义菜单项（HKCU，v1 只进经典菜单）
#[tauri::command]
fn create_custom_entry(name: String, command: String, icon: String, scene: String) -> Result<(), String> {
    registry::create_custom_entry(&name, &command, &icon, &scene)
}

/// 编辑自定义菜单项（场景变化 = 旧位置删、新位置建，快照完全可逆）
#[tauri::command]
fn update_custom_entry(
    reg_path: String,
    name: String,
    command: String,
    icon: String,
    scene: String,
) -> Result<(), String> {
    registry::update_custom_entry(&reg_path, &name, &command, &icon, &scene)
}

/// 原生文件选择：选 exe/dll/ico 作为图标来源
#[tauri::command]
fn pick_icon_file() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("图标来源（exe/dll/ico）", &["exe", "dll", "ico"])
        .add_filter("所有文件", &["*"])
        .pick_file()
        .map(|p| p.to_string_lossy().to_string())
}

/// 图标预览：把选定路径的图标转成 data URL
#[tauri::command]
fn extract_icon_preview(path: String) -> Option<String> {
    crate::icons::data_url(&path, 0)
}

/// 经典菜单开关状态（以注册表键为准）
#[tauri::command]
fn classic_menu_state() -> bool {
    registry::classic_menu_state()
}

/// 一键切回/恢复经典菜单（写前自动留前像快照）
#[tauri::command]
fn set_classic_menu(on: bool) -> Result<(), String> {
    registry::set_classic_menu(on)
}

#[derive(Serialize)]
pub struct SettingsInfo {
    pub version: String,
    pub data_dir: String,
    pub data_dir_portable: bool,
    pub auto_keep: u32,
    pub policy_menu_disabled: bool,
    pub policy_sources: Vec<String>,
}

/// 设置页聚合信息（T15）
#[tauri::command]
fn get_settings() -> Result<SettingsInfo, String> {
    let pol = registry::policy_status();
    let (data_dir, data_dir_portable) = registry::data_dir_info();
    Ok(SettingsInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir,
        data_dir_portable,
        auto_keep: snapshots::auto_keep(),
        policy_menu_disabled: pol.menu_disabled,
        policy_sources: pol.sources,
    })
}

#[tauri::command]
fn set_auto_keep(n: u32) -> Result<(), String> {
    snapshots::set_auto_keep(n)
}

#[tauri::command]
fn clear_snapshots(include_protected: bool) -> Result<usize, String> {
    snapshots::clear(include_protected)
}

#[tauri::command]
fn open_data_dir() -> Result<(), String> {
    let (dir, _) = registry::data_dir_info();
    tauri_plugin_opener::open_path(dir, None::<&str>).map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            list_menu_entries,
            set_entry_enabled,
            delete_entry,
            restart_explorer,
            list_snapshots,
            restore_snapshot,
            create_manual_snapshot,
            create_custom_entry,
            update_custom_entry,
            pick_icon_file,
            extract_icon_preview,
            classic_menu_state,
            set_classic_menu,
            get_settings,
            set_auto_keep,
            clear_snapshots,
            open_data_dir
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
