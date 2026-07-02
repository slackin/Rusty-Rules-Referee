#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use windows::core::w;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};

fn main() {
    unsafe {
        let _ = MessageBoxW(
            HWND::default(),
            w!("Hello, World!"),
            w!("Rust GUI App"),
            MB_OK | MB_ICONINFORMATION,
        );
    }
}
