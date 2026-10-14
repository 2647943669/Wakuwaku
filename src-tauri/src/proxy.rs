// Proxies: Windows' own, as Settings (Network, Proxy) has it and as Clash or
// v2rayN set it when their "system proxy" is on, read for the crash reports
// (crash.rs: a URL for ureq) and for mail (mail/proxy.rs: an address and a
// kind to go through); and a proxy as one is typed in. The registry's
// ProxyServer is "host:port" for every protocol, or one for each:
// "http=host:port;https=host:port;socks=host:port"; ProxyOverride lists the
// hosts reached straight ("localhost;127.*;10.*;<local>", <local> being a
// name without a dot). A PAC script (AutoConfigURL, Clash's "PAC mode") is
// not read: as no proxy.

// How a proxy is spoken to: HTTP's CONNECT, or SOCKS5.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Http,
    Socks5,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        match s {
            "http" => Some(Kind::Http),
            "socks5" => Some(Kind::Socks5),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Http => "http",
            Kind::Socks5 => "socks5",
        }
    }
}

// A proxy: how it is spoken to, and where it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Proxy {
    pub kind: Kind,
    pub host: String,
    pub port: u16,
}

impl Proxy {
    // As one writes it: 127.0.0.1:7890, [::1]:1080.
    pub fn at(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }

    // As typed: host:port (an IPv6 address in brackets), of the kind given
    // unless a scheme before it says (http://, socks5://, socks5h://,
    // socks://); a slash after is left off. None when it is not one.
    pub fn parse(text: &str, kind: Kind) -> Option<Proxy> {
        let text = text.trim();
        let (kind, rest) = match text.split_once("://") {
            Some((scheme, rest)) => {
                let kind = match scheme.to_ascii_lowercase().as_str() {
                    "http" => Kind::Http,
                    "socks" | "socks5" | "socks5h" => Kind::Socks5,
                    _ => return None,
                };
                (kind, rest)
            }
            None => (kind, text),
        };
        let rest = rest.trim_end_matches('/');
        let (host, port) = match rest.strip_prefix('[') {
            Some(v6) => v6.split_once("]:").filter(|(h, _)| h.contains(':'))?,
            None => rest.rsplit_once(':').filter(|(h, _)| !h.contains(':'))?,
        };
        let port = port.parse::<u16>().ok().filter(|p| *p > 0)?;
        let ok = !host.is_empty() && host.len() <= 253 && host.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | ':'));
        ok.then(|| Proxy { kind, host: host.to_ascii_lowercase(), port })
    }
}

// Windows' proxy as set: ProxyServer, ProxyOverride.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct System {
    pub server: String,
    pub bypass: String,
}

impl System {
    // One protocol's entry of ProxyServer ("https=…"), when it has them.
    fn entry(&self, scheme: &str) -> Option<&str> {
        self.server.split(';').find_map(|part| {
            let (name, value) = part.split_once('=')?;
            Some(value.trim()).filter(|v| name.trim().eq_ignore_ascii_case(scheme) && !v.is_empty())
        })
    }

    // For the web (the crash reports' relays), as a URL: the https entry,
    // else the http one, else the one for every protocol.
    pub fn for_web(&self) -> Option<String> {
        let pick = if self.server.contains('=') { self.entry("https").or_else(|| self.entry("http"))? } else { self.server.trim() };
        if pick.is_empty() {
            return None;
        }
        Some(if pick.contains("://") { pick.to_string() } else { format!("http://{pick}") })
    }

    // For mail, to any port: the socks entry where there is one (as Windows
    // means it, the proxy for other protocols), else the https or http one,
    // else the one for every protocol, which Windows speaks HTTP to (Clash
    // sets its mixed port, which takes either; v2rayN its HTTP port).
    pub fn for_mail(&self) -> Option<Proxy> {
        if !self.server.contains('=') {
            return Proxy::parse(&self.server, Kind::Http);
        }
        match self.entry("socks") {
            Some(socks) => Proxy::parse(socks, Kind::Socks5),
            None => Proxy::parse(self.entry("https").or_else(|| self.entry("http"))?, Kind::Http),
        }
    }

    // Whether a host is reached straight, by ProxyOverride: an entry as the
    // host is (* for any run of characters), or <local> for a name with no dot.
    pub fn bypasses(&self, host: &str) -> bool {
        self.bypass.split(';').map(str::trim).filter(|e| !e.is_empty()).any(|e| {
            if e.eq_ignore_ascii_case("<local>") {
                !host.contains('.') && !host.contains(':')
            } else {
                glob(e.as_bytes(), host.as_bytes())
            }
        })
    }
}

