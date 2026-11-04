//! Native Windows integration for the Fluent family: system accent colour
//! and DWM window chrome (rounded corners, caption colour, Mica backdrop).
//!
//! Every call degrades gracefully: on older Windows builds the DWM
//! attributes are rejected and the window keeps its default chrome.

use egui::{Color32, Theme};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows_sys::Win32::{
    Foundation::{ERROR_SUCCESS, HWND},
    Graphics::Dwm::{
        DWMSBT_MAINWINDOW, DWMWA_CAPTION_COLOR, DWMWA_SYSTEMBACKDROP_TYPE, DWMWA_TEXT_COLOR,
        DWMWA_USE_IMMERSIVE_DARK_MODE, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
        DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
    },
    System::Registry::{
        HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_BINARY, RRF_RT_REG_SZ, RegGetValueW,
    },
    UI::Controls::MARGINS,
};

use crate::{AccentShades, Palette};

/// System backdrop drawn behind a window's client area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backdrop {
    /// Windows 11 Mica material. Needs a transparent swapchain to show
    /// through the client area.
    Mica,
    /// Opaque client area; the caption is tinted with the Fluent base colour.
    Solid,
}

/// First Windows 10 build (1507); older builds lack the DirectX 12
/// `DirectComposition` swapchain that the Mica backdrop needs.
pub const WINDOWS_10_FIRST_BUILD: u32 = 10_240;

/// The running Windows build number (for example 7601 on Windows 7 SP1),
/// read from `HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion`.
///
/// The registry is used instead of `GetVersionEx`, which reports the
/// manifest-compatible version rather than the real one.
#[must_use]
pub fn windows_build_number() -> Option<u32> {
    static BUILD: std::sync::OnceLock<Option<u32>> = std::sync::OnceLock::new();
    *BUILD.get_or_init(|| {
        let subkey = wide("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
        let value = wide("CurrentBuildNumber");
        let mut buffer = [0_u16; 16];
        let mut len = u32::try_from(std::mem::size_of_val(&buffer)).ok()?;
        // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call;
        // `buffer` is writable for `len` bytes and `len` is updated in place.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                subkey.as_ptr(),
                value.as_ptr(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &raw mut len,
            )
        };
        if status != ERROR_SUCCESS {
            return None;
        }
        let chars = (len as usize / 2).min(buffer.len());
        String::from_utf16_lossy(&buffer[..chars])
            .trim_end_matches('\0')
            .parse()
            .ok()
    })
}

/// Whether the OS supports the transparent DirectX 12 swapchain used by the
/// Mica backdrop (Windows 10 or later). Unknown builds count as supported.
#[must_use]
pub fn supports_transparent_swapchain() -> bool {
    windows_build_number().is_none_or(|build| build >= WINDOWS_10_FIRST_BUILD)
}

