// The plugins she runs for you, done inside her: the weather, stocks, a
// pomodoro, a deadline, a stretch reminder, CI and dev servers, each as its
// PowerShell script in examples/widgets had it. Here first what a plugin is
// (a turn, what it shows, how long until the next) and the helpers they
// share; they replace the scripts once all of them are here.
#![allow(dead_code)]
mod ci;
pub mod countdown;
mod devserver;
pub mod fmt;
mod pomodoro;
mod stock;
mod stretch;
mod weather;
pub mod web;

use std::time::Duration;

use serde_json::Value;

// --- What a plugin is ------------------------------------------------------------------

// The time a turn is given: ms since 1970, and the local calendar's ms since
// 1970 (the wall's clock, as a deadline is typed: countdown).
#[derive(Clone, Copy, Debug)]
pub struct Now {
    pub ms: u64,
    pub local: i64,
}

// How long until its next turn: while someone is about, and while nobody is
// (quiet) or she is out of sight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wait {
    pub busy: Duration,
    pub idle: Duration,
}

impl Wait {
    pub fn every(d: Duration) -> Wait {
        Wait { busy: d, idle: d }
    }

    fn of(&self, slow: bool) -> Duration {
        if slow {
            self.idle
        } else {
            self.busy
        }
    }
}

// Where a turn's words go: its widgets, as POST /widget took them; and what
// went wrong, in a line.
pub trait Out {
    fn post(&mut self, widget: Value);
    fn say(&mut self, line: String);
}

// A plugin running: one turn now, and how long until the next.
pub trait Job: Send {
    fn turn(&mut self, now: Now, out: &mut dyn Out) -> Wait;
}

// A GET on the web (web::get; a test's recorded answers).
pub type Fetch = Box<dyn Fn(&str, &[(&str, &str)]) -> Result<web::Answer, web::Fail> + Send>;

fn web_fetch() -> Fetch {
    Box::new(|url: &str, headers: &[(&str, &str)]| web::get(url, headers, web::TIMEOUT))
}

// A line of what went wrong, as the Plugins page shows it: its first, 160
// characters at most.
fn tidy(line: &str) -> String {
    line.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("").chars().take(160).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // --- For the plugins' own tests ---

    // What a script posted and warned, run with the web and the clock played
    // back to it (test/fixtures/plugins/ps/<name>.json): each post's
    // seconds since it started, and its body.
    pub fn golden(name: &str) -> (Vec<(f64, Value)>, Vec<String>) {
        let path = format!("{}/../test/fixtures/plugins/ps/{name}.json", env!("CARGO_MANIFEST_DIR"));
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        // ConvertTo-Json writes a list of one as the one.
        let list = |v: &Value| match v {
            Value::Array(a) => a.clone(),
            Value::Null => Vec::new(),
            one => vec![one.clone()],
        };
        let posts = list(&v["posts"]).into_iter().map(|p| (p["t"].as_f64().unwrap(), p["body"].clone())).collect();
        let warnings = list(&v["warnings"]).into_iter().map(|w| w.as_str().unwrap().to_string()).collect();
        (posts, warnings)
    }

    // Two widgets alike: the same fields, numbers alike as numbers (25 and
    // 25.0), a list of one as PowerShell wrote it either way.
    pub fn same(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
            (Value::Object(x), Value::Object(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w))),
            (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(v, w)| same(v, w)),
            (Value::Array(x), y) | (y, Value::Array(x)) if x.len() == 1 => same(&x[0], y),
            _ => a == b,
        }
    }

    #[derive(Default)]
    struct Record {
        posts: Vec<Value>,
        said: Option<String>,
    }

    impl Out for Record {
        fn post(&mut self, widget: Value) {
            self.posts.push(widget);
        }
        fn say(&mut self, line: String) {
            self.said = Some(tidy(&line));
        }
    }

    // One turn at a moment (ms since 1970): what it posted, said, and waits.
    pub fn record(job: &mut dyn Job, ms: u64) -> (Vec<Value>, Option<String>, Wait) {
        let mut out = Record::default();
        let wait = job.turn(Now { ms, local: ms as i64 }, &mut out);
        (out.posts, out.said, wait)
    }

    // The same at a moment of the local calendar (countdown).
    pub fn record_at(job: &mut dyn Job, local: i64) -> (Vec<Value>, Option<String>, Wait) {
        let mut out = Record::default();
        let wait = job.turn(Now { ms: local.max(0) as u64, local }, &mut out);
        (out.posts, out.said, wait)
    }

    pub type Route = Box<dyn Fn() -> (u16, String) + Send + Sync>;

    // The web played back: the first address each starts with answers so;
    // none, no answer. With the addresses asked, in turn.
    pub fn served_with(routes: Vec<(String, Route)>) -> (Fetch, Arc<Mutex<Vec<String>>>) {
        let asked = Arc::new(Mutex::new(Vec::new()));
        let heard = asked.clone();
        let fetch: Fetch = Box::new(move |url: &str, _: &[(&str, &str)]| {
            heard.lock().unwrap().push(url.to_string());
            match routes.iter().find(|(at, _)| url.starts_with(at.as_str())) {
                Some((_, answer)) => {
                    let (status, body) = answer();
                    Ok(web::Answer { status, body })
                }
                None => Err(web::Fail::Network("offline".into())),
            }
        });
        (fetch, asked)
    }

    // The same, each answer a recorded one (test/fixtures/plugins/<file>).
    pub fn served(routes: &[(&str, u16, &str)]) -> (Fetch, Arc<Mutex<Vec<String>>>) {
        served_with(
            routes
                .iter()
                .map(|(at, status, file)| {
                    let body = std::fs::read_to_string(format!("{}/../test/fixtures/plugins/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
                    let status = *status;
                    (at.to_string(), Box::new(move || (status, body.clone())) as Route)
                })
                .collect(),
        )
    }
}
