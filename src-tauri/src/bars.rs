// Her taskbar on the other displays (taskbarDisplays: all): a strip on each
// display but hers, a window of its own (bar-1, bar-2, ...: pet/bar.html),
// its app bar along the same edge as hers, with Start, the windows there and
// the clock; her capsule, the tray and the widgets stay on hers, as Windows'
// own keeps its tray on the main display (the appearance session's spec,
// 2026-10-13). Which windows show on which follows Windows' own setting for
// taskbars on several displays (MMTaskbarMode). The main display only (the
// default): Windows' own taskbar on the others (shell.rs home_display).
// Not while her home is the island, nor while her taskbar is not up; an
// app full screen on a display hides that display's alone.
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::appbar::{self, Edge};
use crate::{island, screen::Area, taskbar, Shared};

type Mon = (i32, i32, i32, i32);

struct Bar {
    label: String,
    mon: Mon,
    sf: f64,
    edge: Edge,
    // Its window's handle once made (0 before), and its app bar's rect.
    hwnd: isize,
    strip: Option<Mon>,
    ready: bool,
    // An app full screen on its display.
    hidden: bool,
    // What its page was last told.
    sent: Value,
}

static BARS: Mutex<Vec<Bar>> = Mutex::new(Vec::new());
static NEXT: AtomicU32 = AtomicU32::new(1);
// Whether the others had hers when last looked (Windows' own put away on them).
static ALL_ON: Mutex<Option<bool>> = Mutex::new(None);

// Her taskbar wanted on every display: her home the taskbar, and set so.
pub fn wanted(sh: &Shared) -> bool {
    sh.home() == "taskbar" && sh.setting("taskbarDisplays").as_str() == Some("all")
}

fn window(sh: &Shared, label: &str) -> Option<WebviewWindow> {
    sh.app.get_webview_window(label)
}

// Made or taken away as the displays and her settings say: one for each
// display but hers, while her taskbar is up. Windows' own taskbar put away
// on every display while they are (none of them left without one), on hers
// alone otherwise.
pub fn sync(sh: &Arc<Shared>) {
    let up = taskbar::is_up();
    let all = wanted(sh) && up;
    let home = island::display(sh);
    let edge = island::edge(sh);
    let want: Vec<Area> = if all { sh.screens().areas.into_iter().filter(|a| Some(a.mon) != home).collect() } else { Vec::new() };
    let (gone, made, moved) = {
        let mut bars = BARS.lock().unwrap();
        let mut gone = Vec::new();
        bars.retain(|b| {
            let keep = want.iter().any(|a| a.mon == b.mon && a.sf == b.sf) && b.edge == edge;
            if !keep {
                gone.push((b.label.clone(), b.hwnd));
            }
            keep
        });
        let mut made = Vec::new();
        for a in &want {
            if !bars.iter().any(|b| b.mon == a.mon) {
                let label = format!("bar-{}", NEXT.fetch_add(1, Ordering::SeqCst));
                bars.push(Bar { label: label.clone(), mon: a.mon, sf: a.sf, edge, hwnd: 0, strip: None, ready: false, hidden: false, sent: Value::Null });
                made.push((label, *a));
            }
        }
        let moved = gone.len() + made.len() > 0;
        (gone, made, moved)
    };
    for (label, hwnd) in gone {
        close(sh, &label, hwnd);
    }
    for (label, area) in made {
        create(sh, label, area, edge);
    }
    if moved {
        sh.log(&format!("bars: {} on the other displays", BARS.lock().unwrap().len()));
    }
    // Windows' own on the others: put away while hers are there, shown again
    // once they are not.
    if up {
        let mut was = ALL_ON.lock().unwrap();
        if *was != Some(all) {
            *was = Some(all);
            crate::shell::home_display(if all { None } else { home });
            crate::shell::rehide();
        }
    }
}

// Every one of them gone (quitting, another home): their app bars given back first.
pub fn close_all(sh: &Shared) {
    let all: Vec<(String, isize)> = BARS.lock().unwrap().drain(..).map(|b| (b.label, b.hwnd)).collect();
    for (label, hwnd) in all {
        close(sh, &label, hwnd);
    }
    *ALL_ON.lock().unwrap() = None;
}