// A pattern with * against a name, letters in either case.
fn glob(pattern: &[u8], text: &[u8]) -> bool {
    match pattern.split_first() {
        None => text.is_empty(),
        Some((b'*', rest)) => (0..=text.len()).any(|i| glob(rest, &text[i..])),
        Some((c, rest)) => text.split_first().is_some_and(|(t, more)| t.eq_ignore_ascii_case(c) && glob(rest, more)),
    }
}

// Windows' proxy, when one is on (and is not only a PAC script).
#[cfg(windows)]
pub fn system() -> Option<System> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let key = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings").ok()?;
    if key.get_value::<u32, _>("ProxyEnable").ok()? == 0 {
        return None;
    }
    let server: String = key.get_value("ProxyServer").ok()?;
    let bypass: String = key.get_value("ProxyOverride").unwrap_or_default();
    (!server.trim().is_empty()).then_some(System { server, bypass })
}

#[cfg(not(windows))]
pub fn system() -> Option<System> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sys(server: &str) -> System {
        System { server: server.into(), bypass: String::new() }
    }

    #[test]
    fn what_clash_and_v2rayn_write_is_read() {
        // Clash (Verge, for Windows): its mixed port, for everything.
        let clash = sys("127.0.0.1:7890");
        assert_eq!(clash.for_mail(), Some(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: 7890 }));
        assert_eq!(clash.for_web().as_deref(), Some("http://127.0.0.1:7890"));
        // v2rayN: its HTTP port.
        assert_eq!(sys("127.0.0.1:10809").for_mail().map(|p| p.at()).as_deref(), Some("127.0.0.1:10809"));
        // One for each protocol: mail through the socks one, the web through the https one.
        let each = sys("http=127.0.0.1:10809;https=127.0.0.1:10809;socks=127.0.0.1:10808");
        assert_eq!(each.for_mail(), Some(Proxy { kind: Kind::Socks5, host: "127.0.0.1".into(), port: 10808 }));
        assert_eq!(each.for_web().as_deref(), Some("http://127.0.0.1:10809"));
        assert_eq!(sys("http=127.0.0.1:8080;https=127.0.0.1:8443").for_mail().map(|p| (p.kind, p.port)), Some((Kind::Http, 8443)));
        assert_eq!(sys("ftp=1.2.3.4:21").for_mail(), None);
        assert_eq!(sys("SOCKS=localhost:1080").for_mail().map(|p| (p.kind, p.host)), Some((Kind::Socks5, "localhost".into())));
        assert_eq!(sys(" ").for_mail(), None);
        assert_eq!(sys("socks5://127.0.0.1:7891").for_mail().map(|p| p.kind), Some(Kind::Socks5));
    }

    #[test]
    fn some_hosts_are_reached_straight() {
        // Clash's list, shortened.
        let s = System { server: "127.0.0.1:7890".into(), bypass: "localhost;127.*;192.168.*;10.*;172.16.*;*.local;<local>".into() };
        assert!(s.bypasses("127.0.0.1"));
        assert!(s.bypasses("LOCALHOST"));
        assert!(s.bypasses("192.168.1.20"));
        assert!(s.bypasses("mail.corp.local"));
        assert!(s.bypasses("intranet"));
        assert!(!s.bypasses("imap.gmail.com"));
        assert!(!s.bypasses("10mail.example.com"));
        assert!(!sys("127.0.0.1:7890").bypasses("127.0.0.1"));
    }

    #[test]
    fn a_proxy_is_typed_as_one_writes_it() {
        let p = |t: &str| Proxy::parse(t, Kind::Http);
        assert_eq!(p(" 127.0.0.1:7890 "), Some(Proxy { kind: Kind::Http, host: "127.0.0.1".into(), port: 7890 }));
        assert_eq!(p("socks5://127.0.0.1:10808/").map(|x| (x.kind, x.port)), Some((Kind::Socks5, 10808)));
        assert_eq!(p("http://Proxy.Example.com:3128").map(|x| x.host), Some("proxy.example.com".into()));
        assert_eq!(p("[::1]:1080").map(|x| x.at()), Some("[::1]:1080".into()));
        for bad in ["127.0.0.1", "127.0.0.1:0", "127.0.0.1:99999", ":7890", "a b:1", "::1:1080", "https://x:1", "x:1 --y"] {
            assert_eq!(p(bad), None, "{bad}");
        }
    }
}
