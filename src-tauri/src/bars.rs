// Her taskbar on the other displays (taskbarDisplays: all): a strip on each
// display but hers, a window of its own (bar-1, bar-2, ...: pet/bar.html),
// its app bar along the same edge as hers, with Start, the windows there and
// the clock; her capsule, the tray and the widgets stay on hers, as Windows'
// own keeps its tray on the main display (the appearance session's spec,
// 2026-10-13), but for a prompt waiting: then her capsule comes at the
// start of each, "waiting for your approval · N", a press showing it in her
// island. Which windows show on which follows Windows' own setting for
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
        // A list open over one gone goes with it.
        send_pop(sh);
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
    hide_pop(sh);
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

// The prompts waiting, for her capsule there (the appearance session's spec,
// 2026-10-13): how many, and "answer" when the one shown first is a
// question, "approve" otherwise (a permission, a plan); none when none wait.
fn waiting(views: &Value) -> Value {
    let list = views.as_array().map(Vec::as_slice).unwrap_or_default();
    match list.first() {
        Some(first) => json!({ "n": list.len(), "kind": if first["kind"] == "question" { "answer" } else { "approve" } }),
        None => Value::Null,
    }
}

// Her capsule there pressed: the prompt shown in her island on the main
// display, as a press on hers (island.rs show_asks).
#[tauri::command]
pub fn bar_asks(app: tauri::AppHandle) {
    let sh = crate::shared(&app);
    if !sh.asks.lock().unwrap().is_empty() {
        island::show_asks(&sh);
    }
}

// One of them up on a display no app full screen covers: her capsule there
// shows the prompts waiting (main.rs asks_seen).
pub fn any_shown() -> bool {
    BARS.lock().unwrap().iter().any(|b| b.ready && !b.hidden && b.strip.is_some())
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
        // A list open over one stepped aside goes with it.
        if hide {
            send_pop(sh);
        }
    }
}

// --- A program's windows or what can be done with it, over its button -----------------
// As on her own taskbar (island.js openPop): the pointer resting on a
// running program's button, its windows, each a card with its live picture
// (Windows draws them, taskbar.rs thumbs); a right press, its menu (another
// window, keep it on the taskbar or no longer, close). A strip there is too
// thin to hold them: one window of their own (barpop: pet/barpop.html),
// made the first time, laid against the strip over the button, for any of
// them. It goes once the pointer has left the button and it a moment
// (barpop.js), at a press anywhere else, or when its program goes.
const POP: &str = "barpop";

struct Pop {
    // The strip it is for, list or menu, the program, its button there (the
    // strip's own px), and where the window is once laid (physical).
    bar: String,
    kind: String,
    key: String,
    at: (f64, f64, f64, f64),
    rect: Option<Mon>,
}

static OPEN: Mutex<Option<Pop>> = Mutex::new(None);
static POP_MADE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static POP_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

// A strip's display, scale, edge and rect, by its window's label.
fn bar_of(label: &str) -> Option<(Mon, f64, Edge, Mon)> {
    BARS.lock().unwrap().iter().find(|b| b.label == label && !b.hidden).and_then(|b| b.strip.map(|s| (b.mon, b.sf, b.edge, s)))
}

// From a strip's page: the windows (list) or the menu of a program, its
// button at (x, y, w, h) in the page's px.
#[tauri::command]
pub fn bar_pop(app: tauri::AppHandle, window: WebviewWindow, kind: String, key: String, at: (f64, f64, f64, f64)) {
    let sh = crate::shared(&app);
    if bar_of(window.label()).is_none() {
        return;
    }
    *OPEN.lock().unwrap() = Some(Pop { bar: window.label().to_string(), kind, key, at, rect: None });
    if !POP_MADE.swap(true, Ordering::SeqCst) {
        return make_pop(&sh);
    }
    send_pop(&sh);
}

// The pointer on a strip's buttons or off them (bar.js), for the list to
// stay while it goes from one to the other (barpop.js).
#[tauri::command]
pub fn bar_pop_over(app: tauri::AppHandle, window: WebviewWindow, over: bool) {
    let open = OPEN.lock().unwrap().as_ref().is_some_and(|p| p.bar == window.label());
    if open {
        let _ = app.emit_to(POP, "barpop:bar", json!({ "over": over }));
    }
}

fn make_pop(sh: &Arc<Shared>) {
    let sh = sh.clone();
    std::thread::spawn(move || {
        let built = WebviewWindowBuilder::new(&sh.app, POP, WebviewUrl::App("pet/barpop.html".into()))
            .title("wakuwaku bar list")
            .inner_size(420.0, 300.0)
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
        match built {
            Ok(win) => {
                #[cfg(windows)]
                crate::tasks::no_transitions(win.hwnd().map(|h| h.0 as isize).unwrap_or(0));
                sh.own_window(&win);
            }
            Err(err) => {
                POP_MADE.store(false, Ordering::SeqCst);
                sh.log(&format!("bars: the list's window not made: {err}"));
            }
        }
    });
}

