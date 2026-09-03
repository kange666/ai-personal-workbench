//! Windows 的任务栏大图标与窗口小图标是两个独立入口。
//! 从 EXE 内嵌资源加载，不依赖安装路径、外置 ICO 文件或系统图标缓存。

use std::ptr::null;
use windows_sys::Win32::{
    Foundation::HWND,
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Shell::SetCurrentProcessExplicitAppUserModelID,
        WindowsAndMessaging::{
            DestroyIcon, GetSystemMetrics, LoadImageW, SendMessageW, HICON, ICON_BIG, ICON_SMALL,
            IMAGE_ICON, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
        },
    },
};

// tauri-build 为 Windows EXE 写入的图标资源编号。
const APP_ICON_RESOURCE: usize = 32512;

pub fn set_app_identity(identifier: &str) -> Result<(), String> {
    if identifier.is_empty() || identifier.contains('\0') {
        return Err("Windows 应用标识为空或包含无效字符".into());
    }
    let identifier: Vec<u16> = identifier.encode_utf16().chain(Some(0)).collect();
    // SAFETY: UTF-16 字符串以零结尾，并在同步调用期间保持有效。
    let result = unsafe { SetCurrentProcessExplicitAppUserModelID(identifier.as_ptr()) };
    if result < 0 {
        return Err(format!(
            "设置 Windows 应用标识失败：0x{:08X}",
            result as u32
        ));
    }
    Ok(())
}

// HICON 是可跨线程传递的 Windows 句柄，以整数保存以供 Tauri 管理其生命周期。
// 不使用 LR_SHARED，避免系统将同名资源的不同尺寸混用；退出时释放自有句柄。
struct OwnedIcon(usize);

