// The desktop's icons out from under her strip. Along the top, the left or
// the right her strip is an app bar of hers, and the work area is set
// without a word to the other windows (taskbar.rs set_work_area: told, Explorer
// set it back at once): so Explorer's desktop never heard of it, and laid its
// icons on the whole screen still, the Recycle Bin under her portrait, seen
// through a clear strip and not to be clicked (the user, 2026-10-13; since
// 0.2.5). Along the bottom Windows' own keeps its room, and the desktop knows.
//
// The desktop told, it alone (WM_SETTINGCHANGE, SPI_SETWORKAREA, to its own
// window: told to every window, Explorer's taskbar set the work area back),
// lays its icons in the work area as it does for Windows' own taskbar there,
// and back on the whole display once told again (tried, 2026-10-13): arranged
// by itself, arranged anew; kept to its grid, the grid moved with the work
// area; placed freely, each moved along with it (300 to 367 px beside a strip
// 60 px wide, back to 300). Back as they were, but one pressed to the top by
// a px or two.
//
// So, the strip taken or its work area set again (strip_taken): the icons'
// places kept first (icons.json in her data, once: those from before her,
// whatever runs and edges come between), the desktop told; seconds later,
// once it has laid them, any still under the strip moved out, and which
// kept. Just before the work area is given back (the bottom again, another
// home, quitting, the guard as she is gone: hold), where they are kept.
// Given back (given_back; a file left at start too): once the work area and
// the desktop keep still, the desktop having put most back itself, those
// placed by hand that are not put back where they were, or where the
// person left them (back_to).
//
// Uses only Windows and serde_json: examples/icons.rs builds it on its own.

use std::path::{Path, PathBuf};

#[cfg(windows)]
use serde_json::{json, Value};

// The file her icons' places are kept in, in her data folder.
pub fn file(data: &Path) -> PathBuf {
    data.join("icons.json")
}

#[cfg(windows)]
#[allow(dead_code)]
mod imp {
    use std::ffi::c_void;

    use serde_json::{json, Value};

    #[repr(C)]
    struct Guid(u32, u16, u16, [u8; 8]);