fn close(sh: &Shared, label: &str, hwnd: isize) {
    if hwnd != 0 {
        appbar::remove(hwnd);
    }
    if let Some(win) = window(sh, label) {
        let _ = win.destroy();
    }
    sh.log(&format!("bars: {label} gone"));
}

// Its window made off the main thread (a menu click may be what asks, and
// building there waits on itself), its strip asked of Windows along the
// edge of its own display, as thick as hers at that display's scale.
fn create(sh: &Arc<Shared>, label: String, area: Area, edge: Edge) {
    let sh = sh.clone();
    std::thread::spawn(move || {
        let thick = (island::TASKBAR_H * area.sf).round() as i32;
        let strip = edge.strip(area.mon, thick);
        let built = WebviewWindowBuilder::new(&sh.app, &label, WebviewUrl::App("pet/bar.html".into()))
            .title("wakuwaku bar")
            .inner_size(strip.2 as f64 / area.sf, strip.3 as f64 / area.sf)
            .additional_browser_args(crate::BROWSER_ARGS)
            .data_directory(crate::webview_dir(&sh))
            .transparent(true)
            .decorations(false)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .skip_taskbar(true)
            .shadow(false)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .focusable(false)
            .focused(false)
            .visible(false)
            .build();
        let win = match built {
            Ok(win) => win,
            Err(err) => return sh.log(&format!("bars: {label} not made: {err}")),
        };
        #[cfg(windows)]
        let hwnd = win.hwnd().map(|h| h.0 as isize).unwrap_or(0);
        #[cfg(not(windows))]
        let hwnd = 0;
        crate::tasks::no_transitions(hwnd);
        sh.own_window(&win);
        let granted = if hwnd != 0 && appbar::register(hwnd) { appbar::place(hwnd, area.mon, edge, thick) } else { strip };
        let _ = win.set_size(PhysicalSize::new(granted.2.max(1) as u32, granted.3.max(1) as u32));
        let _ = win.set_position(PhysicalPosition::new(granted.0, granted.1));
        let gone = {
            let mut bars = BARS.lock().unwrap();
            match bars.iter_mut().find(|b| b.label == label) {
                Some(b) => {
                    b.hwnd = hwnd;
                    b.strip = Some(granted);
                    false
                }
                None => true,
            }
        };
        // Taken away again while it was being made.
        if gone {
            return close(&sh, &label, hwnd);
        }
        let _ = win.show();
        sh.log(&format!("bars: {label} on {:?} at {}%, strip {granted:?} along the {edge:?}", area.mon, (area.sf * 100.0).round()));
    });
}

// Its page has its listeners up: told what to show.
#[tauri::command]
pub fn bar_ready(app: tauri::AppHandle, window: WebviewWindow) {
    let sh = crate::shared(&app);
    if let Some(b) = BARS.lock().unwrap().iter_mut().find(|b| b.label == window.label()) {
        b.ready = true;
        b.sent = Value::Null;
    }
    refresh(&sh);
}

// An app full screen covering a display (or none): that display's taskbar
// steps aside, the others come back.
pub fn full_screen(sh: &Shared, cover: Option<Mon>) {
    let changed: Vec<(String, bool)> = {
        let mut bars = BARS.lock().unwrap();
        bars.iter_mut()
            .filter_map(|b| {
                let hide = cover == Some(b.mon);
                (hide != b.hidden).then(|| {
                    b.hidden = hide;
                    (b.label.clone(), hide)
                })
            })
            .collect()
    };
    for (label, hide) in changed {
        if let Some(win) = window(sh, &label) {
            let _ = if hide { win.hide() } else { win.show() };
        }
        sh.log(&format!("bars: {label} {}", if hide { "stepped aside (an app full screen there)" } else { "back" }));
    }
}

// Windows' setting for taskbars on several displays (Settings > Taskbar >
// Show my taskbar apps on): 0 all taskbars, 1 the main one and where the
// window is open, 2 only where it is open. 0 when not set.
fn spread() -> u32 {
    #[cfg(windows)]
    {
        use winreg::enums::HKEY_CURRENT_USER;
        winreg::RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced")
            .and_then(|k| k.get_value::<u32, _>("MMTaskbarMode"))
            .unwrap_or(0)
    }
    #[cfg(not(windows))]
    {
        0
    }
}

