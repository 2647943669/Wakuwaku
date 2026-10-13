// A bar's room on the screen: registered with Windows as an app bar, as the
// taskbar is, so the work area loses its height and maximized windows stop
// at it. The taskbar (display 'taskbar') takes a strip along an edge of the
// screen (taskbarEdge: the bottom, as Windows' own, or the top, the left,
// the right), Windows' own being put away (shell.rs, which reads and sets
// its state here). Given back when the bar goes (another home, out of
// sight, quitting). Windows only.

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    // The setting's word (taskbarEdge); anything else is the bottom.
    pub fn of(word: &str) -> Edge {
        match word {
            "top" => Edge::Top,
            "left" => Edge::Left,
            "right" => Edge::Right,
            _ => Edge::Bottom,
        }
    }

    // The setting's word for it.
    pub fn word(self) -> &'static str {
        match self {
            Edge::Top => "top",
            Edge::Bottom => "bottom",
            Edge::Left => "left",
            Edge::Right => "right",
        }
    }

    // Along the left or right of the screen: upright.
    pub fn is_side(self) -> bool {
        matches!(self, Edge::Left | Edge::Right)
    }

    // The strip this thick along this edge of a rect (x, y, w, h).
    pub fn strip(self, mon: (i32, i32, i32, i32), thickness: i32) -> (i32, i32, i32, i32) {
        match self {
            Edge::Top => (mon.0, mon.1, mon.2, thickness),
            Edge::Bottom => (mon.0, mon.1 + mon.3 - thickness, mon.2, thickness),
            Edge::Left => (mon.0, mon.1, thickness, mon.3),
            Edge::Right => (mon.0 + mon.2 - thickness, mon.1, thickness, mon.3),
        }
    }

    // A strip's inner side along this edge: where the work area stops.
    pub fn inner(self, strip: (i32, i32, i32, i32)) -> i32 {
        match self {
            Edge::Top => strip.1 + strip.3,
            Edge::Bottom => strip.1,
            Edge::Left => strip.0 + strip.2,
            Edge::Right => strip.0,
        }
    }

    // A rect's side along this edge (x, y, w, h).
    pub fn side_of(self, r: (i32, i32, i32, i32)) -> i32 {
        match self {
            Edge::Top => r.1,
            Edge::Bottom => r.1 + r.3,
            Edge::Left => r.0,
            Edge::Right => r.0 + r.2,
        }
    }

    // Whether a work area reaches into a strip along this edge.
    pub fn reaches(self, work: (i32, i32, i32, i32), strip: (i32, i32, i32, i32)) -> bool {
        let (side, inner) = (self.side_of(work), self.inner(strip));
        match self {
            Edge::Top | Edge::Left => side < inner,
            Edge::Bottom | Edge::Right => side > inner,
        }
    }

    // The work area's sides other than this edge's (and the bottom, the
    // room Windows' own keeps: taskbar.rs band_left) held in from the
    // display by as much as the strip is thick: a strip of hers along that
    // edge before, which Explorer, counting app bars again, kept (from the
    // right to the left, her taskbar on the main display alone, the right's
    // 60 px stayed: the test session, 2026-10-13). Each with the display's
    // side to give back. None along the bottom.
    pub fn stale_sides(self, work: (i32, i32, i32, i32), strip: (i32, i32, i32, i32), mon: (i32, i32, i32, i32)) -> Vec<(Edge, i32)> {
        if self == Edge::Bottom {
            return Vec::new();
        }
        let thick = if self.is_side() { strip.2 } else { strip.3 };
        [Edge::Top, Edge::Left, Edge::Right]
            .into_iter()
            .filter(|&e| e != self && (e.side_of(work) - e.side_of(mon)).abs() == thick)
            .map(|e| (e, e.side_of(mon)))
            .collect()
    }

    // A rect with its side along this edge moved to `side`.
    pub fn with_side(self, r: (i32, i32, i32, i32), side: i32) -> (i32, i32, i32, i32) {
        let (l, t, rt, b) = (r.0, r.1, r.0 + r.2, r.1 + r.3);
        let (l, t, rt, b) = match self {
            Edge::Top => (l, side, rt, b),
            Edge::Bottom => (l, t, rt, side),
            Edge::Left => (side, t, rt, b),
            Edge::Right => (l, t, side, b),
        };
        (l, t, rt - l, b - t)
    }
}

