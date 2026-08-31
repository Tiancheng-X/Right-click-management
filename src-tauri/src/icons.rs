//! 图标提取：从 exe / dll 提取真实图标，编码为 PNG data URL 供前端渲染。
//! SHDefExtractIcon(32px) → HICON → GetDIBits(BGRA) → 反预乘 → PNG → base64。
//! 结果按「路径|索引」缓存（含失败结果），重复扫描零开销。

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use winapi::shared::minwindef::FALSE;
use winapi::shared::windef::HICON;
use winapi::um::wingdi::{DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS};
use winapi::um::winuser::{DestroyIcon, GetDC, GetIconInfo, ReleaseDC, ICONINFO};

// winapi 0.3 未导出 user32 的 PrivateExtractIconsW，自行声明
#[link(name = "user32")]
extern "system" {
    fn PrivateExtractIconsW(
        lpszfilename: *const u16,
        niconindex: u32,
        cxicon: u32,
        cyicon: u32,
        phicon: *mut HICON,
        piconid: *mut u32,
        nicons: u32,
        flags: u32,
    ) -> u32;
}

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 提取图标并返回 `data:image/png;base64,...`；失败返回 None（前端回退占位图形）
pub fn data_url(path: &str, index: i32) -> Option<String> {
    if path.trim().is_empty() {
        return None;
    }
    let key = format!("{}|{}", path.to_lowercase(), index);
    if let Some(hit) = cache().lock().unwrap().get(&key) {
        return hit.clone();
    }
    let out = extract(path, index);
    cache().lock().unwrap().insert(key, out.clone());
    out
}

fn extract(path: &str, index: i32) -> Option<String> {
    unsafe {
        let path_w: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut hicon: HICON = std::ptr::null_mut();
        // 提取 32×32 图标；返回提取数量，0 = 失败
        if PrivateExtractIconsW(
            path_w.as_ptr(),
            index as u32,
            32,
            32,
            &mut hicon,
            std::ptr::null_mut(),
            1,
            0,
        ) == 0
            || hicon.is_null()
        {
            return None;
        }

        let mut info: ICONINFO = std::mem::zeroed();
        if GetIconInfo(hicon, &mut info) == FALSE {
            DestroyIcon(hicon);
            return None;
        }

        let mut out: Option<String> = None;
        const W: i32 = 32;
        const H: i32 = 32;
        if !info.hbmColor.is_null() {
            let mut bmi: BITMAPINFO = std::mem::zeroed();
            bmi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = W;
            bmi.bmiHeader.biHeight = -H; // 负值 = 自上而下
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = BI_RGB;

            let mut buf = vec![0u8; (W * H * 4) as usize];
            let hdc = GetDC(std::ptr::null_mut());
            let lines = GetDIBits(
                hdc,
                info.hbmColor,
                0,
                H as u32,
                buf.as_mut_ptr().cast(),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            ReleaseDC(std::ptr::null_mut(), hdc);

            if lines == H {
                // BGRA → RGBA，并按 alpha 反预乘（Windows 位图为预乘格式）
                for px in buf.chunks_exact_mut(4) {
                    let (b, g, r, a) = (px[0], px[1], px[2], px[3]);
                    if a == 0 {
                        px[0..4].fill(0);
                        continue;
                    }
                    px[0] = ((r as u16 * 255) / a as u16) as u8;
                    px[1] = ((g as u16 * 255) / a as u16) as u8;
                    px[2] = ((b as u16 * 255) / a as u16) as u8;
                    px[3] = a;
                }
                if let Some(img) = image::RgbaImage::from_raw(W as u32, H as u32, buf) {
                    let mut png = std::io::Cursor::new(Vec::new());
                    if img.write_to(&mut png, image::ImageFormat::Png).is_ok() {
                        out = Some(format!("data:image/png;base64,{}", B64.encode(png.get_ref())));
                    }
                }
            }
            DeleteObject(info.hbmColor as *mut _);
        }
        if !info.hbmMask.is_null() {
            DeleteObject(info.hbmMask as *mut _);
        }
        DestroyIcon(hicon);
        out
    }
}
