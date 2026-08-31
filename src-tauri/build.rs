fn main() {
    // 「始终管理员运行」（wayfinder 绘图期决议 Q5）：
    // 注入 requireAdministrator manifest（Tauri 无内置开关，官方路径即 build.rs 注入）。
    // 注意：app.manifest 保留了 Common-Controls v6 依赖（Tauri 默认 manifest 的一部分）。
    let manifest = include_str!("app.manifest");
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new().app_manifest(manifest)),
    )
    .expect("failed to run tauri-build");
}