// Its page has its listeners up.
#[tauri::command]
pub fn barpop_ready(app: tauri::AppHandle) {
    POP_READY.store(true, Ordering::SeqCst);
    send_pop(&crate::shared(&app));
}

// What the open one shows, told its page (again as the windows change): the
// program as that strip shows it, and the look; gone with its program, or
// with its last window for a list.
fn send_pop(sh: &Shared) {
    if !POP_READY.load(Ordering::SeqCst) {
        return;
    }
    let Some((bar, kind, key)) = OPEN.lock().unwrap().as_ref().map(|p| (p.bar.clone(), p.kind.clone(), p.key.clone())) else { return };
    let Some((mon, _, edge, _)) = bar_of(&bar) else { return hide_pop(sh) };
    let all = buttons_for(&taskbar::windows_now(), mon, false, spread());
    let app = all.as_array().into_iter().flatten().find(|a| a["app"].as_str() == Some(key.as_str())).cloned();
    let Some(app) = app.filter(|a| kind == "menu" || a["windows"].as_array().is_some_and(|w| !w.is_empty())) else { return hide_pop(sh) };
    let _ = sh.app.emit_to(
        POP,
        "barpop:show",
        json!({ "kind": kind, "app": app, "edge": edge.word(), "look": taskbar::look_value(), "lang": sh.lang() }),
    );
}

// Its page laid out (w, h: its px, with room for its shadow away from the
// strip): the window laid against the strip over the button, and shown.
#[tauri::command]
pub fn barpop_place(app: tauri::AppHandle, w: f64, h: f64) {
    let sh = crate::shared(&app);
    let Some((bar, at)) = OPEN.lock().unwrap().as_ref().map(|p| (p.bar.clone(), p.at)) else { return };
    let Some((mon, sf, edge, strip)) = bar_of(&bar) else { return hide_pop(&sh) };
    let px = |v: f64| (v * sf).round() as i32;
    let button = (strip.0 + px(at.0), strip.1 + px(at.1), px(at.2), px(at.3));
    let rect = pop_rect(edge, strip, mon, button, (px(w), px(h)));
    let Some(win) = sh.app.get_webview_window(POP) else { return };
    // Its place first: moved to a display of another scale, the window is
    // given a size for that one, which this then replaces.
    let _ = win.set_position(PhysicalPosition::new(rect.0, rect.1));
    let _ = win.set_size(PhysicalSize::new(rect.2.max(1) as u32, rect.3.max(1) as u32));
    let _ = win.set_position(PhysicalPosition::new(rect.0, rect.1));
    let _ = win.show();
    if let Some(p) = OPEN.lock().unwrap().as_mut() {
        p.rect = Some(rect);
    }
    taskbar::watch_presses(taskbar::WATCH_BARS, true);
}

// Where the list goes (physical): against the strip on the screen's side,
// centred on the button along it, within its display.
fn pop_rect(edge: Edge, strip: Mon, mon: Mon, button: Mon, (w, h): (i32, i32)) -> Mon {
    let along_x = (button.0 + button.2 / 2 - w / 2).clamp(mon.0, (mon.0 + mon.2 - w).max(mon.0));
    let along_y = (button.1 + button.3 / 2 - h / 2).clamp(mon.1, (mon.1 + mon.3 - h).max(mon.1));
    match edge {
        Edge::Bottom => (along_x, strip.1 - h, w, h),
        Edge::Top => (along_x, strip.1 + strip.3, w, h),
        Edge::Left => (strip.0 + strip.2, along_y, w, h),
        Edge::Right => (strip.0 - w, along_y, w, h),
    }
}

// Its live pictures where its cards left room for them (window, x, y, w, h
// in its px); none, taken away.
#[tauri::command]
pub fn barpop_thumbs(items: Vec<(i64, f64, f64, f64, f64)>) {
    let Some((bar, rect)) = OPEN.lock().unwrap().as_ref().and_then(|p| p.rect.map(|r| (p.bar.clone(), r))) else {
        return taskbar::thumbs(taskbar::WATCH_BARS, Vec::new());
    };
    let Some((_, sf, _, _)) = bar_of(&bar) else { return };
    let px = |v: f64| (v * sf).round() as i32;
    let items = items.into_iter().map(|(h, x, y, w, ht)| (h as isize, (rect.0 + px(x), rect.1 + px(y), px(w), px(ht)))).collect();
    taskbar::thumbs(taskbar::WATCH_BARS, items);
}

// Its page done with it (the pointer gone, a card or an item pressed).
#[tauri::command]
pub fn barpop_hide(app: tauri::AppHandle) {
    hide_pop(&crate::shared(&app));
}

