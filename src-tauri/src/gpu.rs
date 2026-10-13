// The graphics card that drives a display, for WebView2 to render her pages
// on (#13). WebView2 renders on the card Windows prefers for it (the
// power-saving one: the integrated), wherever her windows are: on a display
// the discrete card drives, each frame was drawn on the integrated one and
// copied across, and her animations stuttered while the integrated one was
// busy (a video playing in Chrome, there too). Rendered on the card that
// drives her display, the copy is gone; and on a display the integrated
// card drives, the discrete one is never woken for her.
//
// Asked of Windows' display kernel by the display's GDI name: its card's
// LUID, and whether that card renders at all (a remote desktop's virtual
// display, an indirect one, does not: then Chromium chooses, as before).

#[cfg(windows)]
mod imp {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Rect {
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
    }

    // MONITORINFOEXW.
    #[repr(C)]
    #[derive(Default)]
    struct MonitorInfo {
        size: u32,
        monitor: Rect,
        work: Rect,
        flags: u32,
        device: [u16; 32],
    }

    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Luid {
        low: u32,
        high: i32,
    }

    // D3DKMT_OPENADAPTERFROMGDIDISPLAYNAME.
    #[repr(C)]
    #[derive(Default)]
    struct OpenFromName {
        device: [u16; 32],
        adapter: u32,
        luid: Luid,
        source: u32,
    }

    // D3DKMT_QUERYADAPTERINFO.
    #[repr(C)]
    struct QueryInfo {
        adapter: u32,
        kind: u32,
        data: *mut c_void,
        size: u32,
    }

    // D3DKMT_CLOSEADAPTER.
    #[repr(C)]
    struct Close {
        adapter: u32,
    }

    // GetMonitorInfoW given the longer MONITORINFOEXW here (the display's
    // name), the plain MONITORINFO elsewhere: the same call.
    #[allow(clashing_extern_declarations)]
    #[link(name = "user32")]
    extern "system" {
        fn MonitorFromPoint(pt: Point, flags: u32) -> *mut c_void;
        fn GetMonitorInfoW(monitor: *mut c_void, info: *mut MonitorInfo) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn D3DKMTOpenAdapterFromGdiDisplayName(open: *mut OpenFromName) -> i32;
        fn D3DKMTQueryAdapterInfo(query: *const QueryInfo) -> i32;
        fn D3DKMTCloseAdapter(close: *const Close) -> i32;
    }

    const MONITOR_DEFAULTTONEAREST: u32 = 2;
    // KMTQAITYPE_ADAPTERTYPE, and in its D3DKMT_ADAPTERTYPE: RenderSupported,
    // SoftwareDevice, IndirectDisplayDevice.
    const KMTQAITYPE_ADAPTERTYPE: u32 = 15;
    const RENDERS: u32 = 0x1;
    const SOFTWARE: u32 = 0x4;
    const INDIRECT: u32 = 0x40;

    // The card driving the display at (x, y) (physical px): its LUID (high,
    // low), and the display's name, if it renders.
    pub fn card_of(x: i32, y: i32) -> Option<((i32, u32), String)> {
        // SAFETY: our own structs, sized as the calls ask; the adapter opened is closed.
        unsafe {
            let monitor = MonitorFromPoint(Point { x, y }, MONITOR_DEFAULTTONEAREST);
            let mut info = MonitorInfo { size: std::mem::size_of::<MonitorInfo>() as u32, ..Default::default() };
            if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
                return None;
            }
            let mut open = OpenFromName { device: info.device, ..Default::default() };
            if D3DKMTOpenAdapterFromGdiDisplayName(&mut open) != 0 {
                return None;
            }
            let mut kind = 0u32;
            let query = QueryInfo { adapter: open.adapter, kind: KMTQAITYPE_ADAPTERTYPE, data: &mut kind as *mut u32 as *mut c_void, size: 4 };
            let asked = D3DKMTQueryAdapterInfo(&query) == 0;
            D3DKMTCloseAdapter(&Close { adapter: open.adapter });
            let len = info.device.iter().position(|&c| c == 0).unwrap_or(32);
            let name = String::from_utf16_lossy(&info.device[..len]);
            (asked && kind & RENDERS != 0 && kind & (SOFTWARE | INDIRECT) == 0).then_some(((open.luid.high, open.luid.low), name))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn card_of(_x: i32, _y: i32) -> Option<((i32, u32), String)> {
        None
    }
}

pub use imp::card_of;

// Chromium's switch for it (ui/gl/gl_display.cc reads "<high>,<low>", in
// decimal), and the display's name for the log; none when the card cannot
// be told.
pub fn switch_for(display: (i32, i32, i32, i32)) -> Option<(String, String)> {
    let (x, y, w, h) = display;
    card_of(x + w / 2, y + h / 2).map(|((high, low), name)| (format!("--use-adapter-luid={high},{low}"), name))
}