    #[repr(C)]
    pub struct Obj {
        vtbl: *const *const c_void,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default, Debug)]
    pub struct Rect {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }

    // VARIANT: its type, then 8 bytes on 32-bit Windows, 16 on 64-bit.
    #[repr(C)]
    struct Variant {
        vt: u16,
        reserved: [u16; 3],
        value: [usize; 2],
    }

    const CLSID_SHELL_WINDOWS: Guid = Guid(0x9BA05972, 0xF6A8, 0x11CF, [0xA4, 0x42, 0x00, 0xA0, 0xC9, 0x0A, 0x8F, 0x39]);
    const IID_ISHELL_WINDOWS: Guid = Guid(0x85CB6900, 0x4D95, 0x11CF, [0x96, 0x0C, 0x00, 0x80, 0xC7, 0xF4, 0xEE, 0x85]);
    const IID_ISERVICE_PROVIDER: Guid = Guid(0x6D5140C1, 0x7436, 0x11CE, [0x80, 0x34, 0x00, 0xAA, 0x00, 0x60, 0x09, 0xFA]);
    const SID_STOP_LEVEL_BROWSER: Guid = Guid(0x4C96BE40, 0x915C, 0x11CF, [0x99, 0xD3, 0x00, 0xAA, 0x00, 0x4A, 0xE8, 0x37]);
    const IID_ISHELL_BROWSER: Guid = Guid(0x000214E2, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
    const IID_IFOLDER_VIEW: Guid = Guid(0xCDE725B0, 0xCCC9, 0x4519, [0x91, 0x7E, 0x32, 0x5D, 0x72, 0xFA, 0xB4, 0xCE]);
    const IID_IFOLDER_VIEW2: Guid = Guid(0x1AF3A467, 0x214F, 0x4298, [0x90, 0x8E, 0x06, 0xB0, 0x3E, 0x0B, 0x39, 0xF9]);
    const IID_ISHELL_FOLDER: Guid = Guid(0x000214E6, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);
    const IID_IENUM_ID_LIST: Guid = Guid(0x000214F2, 0, 0, [0xC0, 0, 0, 0, 0, 0, 0, 0x46]);

    const CLSCTX_ALL: u32 = 0x17;
    const COINIT_APARTMENTTHREADED: u32 = 0x2;
    const VT_I4: u16 = 3;
    const CSIDL_DESKTOP: usize = 0;
    const SWC_DESKTOP: i32 = 8;
    const SWFO_NEEDDISPATCH: i32 = 1;
    const SVGIO_ALLVIEW: u32 = 2;
    const SVSI_POSITIONITEM: u32 = 0x80;
    const SHGDN_FORPARSING: u32 = 0x8000;
    const WM_SETTINGCHANGE: u32 = 0x001A;
    const SPI_GETWORKAREA: u32 = 0x30;
    const SPI_SETWORKAREA: u32 = 0x2F;
    const SMTO_ABORTIFHUNG: u32 = 0x2;

    // Methods by their place. IUnknown: 0-2; IShellWindows (after IDispatch's 3-6):
    const SW_FIND_WINDOW: usize = 15;
    // IServiceProvider:
    const SP_QUERY_SERVICE: usize = 3;
    // IOleWindow (IShellView): GetWindow; IShellBrowser:
    const OLE_GET_WINDOW: usize = 3;
    const SB_QUERY_ACTIVE_SHELL_VIEW: usize = 15;
    // IFolderView:
    const FV_GET_FOLDER: usize = 5;
    const FV_ITEMS: usize = 8;
    const FV_GET_ITEM_POSITION: usize = 11;
    const FV_GET_SPACING: usize = 12;
    const FV_GET_AUTO_ARRANGE: usize = 14;
    const FV_SELECT_AND_POSITION_ITEMS: usize = 16;
    // IFolderView2:
    const FV2_SET_CURRENT_FOLDER_FLAGS: usize = 24;
    const FV2_GET_CURRENT_FOLDER_FLAGS: usize = 25;
    // IEnumIDList:
    const ENUM_NEXT: usize = 3;
    // IShellFolder:
    const SF_GET_DISPLAY_NAME_OF: usize = 11;

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(reserved: *mut c_void, flags: u32) -> i32;
        fn CoUninitialize();
        fn CoCreateInstance(clsid: *const Guid, outer: *mut c_void, ctx: u32, iid: *const Guid, out: *mut *mut c_void) -> i32;
        fn CoTaskMemFree(p: *mut c_void);
    }

    #[link(name = "shlwapi")]
    extern "system" {
        fn StrRetToStrW(strret: *mut c_void, pidl: *const c_void, out: *mut *mut u16) -> i32;
    }

    // As the other modules declare them (one signature each across the crate).
    type Hwnd = *mut c_void;

    #[link(name = "user32")]
    extern "system" {
        fn FindWindowExW(parent: Hwnd, after: Hwnd, class: *const u16, title: *const u16) -> Hwnd;
        fn MapWindowPoints(from: Hwnd, to: Hwnd, points: *mut Point, n: u32) -> i32;
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, flags: u32) -> i32;
        fn SendMessageTimeoutW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize, flags: u32, ms: u32, result: *mut usize) -> isize;
        fn GetAncestor(hwnd: Hwnd, flags: u32) -> Hwnd;
    }

    // SAFETY (callers): the object is alive and has that method there.
    unsafe fn method<F: Copy>(obj: *mut Obj, at: usize) -> F {
        std::mem::transmute_copy(&*(*obj).vtbl.add(at))
    }

    unsafe fn release(obj: *mut Obj) {
        if !obj.is_null() {
            let f: extern "system" fn(*mut Obj) -> u32 = method(obj, 2);
            f(obj);
        }
    }

    unsafe fn query(obj: *mut Obj, iid: &Guid) -> Option<*mut Obj> {
        let mut out: *mut Obj = std::ptr::null_mut();
        let f: extern "system" fn(*mut Obj, *const Guid, *mut *mut Obj) -> i32 = method(obj, 0);
        (f(obj, iid, &mut out) >= 0 && !out.is_null()).then_some(out)
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    // COM on this thread for the call, left as found.
    pub fn with_com<T>(f: impl FnOnce() -> T) -> T {
        // SAFETY: balanced below when it took.
        let init = unsafe { CoInitializeEx(std::ptr::null_mut(), COINIT_APARTMENTTHREADED) };
        let out = f();
        if init >= 0 {
            // SAFETY: the initialization above undone.
            unsafe { CoUninitialize() };
        }
        out
    }

    pub fn work_area() -> Rect {
        let mut r = Rect::default();
        // SAFETY: our own struct, of the size the call writes.
        unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut Rect as *mut c_void, 0) };
        r
    }

    // An icon on the desktop: its item (owned), its parsing name (a file's
    // path, the Recycle Bin's ::{GUID}), where it is (the screen's px).
    pub struct Item {
        pidl: *mut c_void,
        pub name: String,
        pub at: Point,
    }

    impl Drop for Item {
        fn drop(&mut self) {
            // SAFETY: the item Windows handed over, freed once.
            unsafe { CoTaskMemFree(self.pidl) };
        }
    }

    // The desktop's view, as Explorer shows it.
    pub struct Desktop {
        view: *mut Obj,
        folder: *mut Obj,
        // Its window (SHELLDLL_DefView), its list (the icons' coordinates),
        // and the top window holding them (Progman, or a WorkerW).
        pub defview: isize,
        pub list: isize,
        pub top: isize,
    }

    impl Drop for Desktop {
        fn drop(&mut self) {
            // SAFETY: ours, released once.
            unsafe {
                release(self.view);
                release(self.folder);
            }
        }
    }

    // The desktop's view (Raymond Chen's way: the shell's windows, the
    // desktop's, its browser, its view). None with no Explorer.
    pub fn desktop() -> Option<Desktop> {
        // SAFETY: each object released once; out-parameters our own.
        unsafe {
            let mut windows: *mut Obj = std::ptr::null_mut();
            if CoCreateInstance(&CLSID_SHELL_WINDOWS, std::ptr::null_mut(), CLSCTX_ALL, &IID_ISHELL_WINDOWS, &mut windows as *mut *mut Obj as *mut *mut c_void) < 0 || windows.is_null() {
                return None;
            }
            let at = Variant { vt: VT_I4, reserved: [0; 3], value: [CSIDL_DESKTOP, 0] };
            let root = Variant { vt: 0, reserved: [0; 3], value: [0, 0] };
            let mut hwnd = 0i32;
            let mut dispatch: *mut Obj = std::ptr::null_mut();
            let find: extern "system" fn(*mut Obj, *const Variant, *const Variant, i32, *mut i32, i32, *mut *mut Obj) -> i32 = method(windows, SW_FIND_WINDOW);
            let found = find(windows, &at, &root, SWC_DESKTOP, &mut hwnd, SWFO_NEEDDISPATCH, &mut dispatch);
            release(windows);
            if found != 0 || dispatch.is_null() {
                release(dispatch);
                return None;
            }
            let provider = query(dispatch, &IID_ISERVICE_PROVIDER);
            release(dispatch);
            let provider = provider?;
            let mut browser: *mut Obj = std::ptr::null_mut();
            let service: extern "system" fn(*mut Obj, *const Guid, *const Guid, *mut *mut Obj) -> i32 = method(provider, SP_QUERY_SERVICE);
            let got = service(provider, &SID_STOP_LEVEL_BROWSER, &IID_ISHELL_BROWSER, &mut browser);
            release(provider);
            if got < 0 || browser.is_null() {
                return None;
            }
            let mut shell_view: *mut Obj = std::ptr::null_mut();
            let active: extern "system" fn(*mut Obj, *mut *mut Obj) -> i32 = method(browser, SB_QUERY_ACTIVE_SHELL_VIEW);
            let got = active(browser, &mut shell_view);
            release(browser);
            if got < 0 || shell_view.is_null() {
                return None;
            }
            let mut defview = 0isize;
            let window: extern "system" fn(*mut Obj, *mut isize) -> i32 = method(shell_view, OLE_GET_WINDOW);
            window(shell_view, &mut defview);
            let view = query(shell_view, &IID_IFOLDER_VIEW);
            release(shell_view);
            let view = view?;
            let mut folder: *mut Obj = std::ptr::null_mut();
            let get: extern "system" fn(*mut Obj, *const Guid, *mut *mut Obj) -> i32 = method(view, FV_GET_FOLDER);
            if get(view, &IID_ISHELL_FOLDER, &mut folder) < 0 {
                folder = std::ptr::null_mut();
            }
            let class = wide("SysListView32");
            let list = FindWindowExW(defview as Hwnd, std::ptr::null_mut(), class.as_ptr(), std::ptr::null()) as isize;
            const GA_ROOT: u32 = 2;
            let top = GetAncestor(defview as Hwnd, GA_ROOT) as isize;
            Some(Desktop { view, folder, defview, list: if list != 0 { list } else { defview }, top })
        }
    }

    impl Desktop {
        // Whether it arranges its icons itself.
        pub fn auto_arrange(&self) -> bool {
            // SAFETY: the view alive; no arguments.
            unsafe {
                let f: extern "system" fn(*mut Obj) -> i32 = method(self.view, FV_GET_AUTO_ARRANGE);
                f(self.view) == 0
            }
        }

        // Its folder flags (FWF_*), as Explorer keeps them.
        pub fn flags(&self) -> Option<u32> {
            // SAFETY: the view alive; released once; our own out-parameter.
            unsafe {
                let v2 = query(self.view, &IID_IFOLDER_VIEW2)?;
                let mut f = 0u32;
                let get: extern "system" fn(*mut Obj, *mut u32) -> i32 = method(v2, FV2_GET_CURRENT_FOLDER_FLAGS);
                let ok = get(v2, &mut f) >= 0;
                release(v2);
                ok.then_some(f)
            }
        }

        // Folder flags turned on or off (examples/icons.rs, to try: 1 arranging
        // its icons itself, 4 keeping them to the grid).
        pub fn set_flag(&self, flag: u32, on: bool) -> bool {
            // SAFETY: the view alive; released once.
            unsafe {
                let Some(v2) = query(self.view, &IID_IFOLDER_VIEW2) else { return false };
                let set: extern "system" fn(*mut Obj, u32, u32) -> i32 = method(v2, FV2_SET_CURRENT_FOLDER_FLAGS);
                let ok = set(v2, flag, if on { flag } else { 0 }) >= 0;
                release(v2);
                ok
            }
        }

        // One icon's place on the grid, across and down (px).
        pub fn spacing(&self) -> Point {
            let mut p = Point::default();
            // SAFETY: the view alive; our own out-parameter.
            unsafe {
                let f: extern "system" fn(*mut Obj, *mut Point) -> i32 = method(self.view, FV_GET_SPACING);
                f(self.view, &mut p);
            }
            p
        }

        // The list's coordinates to the screen's: where its (0, 0) is.
        pub fn origin(&self) -> Point {
            let mut p = Point::default();
            // SAFETY: one point of our own.
            unsafe { MapWindowPoints(self.list as Hwnd, std::ptr::null_mut(), &mut p, 1) };
            p
        }

        // Its icons, where they are (the screen's px).
        pub fn items(&self) -> Vec<Item> {
            let mut out = Vec::new();
            let o = self.origin();
            // SAFETY: the view and its list alive; each item ours to free (Item).
            unsafe {
                let mut list: *mut Obj = std::ptr::null_mut();
                let items: extern "system" fn(*mut Obj, u32, *const Guid, *mut *mut Obj) -> i32 = method(self.view, FV_ITEMS);
                if items(self.view, SVGIO_ALLVIEW, &IID_IENUM_ID_LIST, &mut list) < 0 || list.is_null() {
                    return out;
                }
                let next: extern "system" fn(*mut Obj, u32, *mut *mut c_void, *mut u32) -> i32 = method(list, ENUM_NEXT);
                let position: extern "system" fn(*mut Obj, *const c_void, *mut Point) -> i32 = method(self.view, FV_GET_ITEM_POSITION);
                loop {
                    let mut pidl: *mut c_void = std::ptr::null_mut();
                    let mut got = 0u32;
                    if next(list, 1, &mut pidl, &mut got) != 0 || got == 0 || pidl.is_null() {
                        break;
                    }
                    let mut at = Point::default();
                    position(self.view, pidl, &mut at);
                    let name = self.name_of(pidl);
                    out.push(Item { pidl, name, at: Point { x: at.x + o.x, y: at.y + o.y } });
                }
                release(list);
            }
            out
        }

        unsafe fn name_of(&self, pidl: *const c_void) -> String {
            if self.folder.is_null() {
                return String::new();
            }
            // STRRET: its kind, then a pointer or 260 bytes.
            let mut ret = [0u64; 34];
            let f: extern "system" fn(*mut Obj, *const c_void, u32, *mut c_void) -> i32 = method(self.folder, SF_GET_DISPLAY_NAME_OF);
            if f(self.folder, pidl, SHGDN_FORPARSING, ret.as_mut_ptr() as *mut c_void) < 0 {
                return String::new();
            }
            let mut text: *mut u16 = std::ptr::null_mut();
            if StrRetToStrW(ret.as_mut_ptr() as *mut c_void, pidl, &mut text) < 0 || text.is_null() {
                return String::new();
            }
            let n = (0..).take_while(|&i| *text.add(i) != 0).count();
            let s = String::from_utf16_lossy(std::slice::from_raw_parts(text, n));
            CoTaskMemFree(text as *mut c_void);
            s
        }

        // Icons to places (the screen's px).
        pub fn place(&self, moves: &[(&Item, Point)]) -> bool {
            if moves.is_empty() {
                return true;
            }
            let o = self.origin();
            let pidls: Vec<*const c_void> = moves.iter().map(|(item, _)| item.pidl as *const c_void).collect();
            let points: Vec<Point> = moves.iter().map(|(_, p)| Point { x: p.x - o.x, y: p.y - o.y }).collect();
            // SAFETY: the view alive; the arrays as long as said.
            unsafe {
                let f: extern "system" fn(*mut Obj, u32, *const *const c_void, *const Point, u32) -> i32 = method(self.view, FV_SELECT_AND_POSITION_ITEMS);
                f(self.view, pidls.len() as u32, pidls.as_ptr(), points.as_ptr(), SVSI_POSITIONITEM) >= 0
            }
        }

        // The desktop told the work area changed, it alone (told to every
        // window, Explorer's taskbar set it back: taskbar.rs set_work_area).
        pub fn tell(&self, to: isize) -> bool {
            let mut done = 0usize;
            // SAFETY: a message with no pointers, waited for a second at most.
            unsafe { SendMessageTimeoutW(to as Hwnd, WM_SETTINGCHANGE, SPI_SETWORKAREA as usize, 0, SMTO_ABORTIFHUNG, 1000, &mut done) != 0 }
        }
    }

    // What there is, for the log and examples/icons.rs.
    pub fn describe() -> Value {
        with_com(|| {
            let Some(d) = desktop() else { return json!({ "desktop": null }) };
            let w = work_area();
            let items: Vec<Value> = d.items().iter().map(|i| json!({ "name": i.name, "x": i.at.x, "y": i.at.y })).collect();
            let s = d.spacing();
            let o = d.origin();
            json!({
                "auto": d.auto_arrange(),
                "flags": d.flags().map(|f| format!("{f:#010x}")),
                "spacing": [s.x, s.y],
                "origin": [o.x, o.y],
                "windows": { "defview": d.defview, "list": d.list, "top": d.top },
                "work": [w.left, w.top, w.right, w.bottom],
                "items": items,
            })
        })
    }
}