impl OwnedIcon {
    fn load(width: i32, height: i32) -> Result<Self, String> {
        // SAFETY: 空模块名表示当前 EXE；整数资源编号采用 Win32 MAKEINTRESOURCE 约定。
        let icon = unsafe {
            let module = GetModuleHandleW(null());
            if module.is_null() {
                return Err(format!(
                    "读取程序图标模块失败：{}",
                    std::io::Error::last_os_error()
                ));
            }
            LoadImageW(
                module,
                APP_ICON_RESOURCE as *const u16,
                IMAGE_ICON,
                width,
                height,
                0,
            )
        };
        if icon.is_null() {
            return Err(format!(
                "读取内嵌图标失败：{}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self(icon as usize))
    }
}

impl Drop for OwnedIcon {
    fn drop(&mut self) {
        // SAFETY: 句柄仅由本实例持有，且来自不带 LR_SHARED 的 LoadImageW。
        unsafe { DestroyIcon(self.0 as HICON) };
    }
}

pub struct WindowIcons {
    small: OwnedIcon,
    large: OwnedIcon,
}

impl WindowIcons {
    fn load() -> Result<Self, String> {
        // SAFETY: GetSystemMetrics 只读取系统当前所需的图标尺寸。
        unsafe {
            Ok(Self {
                small: OwnedIcon::load(
                    GetSystemMetrics(SM_CXSMICON),
                    GetSystemMetrics(SM_CYSMICON),
                )?,
                large: OwnedIcon::load(GetSystemMetrics(SM_CXICON), GetSystemMetrics(SM_CYICON))?,
            })
        }
    }

    /// 调用方须确保窗口有效，且在窗口销毁之前保留本对象。
    unsafe fn attach(&self, hwnd: HWND) {
        // 不销毁 WM_SETICON 返回的旧句柄：旧图标仍由 Tauri/Tao 管理。
        SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, self.small.0 as isize);
        SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, self.large.0 as isize);
    }
}

pub fn install_window_icons<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<WindowIcons, String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let icons = WindowIcons::load()?;
    // SAFETY: 在 Tauri setup 的窗口线程中操作有效窗口，返回值由应用持有至退出。
    unsafe { icons.attach(hwnd.0 as HWND) };
    Ok(icons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Graphics::Gdi::{
        CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, BITMAPINFO, BITMAPINFOHEADER,
        BI_RGB, DIB_RGB_COLORS,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DestroyWindow, GetIconInfo, RegisterClassW,
        UnregisterClassW, ICONINFO, LR_LOADFROMFILE, WM_GETICON, WNDCLASSW, WS_OVERLAPPEDWINDOW,
    };

    // Rust 库测试 EXE 不链接 Tauri 的图标资源，使用同一份打包 ICO 验证原生窗口行为。
    // 实际应用仍只读取 EXE 内嵌资源，不依赖安装目录中的外置图标文件。
    fn load_packaged_icon(size: i32) -> OwnedIcon {
        let path: Vec<u16> = concat!(env!("CARGO_MANIFEST_DIR"), "/icons/icon.ico")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let icon = unsafe {
            LoadImageW(
                std::ptr::null_mut(),
                path.as_ptr(),
                IMAGE_ICON,
                size,
                size,
                LR_LOADFROMFILE,
            )
        };
        assert!(!icon.is_null(), "{}", std::io::Error::last_os_error());
        OwnedIcon(icon as usize)
    }

    #[test]
    fn icons_are_attached_to_both_windows_icon_slots() {
        let class_name: Vec<u16> = format!("AstrionIconTest-{}", uuid::Uuid::new_v4())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // 使用不显示的原生测试窗口，不启动工作台或连接真实数据库。
        unsafe {
            let module = GetModuleHandleW(null());
            let class = WNDCLASSW {
                lpfnWndProc: Some(DefWindowProcW),
                hInstance: module,
                lpszClassName: class_name.as_ptr(),
                ..std::mem::zeroed()
            };
            assert_ne!(RegisterClassW(&class), 0);
            let hwnd = CreateWindowExW(
                0,
                class_name.as_ptr(),
                class_name.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                200,
                100,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                module,
                null(),
            );
            assert!(!hwnd.is_null());
            assert_eq!(SendMessageW(hwnd, WM_GETICON, ICON_BIG as usize, 0), 0);
            let icons = WindowIcons {
                small: load_packaged_icon(16),
                large: load_packaged_icon(32),
            };
            icons.attach(hwnd);
            let small = SendMessageW(hwnd, WM_GETICON, ICON_SMALL as usize, 0);
            let large = SendMessageW(hwnd, WM_GETICON, ICON_BIG as usize, 0);
            DestroyWindow(hwnd);
            UnregisterClassW(class_name.as_ptr(), module);
            assert_eq!(small as usize, icons.small.0);
            assert_eq!(large as usize, icons.large.0);
            assert_ne!(small, 0);
            assert_ne!(large, 0);
            assert_ne!(small, large);
        }
    }

    #[test]
    fn packaged_icon_has_transparent_corners_and_opaque_artwork_at_windows_sizes() {
        for size in [16, 24, 32, 48, 64, 256] {
            let icon = load_packaged_icon(size);
            // 检查 Windows 实际读取的 ICO，而不是仅检查 PNG 源文件。
            unsafe {
                let mut info: ICONINFO = std::mem::zeroed();
                assert_ne!(GetIconInfo(icon.0 as HICON, &mut info), 0);
                let dc = CreateCompatibleDC(std::ptr::null_mut());
                let mut bitmap: BITMAPINFO = std::mem::zeroed();
                bitmap.bmiHeader = BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: size,
                    biHeight: -size,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..std::mem::zeroed()
                };
                let mut pixels = vec![0_u8; (size * size * 4) as usize];
                let rows = GetDIBits(
                    dc,
                    info.hbmColor,
                    0,
                    size as u32,
                    pixels.as_mut_ptr().cast(),
                    &mut bitmap,
                    DIB_RGB_COLORS,
                );
                DeleteObject(info.hbmColor);
                DeleteObject(info.hbmMask);
                DeleteDC(dc);
                assert_eq!(rows, size);
                for (x, y) in [(0, 0), (size - 1, 0), (0, size - 1), (size - 1, size - 1)] {
                    assert_eq!(
                        pixels[((y * size + x) * 4 + 3) as usize],
                        0,
                        "{size}px 图标角落必须真正透明"
                    );
                }
                assert_eq!(
                    pixels[(((size / 2) * size + size / 2) * 4 + 3) as usize],
                    255,
                    "中心星芒不能被抠除"
                );
            }
        }
    }

    #[test]
    fn process_identity_uses_installer_bundle_id() {
        let config: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let identifier = config["identifier"].as_str().unwrap();
        assert_eq!(identifier, "com.local.ai-personal-workbench");
        set_app_identity(identifier).unwrap();
        assert!(set_app_identity("").is_err());
        assert!(set_app_identity("bad\0identifier").is_err());
    }
}