#[cfg(windows)]
mod imp {
    use super::Edge;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    #[repr(C)]
    pub struct AppBarData {
        size: u32,
        hwnd: isize,
        callback: u32,
        edge: u32,
        rc: Rect,
        lparam: isize,
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHAppBarMessage(message: u32, data: *mut AppBarData) -> usize;
    }

    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromPoint(pt: Point, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
        fn SystemParametersInfoW(action: u32, param: u32, pv: *mut c_void, flags: u32) -> i32;
    }

    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    const SPI_SETWORKAREA: u32 = 0x2F;

    // A display's work area (x, y, w, h, physical), the display by its rect.
    pub fn work_of(mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
        let at = Point { x: mon.0 + mon.2 / 2, y: mon.1 + mon.3 / 2 };
        // SAFETY: our own struct, of the size the call writes.
        let ok = unsafe { GetMonitorInfoW(MonitorFromPoint(at, MONITOR_DEFAULTTONEAREST), &mut info) } != 0;
        let w = &info.work;
        ok.then_some((w.left, w.top, w.right - w.left, w.bottom - w.top))
    }

    // The work area of the display a rect lies on set to it, quietly, as
    // the main strip's is (taskbar.rs set_work_area: told to every window,
    // Explorer set it back at once).
    pub fn set_work(r: (i32, i32, i32, i32)) -> bool {
        let mut rect = Rect { left: r.0, top: r.1, right: r.0 + r.2, bottom: r.1 + r.3 };
        // SAFETY: our own struct, handed back.
        unsafe { SystemParametersInfoW(SPI_SETWORKAREA, 0, &mut rect as *mut Rect as *mut c_void, 0) != 0 }
    }

    const ABM_NEW: u32 = 0;
    const ABM_REMOVE: u32 = 1;
    const ABM_QUERYPOS: u32 = 2;
    const ABM_SETPOS: u32 = 3;
    const ABM_GETSTATE: u32 = 4;
    const ABM_SETSTATE: u32 = 10;
    const ABE_LEFT: u32 = 0;
    const ABE_TOP: u32 = 1;
    const ABE_RIGHT: u32 = 2;
    const ABE_BOTTOM: u32 = 3;
    // What Windows sends a bar about changes, as the message's wParam (the top
    // bar does not listen: it asks again whenever the screens change, see
    // island.rs). For ABN_FULLSCREENAPP, lParam is 1 when a full-screen
    // window opens on the bar's display and 0 when the last one goes.
    pub const CALLBACK: u32 = 0x8000 + 0x57;
    #[allow(dead_code)]
    pub const ABN_STATECHANGE: usize = 0;
    #[allow(dead_code)]
    pub const ABN_POSCHANGED: usize = 1;
    #[allow(dead_code)]
    pub const ABN_FULLSCREENAPP: usize = 2;
    // The taskbar's state: hiding itself (ABS_AUTOHIDE), and always on top.
    pub const ABS_AUTOHIDE: u32 = 1;

    fn data(hwnd: isize, edge: Edge) -> AppBarData {
        let edge = match edge {
            Edge::Top => ABE_TOP,
            Edge::Bottom => ABE_BOTTOM,
            Edge::Left => ABE_LEFT,
            Edge::Right => ABE_RIGHT,
        };
        AppBarData { size: std::mem::size_of::<AppBarData>() as u32, hwnd, callback: CALLBACK, edge, rc: Rect::default(), lparam: 0 }
    }