// describe, Rect and the folder flags: for examples/icons.rs.
#[cfg(windows)]
#[allow(unused_imports)]
pub use imp::{describe, desktop, with_com, Item, Point, Rect};

// One at a time, and each but a moment: quitting waits on it (hold, given_back).
#[cfg(windows)]
static BUSY: std::sync::Mutex<()> = std::sync::Mutex::new(());

// Bumped as the strip is given back (hold, given_back): a later look at the
// icons for a strip taken before is not taken any more.
#[cfg(windows)]
static ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[cfg(windows)]
use std::time::{Duration, Instant};

// Whether an icon's place on the grid (its top-left, a place across and
// down) reaches under the strip (x, y, w, h).
#[cfg(windows)]
fn under(at: Point, spacing: Point, strip: (i32, i32, i32, i32)) -> bool {
    at.x < strip.0 + strip.2 && at.x + spacing.x > strip.0 && at.y < strip.1 + strip.3 && at.y + spacing.y > strip.1
}

// Where an icon under the strip goes: out of it on its inner side, as far
// as it reaches in.
#[cfg(windows)]
fn out_of(at: Point, spacing: Point, strip: (i32, i32, i32, i32), edge: &str) -> Point {
    match edge {
        "left" => Point { x: strip.0 + strip.2, y: at.y },
        "right" => Point { x: strip.0 - spacing.x, y: at.y },
        "top" => Point { x: at.x, y: strip.1 + strip.3 },
        _ => Point { x: at.x, y: strip.1 - spacing.y },
    }
}

