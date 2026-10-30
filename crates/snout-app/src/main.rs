// On Windows the app is a windowed program: no console window opens with it (what it writes
// goes to the console of the command prompt it was started from, see `platform::app_started`).
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    snout_app::run();
}