    pub fn register(hwnd: isize) -> bool {
        let mut d = data(hwnd, Edge::Top);
        // SAFETY: a struct of the size the call asks for, about our own window.
        unsafe { SHAppBarMessage(ABM_NEW, &mut d) != 0 }
    }

    pub fn remove(hwnd: isize) {
        let mut d = data(hwnd, Edge::Top);
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_REMOVE, &mut d) };
    }

    // Where a strip this thick along an edge of a display (x, y, w, h) would
    // go, as Windows would grant it now (another bar may be there first).
    fn ask(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> AppBarData {
        let mut d = data(hwnd, edge);
        let (x, y, w, h) = edge.strip(mon, thickness);
        d.rc = Rect { left: x, top: y, right: x + w, bottom: y + h };
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_QUERYPOS, &mut d) };
        // The edge may have moved in past another bar: the thickness stays
        // ours (a side bar's length may be shortened by one along the top or
        // the bottom).
        match edge {
            Edge::Top => d.rc.bottom = d.rc.top + thickness,
            Edge::Bottom => d.rc.top = d.rc.bottom - thickness,
            Edge::Left => d.rc.right = d.rc.left + thickness,
            Edge::Right => d.rc.left = d.rc.right - thickness,
        }
        d
    }

    fn rect(d: &AppBarData) -> (i32, i32, i32, i32) {
        (d.rc.left, d.rc.top, d.rc.right - d.rc.left, d.rc.bottom - d.rc.top)
    }

    // Asked only, nothing taken: x, y, w, h. When another bar moves
    // (ABN_POSCHANGED), taking the strip again only if this changed keeps two
    // bars from waking each other, back and forth.
    #[allow(dead_code)]
    pub fn query(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        rect(&ask(hwnd, mon, edge, thickness))
    }

    // The strip taken, as Windows grants it: x, y, w, h.
    pub fn place(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        let mut d = ask(hwnd, mon, edge, thickness);
        // SAFETY: as above.
        unsafe { SHAppBarMessage(ABM_SETPOS, &mut d) };
        rect(&d)
    }

    // Windows' own taskbar's state (ABS_*).
    pub fn taskbar_state() -> u32 {
        let mut d = data(0, Edge::Bottom);
        // SAFETY: as above; the call reads no window.
        unsafe { SHAppBarMessage(ABM_GETSTATE, &mut d) as u32 }
    }

    // Sets it, as the taskbar's settings do (Windows keeps it, across restarts).
    pub fn set_taskbar_state(taskbar: isize, state: u32) {
        let mut d = data(taskbar, Edge::Bottom);
        d.lparam = state as isize;
        // SAFETY: as above, about the taskbar's window.
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut d) };
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Edge;

    pub const CALLBACK: u32 = 0;
    pub const ABN_STATECHANGE: usize = 0;
    pub const ABN_POSCHANGED: usize = 1;
    pub const ABN_FULLSCREENAPP: usize = 2;
    pub const ABS_AUTOHIDE: u32 = 1;

    pub fn register(_hwnd: isize) -> bool {
        false
    }
    pub fn remove(_hwnd: isize) {}
    pub fn place(_hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        edge.strip(mon, thickness)
    }
    pub fn query(hwnd: isize, mon: (i32, i32, i32, i32), edge: Edge, thickness: i32) -> (i32, i32, i32, i32) {
        place(hwnd, mon, edge, thickness)
    }
    pub fn taskbar_state() -> u32 {
        0
    }
    pub fn set_taskbar_state(_taskbar: isize, _state: u32) {}
    pub fn work_of(_mon: (i32, i32, i32, i32)) -> Option<(i32, i32, i32, i32)> {
        None
    }
    pub fn set_work(_r: (i32, i32, i32, i32)) -> bool {
        false
    }
}

// What a window that hears Windows about its bar uses (examples/taskbar_spike.rs
// does; her home's window, a webview's, does not hear it).
#[allow(unused_imports)]
pub use imp::{query, ABN_FULLSCREENAPP, ABN_POSCHANGED, ABN_STATECHANGE, CALLBACK};
pub use imp::{place, register, remove, set_taskbar_state, set_work, taskbar_state, work_of, ABS_AUTOHIDE};