#[cfg(windows)]
fn places(items: &[Item]) -> Value {
    Value::Array(items.iter().map(|i| json!({ "name": i.name, "x": i.at.x, "y": i.at.y })).collect())
}

// One icon's place in a list kept (places), by its name.
#[cfg(windows)]
fn place_of(list: &Value, name: &str) -> Option<Point> {
    let p = list.as_array()?.iter().find(|p| p["name"].as_str() == Some(name))?;
    Some(Point { x: p["x"].as_i64()? as i32, y: p["y"].as_i64()? as i32 })
}

#[cfg(windows)]
fn kept(data: &Path) -> Option<Value> {
    std::fs::read_to_string(file(data)).ok().and_then(|t| serde_json::from_str(&t).ok())
}

#[cfg(windows)]
fn keep(data: &Path, saved: &Value) -> std::io::Result<()> {
    std::fs::write(file(data), saved.to_string())
}

// The icons once the desktop has done laying them: read every 200 ms until
// they have kept still for 1.2 s, so long at most.
#[cfg(windows)]
fn settled(d: &imp::Desktop, most: Duration) -> Vec<Item> {
    let t = Instant::now();
    let mut last = d.items();
    let mut since = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(200));
        let now = d.items();
        let same = now.len() == last.len() && now.iter().zip(&last).all(|(a, b)| a.name == b.name && a.at == b.at);
        if !same {
            since = Instant::now();
        }
        if (same && since.elapsed() >= Duration::from_millis(1200)) || t.elapsed() >= most {
            return now;
        }
        last = now;
    }
}

