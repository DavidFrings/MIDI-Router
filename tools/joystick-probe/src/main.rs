use gilrs::{EventType, Gilrs, GilrsBuilder};
use std::fs::File;
use std::io::{Write, stdin};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct Log {
    file: Arc<Mutex<File>>,
    start: Instant,
    step: Arc<Mutex<String>>,
}

impl Log {
    fn line(&self, msg: &str) {
        let step = self.step.lock().map(|s| s.clone()).unwrap_or_default();
        let l = format!(
            "[{:>7} ms][{}] {}",
            self.start.elapsed().as_millis(),
            step,
            msg
        );
        println!("{l}");
        if let Ok(mut f) = self.file.lock() {
            let _ = writeln!(f, "{l}");
            let _ = f.flush();
        }
    }

    fn set_step(&self, s: &str) {
        if let Ok(mut g) = self.step.lock() {
            *g = s.to_string();
        }
    }
}

fn hex(b: &[u8]) -> String {
    b.iter()
        .map(|x| format!("{x:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Drains pending gilrs events into the log, then sleeps briefly.
fn pump(log: &Log, gilrs: &mut Option<Gilrs>) {
    if let Some(g) = gilrs.as_mut() {
        while let Some(ev) = g.next_event() {
            match ev.event {
                EventType::ButtonPressed(b, c) => {
                    let code = c.into_u32();
                    log.line(&format!("gilrs ButtonPressed {b:?} code={code} value=1"));
                    println!("Button {code} gedrückt");
                }
                EventType::ButtonReleased(b, c) => {
                    log.line(&format!(
                        "gilrs ButtonReleased {b:?} code={} value=0",
                        c.into_u32()
                    ));
                }
                EventType::ButtonChanged(b, v, c) => {
                    log.line(&format!(
                        "gilrs ButtonChanged {b:?} code={} value={v}",
                        c.into_u32()
                    ));
                }
                EventType::AxisChanged(a, v, c) => {
                    log.line(&format!(
                        "gilrs AxisChanged {a:?} code={} value={v}",
                        c.into_u32()
                    ));
                }
                other => log.line(&format!("gilrs {other:?}")),
            }
        }
    }
    std::thread::sleep(Duration::from_millis(5));
}

fn main() {
    const NAME: &str = "joystick-probe-log.txt";
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));
    let candidates = [
        exe_dir.map(|d| d.join(NAME)),
        Some(std::env::temp_dir().join(NAME)),
    ];
    let mut opened = None;
    for path in candidates.into_iter().flatten() {
        match File::create(&path) {
            Ok(f) => {
                opened = Some((path, f));
                break;
            }
            Err(e) => println!(
                "Log-Datei {} kann nicht angelegt werden: {e}",
                path.display()
            ),
        }
    }
    let Some((path, file)) = opened else {
        println!("Es konnte keine Log-Datei angelegt werden. Bitte Enter drücken zum Beenden.");
        let _ = stdin().read_line(&mut String::new());
        return;
    };
    let log = Log {
        file: Arc::new(Mutex::new(file)),
        start: Instant::now(),
        step: Arc::new(Mutex::new("0".into())),
    };
    log.line(&format!("Log-Datei: {}", path.display()));
    log.line(&format!("OS: {}", std::env::consts::OS));

    // Enter presses arrive over a channel; gilrs stays on the main thread.
    let (tx, rx) = mpsc::channel::<()>();
    std::thread::spawn(move || {
        let mut s = String::new();
        while matches!(stdin().read_line(&mut s), Ok(n) if n > 0) {
            s.clear();
            if tx.send(()).is_err() {
                break;
            }
        }
    });

    let mut gilrs = match GilrsBuilder::new().with_default_filters(false).build() {
        Ok(g) => {
            for (id, gp) in g.gamepads() {
                log.line(&format!(
                    "gilrs pad {id}: name={:?} os_name={:?} vid={:04X?} pid={:04X?} uuid={} power={:?}",
                    gp.name(),
                    gp.os_name(),
                    gp.vendor_id(),
                    gp.product_id(),
                    hex(&gp.uuid()),
                    gp.power_info()
                ));
            }
            Some(g)
        }
        Err(e) => {
            log.line(&format!("gilrs Fehler: {e}"));
            None
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    let mut hid_threads = Vec::new();
    match hidapi::HidApi::new() {
        Ok(api) => {
            for d in api.device_list() {
                log.line(&format!(
                    "hid: vid={:04X} pid={:04X} product={:?} usage_page={} usage={} interface={} path={:?}",
                    d.vendor_id(),
                    d.product_id(),
                    d.product_string(),
                    d.usage_page(),
                    d.usage(),
                    d.interface_number(),
                    d.path()
                ));
            }
            let all = || api.device_list();
            let mut chosen: Vec<_> = all()
                .filter(|d| {
                    d.vendor_id() == 0x044F && d.usage_page() == 1 && matches!(d.usage(), 4 | 5)
                })
                .collect();
            if chosen.is_empty() {
                chosen = all().filter(|d| d.vendor_id() == 0x044F).collect();
            }
            if chosen.is_empty() {
                chosen = all()
                    .filter(|d| d.usage_page() == 1 && d.usage() == 4)
                    .take(1)
                    .collect();
            }
            if chosen.is_empty() {
                log.line("hid: kein passendes Gerät gefunden");
            }
            for (idx, info) in chosen.into_iter().enumerate() {
                let tag = format!("hid#{idx} {:?}", info.path());
                log.line(&format!(
                    "{tag}: gewählt vid={:04X} pid={:04X} usage_page={} usage={} {:?}",
                    info.vendor_id(),
                    info.product_id(),
                    info.usage_page(),
                    info.usage(),
                    info.product_string()
                ));
                let dev = match info.open_device(&api) {
                    Ok(d) => d,
                    Err(e) => {
                        log.line(&format!("{tag}: open Fehler: {e}"));
                        continue;
                    }
                };
                let mut desc = [0u8; 4096];
                match dev.get_report_descriptor(&mut desc) {
                    Ok(n) => log.line(&format!(
                        "{tag}: Report-Descriptor ({n} Bytes): {}",
                        hex(&desc[..n])
                    )),
                    Err(e) => log.line(&format!("{tag}: Descriptor Fehler: {e}")),
                }
                let (l, stop) = (log.clone(), stop.clone());
                hid_threads.push(std::thread::spawn(move || {
                    let mut buf = [0u8; 256];
                    let mut prev: Vec<u8> = Vec::new();
                    while !stop.load(Ordering::Relaxed) {
                        match dev.read_timeout(&mut buf, 50) {
                            Ok(0) => {}
                            Ok(n) => {
                                if buf[..n] != prev[..] {
                                    l.line(&format!("{tag} report: {}", hex(&buf[..n])));
                                    prev = buf[..n].to_vec();
                                }
                            }
                            Err(e) => {
                                l.line(&format!("{tag}: Lesefehler: {e}"));
                                break;
                            }
                        }
                    }
                }));
            }
        }
        Err(e) => log.line(&format!("hidapi Fehler: {e}")),
    }

    let steps = [
        ("1", "Geräte suchen: fertig (siehe oben). Weiter mit Enter."),
        (
            "2",
            "Drücke nacheinander Trigger, Button 2, 3, 4 – dann Enter",
        ),
        (
            "3",
            "Drücke die Basis-Buttons 5–16 der Reihe nach – dann Enter",
        ),
        ("4", "Hat: hoch, rechts, runter, links – dann Enter"),
        (
            "5",
            "Stick: ganz links, ganz rechts, ganz vor, ganz zurück, dann loslassen – dann Enter",
        ),
    ];
    for (id, text) in steps {
        log.set_step(id);
        log.line(&format!("Schritt {id}: {text}"));
        while rx.try_recv().is_err() {
            pump(&log, &mut gilrs);
        }
    }

    log.set_step("6");
    log.line("Hintergrund-Test: Klick JETZT in ein anderes Fenster (z. B. Editor). Dann Trigger drücken und Stick bewegen. 15 Sekunden…");
    let t0 = Instant::now();
    let mut last = u64::MAX;
    while t0.elapsed() < Duration::from_secs(15) {
        let left = 15 - t0.elapsed().as_secs();
        if left != last {
            last = left;
            log.line(&format!("noch {left} s"));
        }
        pump(&log, &mut gilrs);
    }
    while rx.try_recv().is_ok() {}

    log.set_step("7");
    log.line(&format!(
        "Fertig. Schick die Datei joystick-probe-log.txt zurück ({}). Enter zum Beenden.",
        path.display()
    ));
    let _ = rx.recv();
    stop.store(true, Ordering::Relaxed);
    for h in hid_threads {
        let _ = h.join();
    }
}