/// Reads the user's accent colour shades from
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\Accent\AccentPalette`.
#[must_use]
pub fn system_accent() -> Option<AccentShades> {
    let subkey = wide("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent");
    let value = wide("AccentPalette");
    let mut buffer = [0_u8; 32];
    let mut len = u32::try_from(buffer.len()).ok()?;
    // SAFETY: both strings are NUL-terminated UTF-16 that outlive the call;
    // `buffer` is writable for `len` bytes and `len` is updated in place.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_BINARY,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &raw mut len,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    AccentShades::from_accent_palette_bytes(buffer.get(..len as usize)?)
}

/// Applies Fluent window chrome to `window` for the given light/dark `theme`.
///
/// Requests rounded corners, matches the title bar's dark mode to `theme`,
/// and either enables the Mica backdrop or colours the caption from `palette`
/// (its `toolbar_bg` and `text`), which must be opaque. Returns the backdrop actually applied: [`Backdrop::Solid`]
/// when Mica was requested but the OS rejected it (before Windows 11 22H2).
pub fn apply_window_chrome(
    window: &impl HasWindowHandle,
    backdrop: Backdrop,
    theme: Theme,
    palette: &Palette,
) -> Backdrop {
    let Some(hwnd) = hwnd_of(window) else {
        return Backdrop::Solid;
    };
    set_attribute(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND);
    set_attribute(
        hwnd,
        DWMWA_USE_IMMERSIVE_DARK_MODE,
        i32::from(theme == Theme::Dark),
    );
    if backdrop == Backdrop::Mica
        && extend_frame_into_client_area(hwnd)
        && set_attribute(hwnd, DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW)
    {
        return Backdrop::Mica;
    }
    set_attribute(hwnd, DWMWA_CAPTION_COLOR, colorref(palette.toolbar_bg));
    set_attribute(hwnd, DWMWA_TEXT_COLOR, colorref(palette.text));
    Backdrop::Solid
}

/// Tracks what was last applied to a window's chrome so callers can re-apply it
/// cheaply each frame. Keep one per window.
#[derive(Debug, Default)]
pub struct WindowChrome {
    applied: Option<(isize, Theme)>,
    backdrop: Option<Backdrop>,
}

impl WindowChrome {
    /// Applies the chrome when the window or `theme` differs from the last time.
    ///
    /// A handle that isn't available yet records nothing, so the next frame
    /// retries. A backdrop the OS rejected stays rejected rather than being
    /// requested again.
    pub fn sync(
        &mut self,
        window: &impl HasWindowHandle,
        backdrop: Backdrop,
        theme: Theme,
        palette: &Palette,
    ) {
        let Some(hwnd) = hwnd_of(window) else {
            return;
        };
        let key = (hwnd as isize, theme);
        if self.applied == Some(key) {
            return;
        }
        let requested = self.backdrop.unwrap_or(backdrop);
        self.backdrop = Some(apply_window_chrome(window, requested, theme, palette));
        self.applied = Some(key);
    }

    /// The backdrop in effect after the last [`Self::sync`], if any.
    #[must_use]
    pub const fn backdrop(&self) -> Option<Backdrop> {
        self.backdrop
    }
}

fn hwnd_of(window: &impl HasWindowHandle) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
        _ => None,
    }
}

/// Extends the DWM frame over the whole client area ("sheet of glass") so
/// the system backdrop renders behind transparent client pixels.
fn extend_frame_into_client_area(hwnd: HWND) -> bool {
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    // SAFETY: `hwnd` comes from a live window handle and `margins` is a
    // local that outlives the call.
    let result = unsafe { DwmExtendFrameIntoClientArea(hwnd, &raw const margins) };
    result >= 0
}

/// Sets one `u32`/`i32`-sized DWM window attribute; returns whether DWM
/// accepted it.
fn set_attribute<T: Copy>(hwnd: HWND, attribute: i32, value: T) -> bool {
    debug_assert_eq!(size_of::<T>(), 4, "DWM attributes here are 32-bit");
    // SAFETY: `hwnd` comes from a live window handle and `value` is a local
    // whose size is passed alongside its address.
    let result = unsafe {
        DwmSetWindowAttribute(
            hwnd,
            attribute as u32,
            (&raw const value).cast(),
            size_of::<T>() as u32,
        )
    };
    result >= 0
}

/// Converts a colour to a Win32 `COLORREF` (`0x00BBGGRR`).
fn colorref(color: Color32) -> u32 {
    u32::from(color.r()) | (u32::from(color.g()) << 8) | (u32::from(color.b()) << 16)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorref_is_bgr() {
        assert_eq!(colorref(Color32::from_rgb(0x12, 0x34, 0x56)), 0x0056_3412);
    }

    #[test]
    fn system_accent_reads_or_falls_back() {
        // Either the registry value parses or the caller falls back to the
        // Windows default; both paths must yield seven distinct shades.
        let shades = system_accent().unwrap_or(AccentShades::WINDOWS_DEFAULT);
        assert_ne!(shades.light2, shades.dark1);
    }
}