// The work area once it keeps still (the same for 600 ms), so long at most.
#[cfg(windows)]
fn work_settled(most: Duration) {
    let t = Instant::now();
    let key = || {
        let r = imp::work_area();
        (r.left, r.top, r.right, r.bottom)
    };
    let (mut last, mut since) = (key(), Instant::now());
    while t.elapsed() < most && since.elapsed() < Duration::from_millis(600) {
        std::thread::sleep(Duration::from_millis(100));
        let now = key();
        if now != last {
            (last, since) = (now, Instant::now());
        }
    }
}

// Her strip along an edge (top, left, right; bottom when its work area was
// set again), the work area set: the places kept (once: those from before
// her), and the desktop told. It lays its icons for it only some seconds
// later, once Windows' own taskbar is away (the test session's traces,
// 2026-10-13): looked at then (3 s, and 8 s, after), any still under the
// strip moved out to its inner side, and which kept (moved: from, to),
// to put back. Lines for the log, now and then (`log`).
#[cfg(windows)]
pub fn strip_taken(data: &Path, strip: (i32, i32, i32, i32), edge: &str, log: impl Fn(String) + Send + 'static) {
    let round = ROUND.load(std::sync::atomic::Ordering::SeqCst);
    {
        let _one = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        let line = with_com(|| {
            let Some(d) = desktop() else { return "icons: no desktop to tell".to_string() };
            let kept_now = kept(data).is_some();
            if !kept_now {
                let items = d.items();
                let saved = json!({ "auto": d.auto_arrange(), "items": places(&items), "moved": [] });
                if let Err(e) = keep(data, &saved) {
                    return format!("icons: places not written ({e})");
                }
            }
            format!(
                "icons: the desktop told of the work area along the {edge} ({}){}",
                if d.tell(d.top) { "answered" } else { "no answer" },
                if kept_now { "" } else { "; places kept" }
            )
        });
        log(line);
    }
    let (data, edge) = (data.to_path_buf(), edge.to_string());
    std::thread::spawn(move || {
        let began = Instant::now();
        for at in [Duration::from_secs(3), Duration::from_secs(8)] {
            std::thread::sleep(at.saturating_sub(began.elapsed()));
            if ROUND.load(std::sync::atomic::Ordering::SeqCst) != round {
                return;
            }
            let _one = BUSY.lock().unwrap_or_else(|e| e.into_inner());
            let Some(mut saved) = kept(&data) else { return };
            let line = with_com(|| {
                let d = desktop()?;
                let spacing = d.spacing();
                let items = d.items();
                let out: Vec<(&Item, Point)> = items.iter().filter(|i| under(i.at, spacing, strip)).map(|i| (i, out_of(i.at, spacing, strip, &edge))).collect();
                if out.is_empty() {
                    return None;
                }
                let put = d.place(&out);
                if let Some(moved) = saved["moved"].as_array_mut() {
                    for (i, to) in &out {
                        // Where it was moved to (the last time).
                        if !moved.iter().any(|m| m["name"].as_str() == Some(i.name.as_str())) {
                            moved.push(json!({ "name": i.name, "x": to.x, "y": to.y }));
                        } else if let Some(m) = moved.iter_mut().find(|m| m["name"].as_str() == Some(i.name.as_str())) {
                            m["x"] = json!(to.x);
                            m["y"] = json!(to.y);
                        }
                    }
                }
                let _ = keep(&data, &saved);
                Some(format!("icons: {} still under the strip {} s after, moved out: {put}", out.len(), at.as_secs()))
            });
            if let Some(line) = line {
                log(line);
            }
        }
    });
}

