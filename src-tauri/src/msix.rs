//! MSIX 打包条目枚举（T14 / T1 调研结论落地）：
//! PackageManager 枚举全部主包 → 读各包 AppxManifest.xml →
//! 解析 desktop4/desktop5:FileExplorerContextMenus（ItemType × Verb × Clsid）。
//!
//! 限制（与 T1 结论一致）：
//! - 条目显示名在 IExplorerCommand::GetTitle 里，需要 COM 激活才能取——v1 用「包显示名 · VerbId」并标注；
//! - 逐项屏蔽复用 Blocked 键（对打包条目实测有效但官方未文档化，UI 标注）；
//! - v1 不做删除（不卸载应用包）。

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

pub struct PackagedEntry {
    pub package_display: String,
    pub verb_id: String,
    pub clsid: String,
    pub item_types: Vec<String>,
    pub icon: Option<String>,
}

pub fn enumerate() -> Result<Vec<PackagedEntry>, String> {
    use windows::Management::Deployment::{PackageManager, PackageTypes};

    let pm = PackageManager::new().map_err(|e| format!("PackageManager 不可用: {e}"))?;
    let pkgs = pm
        .FindPackagesWithPackageTypes(PackageTypes::Main)
        .map_err(|e| format!("枚举应用包失败: {e}"))?;

    let it = pkgs.First().map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    loop {
        if !it.HasCurrent().map_err(|e| e.to_string())? {
            break;
        }
        let pkg = it.Current().map_err(|e| e.to_string())?;
        if let Ok(install) = pkg.InstalledPath() {
            let install_s = install.to_string();
            let manifest = std::path::Path::new(&install_s).join("AppxManifest.xml");
            if let Ok(text) = std::fs::read_to_string(&manifest) {
                let display = pkg
                    .DisplayName()
                    .ok()
                    .map(|h| h.to_string())
                    .filter(|s| {
                        !s.trim().is_empty() && !s.starts_with("ms-resource:")
                    })
                    .unwrap_or_else(|| {
                        pkg.Id()
                            .ok()
                            .and_then(|i| i.Name().ok().map(|n| n.to_string()))
                            .unwrap_or_else(|| "未知应用包".into())
                    });
                let logo = manifest_logo(&text, &install_s);
                collect_entries(&text, &display, logo.as_deref(), &mut out);
            }
        }
        if !it.MoveNext().map_err(|e| e.to_string())? {
            break;
        }
    }
    Ok(out)
}

/// 包 Properties/Logo 的 data URL（ms-resource 或文件缺失则放弃）
fn manifest_logo(text: &str, install: &str) -> Option<String> {
    let doc = roxmltree::Document::parse(text).ok()?;
    let node = doc.descendants().find(|n| n.tag_name().name() == "Logo")?;
    let rel = node.text()?.trim();
    if rel.starts_with("ms-resource:") {
        return None;
    }
    let p = std::path::Path::new(install).join(rel.replace('/', "\\"));
    if !p.exists() {
        return None;
    }
    let ext = p.extension()?.to_str()?.to_ascii_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return None,
    };
    let bytes = std::fs::read(&p).ok()?;
    Some(format!("data:{mime};base64,{}", B64.encode(bytes)))
}

fn collect_entries(
    text: &str,
    display: &str,
    logo: Option<&str>,
    out: &mut Vec<PackagedEntry>,
) {
    let Ok(doc) = roxmltree::Document::parse(text) else {
        return;
    };
    for node in doc.descendants() {
        // 按本地名匹配，同时覆盖 desktop4（文件项）与 desktop5（目录背景）命名空间
        if node.tag_name().name() != "FileExplorerContextMenus" {
            continue;
        }
        let mut types: Vec<String> = Vec::new();
        let mut verbs: Vec<(String, String)> = Vec::new();
        for ch in node.children() {
            match ch.tag_name().name() {
                "ItemType" => {
                    if let Some(t) = ch.attribute("Type") {
                        types.push(t.to_string());
                    }
                }
                "Verb" => {
                    if let (Some(id), Some(clsid)) = (ch.attribute("Id"), ch.attribute("Clsid")) {
                        verbs.push((id.to_string(), clsid.to_uppercase()));
                    }
                }
                _ => {}
            }
        }
        for (verb_id, clsid) in verbs {
            out.push(PackagedEntry {
                package_display: display.to_string(),
                verb_id,
                clsid,
                item_types: types.clone(),
                icon: logo.map(|s| s.to_string()),
            });
        }
    }
}
