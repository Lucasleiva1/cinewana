#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = cine_wana_desktop_lib::run_libmpv_probe_if_requested() {
        std::process::exit(code);
    }
    cine_wana_desktop_lib::run()
}