// Just before the work area is given back (the bottom again, another home,
// quitting, the guard as she is gone): where the icons are now, kept
// (now), before the desktop moves them for the whole display, so one the
// person moved meanwhile is put back where they left it.
#[cfg(windows)]
pub fn hold(data: &Path) -> Vec<String> {
    ROUND.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let _one = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    let Some(mut saved) = kept(data) else { return Vec::new() };
    with_com(|| {
        let Some(d) = desktop() else { return vec!["icons: no desktop to look at before giving back".to_string()] };
        let items = d.items();
        saved["now"] = places(&items);
        let w = imp::work_area();
        match keep(data, &saved) {
            Ok(()) => vec![format!("icons: {} places held before giving back (work area ({}, {}, {}, {}))", items.len(), w.left, w.top, w.right, w.bottom)],
            Err(e) => vec![format!("icons: places held not written ({e})")],
        }
    })
}

// What an icon the desktop does not arrange itself is put back to, given
// back, from where the desktop laid it (at): none when it is where it was
// before her (within half a place on the grid: the desktop puts them back
// itself, and moved there again it came out a place off); moved out from
// under the strip by her and untouched since (now where moved), where it
// was; else, held (now), where the person left it; nothing held, left be.
#[cfg(windows)]
fn back_to(name: &str, at: Point, saved: &Value, spacing: Point) -> Option<Point> {
    let near = |a: Point, b: Point| (a.x - b.x).abs() * 2 < spacing.x && (a.y - b.y).abs() * 2 < spacing.y;
    let was = place_of(&saved["items"], name);
    if was.is_some_and(|w| near(at, w)) {
        return None;
    }
    let now = place_of(&saved["now"], name);
    let moved = place_of(&saved["moved"], name);
    let to = match (now, moved) {
        (Some(n), Some(m)) if near(n, m) => was,
        (Some(n), _) => Some(n),
        (None, Some(_)) => was,
        (None, None) => None,
    };
    to.filter(|t| *t != at)
}

