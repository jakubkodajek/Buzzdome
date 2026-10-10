// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    quiz_show_lib::sheet_reader::serve_if_asked();
    quiz_show_lib::run()
}
