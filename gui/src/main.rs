mod backend;
mod demo;
mod hardware;
mod view;
mod worker;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};
use std::sync::{OnceLock, atomic::Ordering};
#[derive(Default)]
pub struct Config {
    demo: bool,
    smoke: bool,
    page: i32,
}
static CONFIG: OnceLock<Config> = OnceLock::new();
pub fn config() -> &'static Config {
    CONFIG.get_or_init(Config::default)
}
fn main() {
    let mut options = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--demo" => options.demo = true,
            "--smoke-test" => {
                options.smoke = true;
                options.demo = true;
            }
            "--page" => {
                options.page = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .filter(|v| (0..4).contains(v))
                    .unwrap_or_else(|| {
                        eprintln!("--page expects 0–3");
                        std::process::exit(2)
                    })
            }
            "--help" | "-h" => {
                println!(
                    "ZenTools GUI\n  --demo                 Preview with synthetic telemetry; hardware writes disabled\n  --smoke-test           Bounded offscreen UI interaction test (implies --demo)\n  --page 0–3             Initial page"
                );
                return;
            }
            _ => {
                eprintln!("Unknown option: {arg}");
                std::process::exit(2);
            }
        }
    }
    let _ = CONFIG.set(options);
    let mut app = QGuiApplication::new();
    let app_ref = app.as_mut().expect("Qt application");
    app_ref.set_application_name(&QString::from("ZenTools"));
    QGuiApplication::set_desktop_file_name(&QString::from("zentools"));
    let mut engine = QQmlApplicationEngine::new();
    engine
        .as_mut()
        .expect("Qt QML engine")
        .on_object_creation_failed(|_, _| {
            eprintln!("Unable to load the ZenTools interface");
            std::process::exit(1);
        })
        .release();
    engine
        .as_mut()
        .expect("Qt QML engine")
        .load(&QUrl::from("qrc:/qt/qml/ZenTools/qml/Main.qml"));
    let result = app.as_mut().expect("Qt application").exec();
    worker::shutdown_all();
    drop(engine);
    if config().smoke {
        let count = backend::SMOKE_CHECKS.load(Ordering::SeqCst);
        if backend::SMOKE_FAILED.load(Ordering::SeqCst) || count < 8 {
            eprintln!("GUI smoke test failed ({count} checks)");
            std::process::exit(1);
        }
        println!("GUI smoke test passed ({count} checks)");
    }
    if result != 0 {
        std::process::exit(result);
    }
}