// The work area the display's again (her strip at the bottom, given back,
// gone): once it keeps still, the desktop told, and once it is done laying
// them, those it does not arrange itself and did not put back as they were
// put back (back_to); looked at again, and once more off by as much the
// other way if they came out elsewhere. The places forgotten then. Nothing
// when none were kept. Lines for the log.
#[cfg(windows)]
pub fn given_back(data: &Path) -> Vec<String> {
    ROUND.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let _one = BUSY.lock().unwrap_or_else(|e| e.into_inner());
    let Some(saved) = kept(data) else { return Vec::new() };
    work_settled(Duration::from_secs(5));
    with_com(|| {
        let Some(d) = desktop() else { return vec!["icons: no desktop to tell; the places kept for later".to_string()] };
        let told = d.tell(d.top);
        let items = settled(&d, Duration::from_secs(5));
        let auto = d.auto_arrange();
        let spacing = d.spacing();
        let wanted: Vec<(String, Point, Point)> = if auto { Vec::new() } else { items.iter().filter_map(|i| back_to(&i.name, i.at, &saved, spacing).map(|t| (i.name.clone(), i.at, t))).collect() };
        let mut rounds = Vec::new();
        let mut put = true;
        let mut ask: Vec<(String, Point)> = wanted.iter().map(|(name, _, t)| (name.clone(), *t)).collect();
        for _ in 0..2 {
            if ask.is_empty() {
                break;
            }
            let now = d.items();
            let moves: Vec<(&Item, Point)> = ask.iter().filter_map(|(name, p)| now.iter().find(|i| &i.name == name).map(|i| (i, *p))).collect();
            put &= d.place(&moves);
            let after = settled(&d, Duration::from_secs(2));
            let mut off = Vec::new();
            for ((name, _, want), (_, asked)) in wanted.iter().zip(ask.iter_mut()) {
                if let Some(i) = after.iter().find(|i| &i.name == name) {
                    let (dx, dy) = (i.at.x - want.x, i.at.y - want.y);
                    if (dx, dy) != (0, 0) {
                        off.push(format!("{dx:+},{dy:+}"));
                        *asked = Point { x: asked.x - dx, y: asked.y - dy };
                    }
                }
            }
            rounds.push(if off.is_empty() { "there".to_string() } else { format!("off {}", off.join(" ")) });
            if off.is_empty() {
                break;
            }
        }
        let found: Vec<String> = wanted.iter().map(|(_, at, t)| format!("{},{}→{},{}", at.x, at.y, t.x, t.y)).collect();
        let w = imp::work_area();
        let _ = std::fs::remove_file(file(data));
        vec![format!(
            "icons: the desktop told the work area is whole again ({}; work area ({}, {}, {}, {})): arranged by itself: {auto}, {} of {} put back{}{}{}",
            if told { "answered" } else { "no answer" },
            w.left,
            w.top,
            w.right,
            w.bottom,
            wanted.len(),
            items.len(),
            if found.is_empty() { String::new() } else { format!(" ({})", found.join(" ")) },
            if put { "" } else { " (refused)" },
            if rounds.is_empty() { String::new() } else { format!(": {}", rounds.join(", then ")) }
        )]
    })
}

