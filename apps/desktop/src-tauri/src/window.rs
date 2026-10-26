// SPDX-License-Identifier: MIT OR Apache-2.0

//! The window frame (docs/08, "Navigation"). The main window has no system title bar: the
//! webview draws its own in the theme's colours, and its caption buttons and drag area call
//! these. Each acts only on the window that called it and takes no arguments, so the
//! capability still grants GMM's own commands and nothing from Tauri's window plugin.
//!
//! The first size is worked out from the screen when the app starts ([`fit_to_screen`]).

use tauri::{LogicalSize, Runtime, Window};

use crate::error::{CmdError, CmdResult};

fn failed(e: tauri::Error) -> CmdError {
    CmdError::other(format!("the window didn't respond: {e}"))
}

#[tauri::command]
pub fn window_minimize<R: Runtime>(window: Window<R>) -> CmdResult<()> {
    window.minimize().map_err(failed)
}

/// Maximizes the window, or restores it when it already is. Returns whether it's maximized.
#[tauri::command]
pub fn window_toggle_maximize<R: Runtime>(window: Window<R>) -> CmdResult<bool> {
    if window.is_maximized().map_err(failed)? {
        window.unmaximize().map_err(failed)?;
        Ok(false)
    } else {
        window.maximize().map_err(failed)?;
        Ok(true)
    }
}

#[tauri::command]
pub fn window_is_maximized<R: Runtime>(window: Window<R>) -> CmdResult<bool> {
    window.is_maximized().map_err(failed)
}

#[tauri::command]
pub fn window_close<R: Runtime>(window: Window<R>) -> CmdResult<()> {
    window.close().map_err(failed)
}

/// Starts moving the window with the mouse (a press on the title bar).
#[tauri::command]
pub fn window_start_drag<R: Runtime>(window: Window<R>) -> CmdResult<()> {
    window.start_dragging().map_err(failed)
}

/// The size the window opens at: 16:9, 1952 wide on a screen with room for it (what the
/// project owner settled on, 2026-10-26, on a 2560×1440 screen), otherwise about three
/// quarters of the screen's width, never taller than nine tenths of its height. Centred.
pub fn fit_to_screen<R: Runtime>(window: &tauri::WebviewWindow<R>) {
    const WIDEST: f64 = 1952.0;
    let Ok(Some(monitor)) = window
        .current_monitor()
        .or_else(|_| window.primary_monitor())
    else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let mut width = WIDEST.min(area.width * 0.76);
    let mut height = width * 9.0 / 16.0;
    if height > area.height * 0.9 {
        height = area.height * 0.9;
        width = height * 16.0 / 9.0;
    }
    let _ = window.set_size(LogicalSize::new(width.round(), height.round()));
    let _ = window.center();
}