// The buttons a taskbar shows of all (taskbar.rs, each window with the
// display it is on, mon): on the main display all of them, unless only
// where open (2); on another display all of them (0), or only those of
// its own windows (1, 2), the pinned ones then on the main one alone.
fn buttons_for(all: &Value, mon: Mon, main: bool, spread: u32) -> Value {
    let on_it = |w: &Value| w["mon"].as_array().is_some_and(|m| m.iter().filter_map(Value::as_i64).map(|v| v as i32).eq([mon.0, mon.1, mon.2, mon.3]));
    let keep_all = spread == 0 || (main && spread == 1);
    let apps = all.as_array().into_iter().flatten().filter_map(|app| {
        let windows: Vec<Value> = app["windows"].as_array().into_iter().flatten().filter(|w| keep_all || on_it(w)).cloned().collect();
        let pinned = app["pinned"].as_bool() == Some(true) && (spread == 0 || main);
        (pinned || !windows.is_empty()).then(|| {
            let mut app = app.clone();
            app["windows"] = Value::Array(windows);
            app
        })
    });
    Value::Array(apps.collect())
}

// The buttons on her own taskbar, while hers are on the others too.
pub fn for_main(sh: &Shared, all: &Value) -> Value {
    if !wanted(sh) || BARS.lock().unwrap().is_empty() {
        return all.clone();
    }
    match island::display(sh) {
        Some(mon) => buttons_for(all, mon, true, spread()),
        None => all.clone(),
    }
}

// Each page told what to show, if it changed: the windows on its display,
// the look of hers (edge, its buttons' alignment, the material, light or
// dark and the accent), her language.
pub fn refresh(sh: &Shared) {
    let bars: Vec<(String, Mon, f64, Value)> = BARS.lock().unwrap().iter().filter(|b| b.ready).map(|b| (b.label.clone(), b.mon, b.sf, b.sent.clone())).collect();
    if bars.is_empty() {
        return;
    }
    let all = taskbar::windows_now();
    let look = taskbar::look_value();
    let spread = spread();
    let edge = island::edge(sh);
    for (label, mon, sf, sent) in bars {
        let state = json!({
            "edge": edge.word(),
            "align": sh.setting("taskbarAlign"),
            "material": sh.setting("taskbarMaterial"),
            "look": look,
            "lang": sh.lang(),
            "sf": sf,
            "windows": buttons_for(&all, mon, false, spread),
        });
        if state != sent {
            if let Some(b) = BARS.lock().unwrap().iter_mut().find(|b| b.label == label) {
                b.sent = state.clone();
            }
            let _ = sh.app.emit_to(label.as_str(), "bar:state", state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_by_windows_own_setting() {
        let main = (0, 0, 2560, 1440);
        let other = (2560, -150, 1829, 1143);
        let all = json!([
            { "app": "chrome", "pinned": true, "windows": [{ "id": 1, "mon": [0, 0, 2560, 1440] }, { "id": 2, "mon": [2560, -150, 1829, 1143] }] },
            { "app": "notes", "pinned": true, "windows": [] },
            { "app": "code", "pinned": false, "windows": [{ "id": 3, "mon": [2560, -150, 1829, 1143] }] },
        ]);
        let ids = |v: &Value| -> Vec<(String, Vec<i64>)> {
            v.as_array().unwrap().iter().map(|a| (a["app"].as_str().unwrap().to_string(), a["windows"].as_array().unwrap().iter().map(|w| w["id"].as_i64().unwrap()).collect())).collect()
        };
        // All taskbars (0): every one everywhere.
        assert_eq!(ids(&buttons_for(&all, other, false, 0)).len(), 3);
        // The main one and where open (1): the main all; the other its own, no pinned ones without a window there.
        assert_eq!(ids(&buttons_for(&all, main, true, 1)), ids(&all));
        assert_eq!(ids(&buttons_for(&all, other, false, 1)), vec![("chrome".to_string(), vec![2]), ("code".to_string(), vec![3])]);
        // Only where open (2): the main its own and the pinned; the other its own.
        assert_eq!(ids(&buttons_for(&all, main, true, 2)), vec![("chrome".to_string(), vec![1]), ("notes".to_string(), vec![])]);
        assert_eq!(ids(&buttons_for(&all, other, false, 2)), vec![("chrome".to_string(), vec![2]), ("code".to_string(), vec![3])]);
    }
}