#[cfg(all(test, windows, target_pointer_width = "64"))]
mod tests {
    #[test]
    fn the_struct_is_the_size_windows_expects() {
        // APPBARDATA on 64-bit Windows: 48 bytes.
        assert_eq!(std::mem::size_of::<super::imp::AppBarData>(), 48);
    }
}

#[cfg(test)]
mod edge_tests {
    use super::Edge;

    #[test]
    fn a_strip_along_each_edge() {
        let mon = (0, 0, 2560, 1440);
        assert_eq!(Edge::of("bottom").strip(mon, 72), (0, 1368, 2560, 72));
        assert_eq!(Edge::of("top").strip(mon, 72), (0, 0, 2560, 72));
        assert_eq!(Edge::of("left").strip(mon, 72), (0, 0, 72, 1440));
        assert_eq!(Edge::of("right").strip((-1920, 0, 1920, 1080), 48), (-48, 0, 48, 1080));
        assert_eq!(Edge::of("nope"), Edge::Bottom);
        assert!(Edge::Left.is_side() && Edge::Right.is_side() && !Edge::Top.is_side());
    }

    #[test]
    fn a_work_area_against_a_strip_on_another_display() {
        // A display right of the main one at 175%, its strip 84 px thick.
        let mon = (2560, -149, 2560, 1600);
        let left = Edge::Left.strip(mon, 84);
        // The whole display: it reaches under a strip along the left; made to stop at its inner side.
        assert!(Edge::Left.reaches(mon, left));
        assert_eq!(Edge::Left.with_side(mon, Edge::Left.inner(left)), (2644, -149, 2476, 1600));
        // Once made so, it does not.
        assert!(!Edge::Left.reaches((2644, -149, 2476, 1600), left));
        // Along the bottom, Windows' own room there (84 px) already keeps it clear.
        let bottom = Edge::Bottom.strip(mon, 84);
        assert_eq!(Edge::Bottom.inner(bottom), 1367);
        assert!(!Edge::Bottom.reaches((2560, -149, 2560, 1516), bottom));
        assert!(Edge::Bottom.reaches(mon, bottom));
        // Along the top and the right.
        assert_eq!(Edge::Top.with_side(mon, Edge::Top.inner(Edge::Top.strip(mon, 84))), (2560, -65, 2560, 1516));
        assert_eq!(Edge::Right.with_side(mon, Edge::Right.inner(Edge::Right.strip(mon, 84))), (2560, -149, 2476, 1600));
        assert_eq!(Edge::Right.side_of(mon), 5120);
    }

    #[test]
    fn a_side_an_earlier_strip_held_in_is_given_back() {
        // The main display at 125%, her strip along the left (60 px); Explorer
        // kept the right's 60 px from when it was along the right.
        let mon = (0, 0, 2560, 1440);
        let left = Edge::Left.strip(mon, 60);
        assert_eq!(Edge::Left.stale_sides((60, 0, 2440, 1440), left, mon), vec![(Edge::Right, 2560)]);
        // Only the strip's own side held in: nothing else to give back.
        assert!(Edge::Left.stale_sides((60, 0, 2500, 1440), left, mon).is_empty());
        // Another bar's side of another thickness is not hers: left be.
        assert!(Edge::Left.stale_sides((60, 0, 2400, 1440), left, mon).is_empty());
        // Along the top, the left's 60 px kept from before.
        assert_eq!(Edge::Top.stale_sides((60, 60, 2500, 1380), Edge::Top.strip(mon, 60), mon), vec![(Edge::Left, 0)]);
        // Along the bottom, none (the room there is Windows' own).
        assert!(Edge::Bottom.stale_sides((60, 0, 2500, 1380), Edge::Bottom.strip(mon, 60), mon).is_empty());
    }
}