// hold and given_back, waited for so long at most (quitting, starting,
// a thread that must go on): lines for the log, or one saying it was not
// waited for. Nothing when no places were kept. (Not for examples/icons.rs.)
#[allow(dead_code)]
fn within(data: &Path, most: std::time::Duration, f: fn(&Path) -> Vec<String>) -> Vec<String> {
    if !file(data).exists() {
        return Vec::new();
    }
    let data = data.to_path_buf();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(f(&data));
    });
    rx.recv_timeout(most).unwrap_or_else(|_| vec![format!("icons: not done within {} ms", most.as_millis())])
}

#[allow(dead_code)]
pub fn given_back_within(data: &Path, most: std::time::Duration) -> Vec<String> {
    within(data, most, given_back)
}

#[allow(dead_code)]
pub fn hold_within(data: &Path, most: std::time::Duration) -> Vec<String> {
    within(data, most, hold)
}

#[cfg(not(windows))]
pub fn strip_taken(_data: &Path, _strip: (i32, i32, i32, i32), _edge: &str, _log: impl Fn(String) + Send + 'static) {}

#[cfg(not(windows))]
pub fn hold(_data: &Path) -> Vec<String> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn given_back(_data: &Path) -> Vec<String> {
    Vec::new()
}

#[cfg(not(windows))]
pub fn describe() -> serde_json::Value {
    serde_json::json!({ "desktop": null })
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    #[test]
    fn put_back_where_it_was_or_where_the_person_left_it() {
        let grid = Point { x: 93, y: 122 };
        let p = |x, y| Point { x, y };
        let saved = json!({
            "items": [{ "name": "bin", "x": 16, "y": 2 }, { "name": "app", "x": 16, "y": 124 }, { "name": "doc", "x": 300, "y": 400 }],
            "moved": [{ "name": "bin", "x": 60, "y": 2 }],
            "now": [{ "name": "bin", "x": 60, "y": 2 }, { "name": "app", "x": 77, "y": 129 }, { "name": "doc", "x": 900, "y": 600 }, { "name": "new", "x": 500, "y": 500 }],
        });
        // Put back by the desktop itself: left be.
        assert_eq!(back_to("app", p(16, 124), &saved, grid), None);
        assert_eq!(back_to("app", p(20, 120), &saved, grid), None);
        // Moved out by her, untouched since: back where it was.
        assert_eq!(back_to("bin", p(109, 2), &saved, grid), Some(p(16, 2)));
        // Dragged by the person meanwhile: where they left it.
        assert_eq!(back_to("doc", p(822, 576), &saved, grid), Some(p(900, 600)));
        assert_eq!(back_to("doc", p(900, 600), &saved, grid), None);
        // One that came while she was there: where it was left.
        assert_eq!(back_to("new", p(470, 480), &saved, grid), Some(p(500, 500)));
        // Nothing held: left be.
        let bare = json!({ "items": [{ "name": "bin", "x": 16, "y": 2 }] });
        assert_eq!(back_to("bin", p(109, 2), &bare, grid), None);
    }

    #[test]
    fn under_the_strip_and_out_of_it() {
        let grid = Point { x: 93, y: 122 };
        let left = (0, 0, 60, 1440);
        // The Recycle Bin at the top-left: under a left strip, moved to its side.
        assert!(under(Point { x: 0, y: 5 }, grid, left));
        assert_eq!(out_of(Point { x: 16, y: 2 }, grid, left, "left"), Point { x: 60, y: 2 });
        // One a place further: clear of it.
        assert!(!under(Point { x: 93, y: 5 }, grid, left));
        let right = (2500, 0, 60, 1440);
        assert!(under(Point { x: 2460, y: 300 }, grid, right));
        assert_eq!(out_of(Point { x: 2460, y: 300 }, grid, right, "right"), Point { x: 2407, y: 300 });
        let top = (0, 0, 2560, 60);
        assert!(under(Point { x: 400, y: 0 }, grid, top));
        assert_eq!(out_of(Point { x: 400, y: 0 }, grid, top, "top"), Point { x: 400, y: 60 });
    }
}