fn hide_pop(sh: &Shared) {
    let Some(was) = OPEN.lock().unwrap().take() else { return };
    taskbar::thumbs(taskbar::WATCH_BARS, Vec::new());
    taskbar::watch_presses(taskbar::WATCH_BARS, false);
    if let Some(win) = sh.app.get_webview_window(POP) {
        let _ = win.hide();
    }
    let _ = sh.app.emit_to(POP, "barpop:gone", json!({}));
    let _ = sh.app.emit_to(was.bar.as_str(), "bar:pop-gone", json!({}));
}

// A press anywhere (taskbar.rs press hook): off the list and off its strip,
// it goes, as Windows' own; on the strip, its page has the press.
pub fn pressed(sh: &Shared, (x, y): (i32, i32)) {
    let inside = |r: Mon| x >= r.0 && y >= r.1 && x < r.0 + r.2 && y < r.1 + r.3;
    let away = {
        let open = OPEN.lock().unwrap();
        let Some(p) = open.as_ref() else { return };
        let strip = bar_of(&p.bar).map(|b| b.3);
        !p.rect.is_some_and(inside) && !strip.is_some_and(inside)
    };
    if away {
        hide_pop(sh);
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
// dark and the accent, her island's look), her language; the prompts
// waiting and her sheet, for her capsule.
pub fn refresh(sh: &Shared) {
    let bars: Vec<(String, Mon, f64, Value)> = BARS.lock().unwrap().iter().filter(|b| b.ready).map(|b| (b.label.clone(), b.mon, b.sf, b.sent.clone())).collect();
    if bars.is_empty() {
        return;
    }
    let all = taskbar::windows_now();
    let look = taskbar::look_value();
    let spread = spread();
    let edge = island::edge(sh);
    let asks = waiting(&sh.asks.lock().unwrap().views());
    let pet = sh.setting("pet");
    let sprite = crate::data::pets(&sh.dir).into_iter().find(|p| Some(p.id.as_str()) == pet.as_str());
    for (label, mon, sf, sent) in bars {
        let state = json!({
            "edge": edge.word(),
            "align": sh.setting("taskbarAlign"),
            "material": sh.setting("taskbarMaterial"),
            "look": look,
            "islandLook": sh.setting("islandLook"),
            "lang": sh.lang(),
            "sf": sf,
            "windows": buttons_for(&all, mon, false, spread),
            "asks": asks,
            "sprite": sprite.as_ref().map(|p| crate::data::asset_url(&p.sheet)),
            "spriteVersion": sprite.as_ref().map_or(2, |p| p.version),
        });
        if state != sent {
            if let Some(b) = BARS.lock().unwrap().iter_mut().find(|b| b.label == label) {
                b.sent = state.clone();
            }
            let _ = sh.app.emit_to(label.as_str(), "bar:state", state);
        }
    }
    // A list open: its program's windows as they are now.
    if OPEN.lock().unwrap().is_some() {
        send_pop(sh);
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

    #[test]
    fn the_list_against_the_strip_over_its_button() {
        // A display right of the main one, 150% (its strip 72 px thick).
        let mon = (2560, 0, 1920, 1080);
        let bottom = (2560, 1008, 1920, 72);
        let button = (3400, 1014, 66, 60);
        // Along the bottom: on the strip, centred on the button.
        assert_eq!(pop_rect(Edge::Bottom, bottom, mon, button, (400, 300)), (3233, 708, 400, 300));
        // Near the display's ends: kept on it.
        assert_eq!(pop_rect(Edge::Bottom, bottom, mon, (2570, 1014, 66, 60), (400, 300)), (2560, 708, 400, 300));
        assert_eq!(pop_rect(Edge::Bottom, bottom, mon, (4400, 1014, 66, 60), (400, 300)), (4080, 708, 400, 300));
        // Along the top: under it.
        assert_eq!(pop_rect(Edge::Top, (2560, 0, 1920, 72), mon, (3400, 6, 66, 60), (400, 300)), (3233, 72, 400, 300));
        // Along a side: beside it, centred on the button up and down.
        assert_eq!(pop_rect(Edge::Left, (2560, 0, 72, 1080), mon, (2566, 500, 60, 66), (300, 200)), (2632, 433, 300, 200));
        assert_eq!(pop_rect(Edge::Right, (4408, 0, 72, 1080), mon, (4414, 500, 60, 66), (300, 200)), (4108, 433, 300, 200));
    }

    #[test]
    fn how_many_wait_and_for_what() {
        assert_eq!(waiting(&json!([])), Value::Null);
        assert_eq!(waiting(&json!([{ "kind": "permission" }, { "kind": "question" }])), json!({ "n": 2, "kind": "approve" }));
        assert_eq!(waiting(&json!([{ "kind": "plan" }])), json!({ "n": 1, "kind": "approve" }));
        assert_eq!(waiting(&json!([{ "kind": "question" }, { "kind": "permission" }])), json!({ "n": 2, "kind": "answer" }));
    }
}
