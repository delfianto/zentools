use crate::{
    view::State,
    worker::{self, Command},
};
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::{QString, QUrl};
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::Sender,
    },
};
pub static SMOKE_FAILED: AtomicBool = AtomicBool::new(false);
pub static SMOKE_CHECKS: AtomicUsize = AtomicUsize::new(0);
#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("cxx-qt-lib/qurl.h");
        type QUrl = cxx_qt_lib::QUrl;
    }
    #[auto_cxx_name]
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qproperty(QString, view_json)]
        #[qproperty(bool, smoke_test)]
        #[qproperty(i32, initial_page)]
        type Backend = super::BackendRust;
        #[qinvokable]
        fn start(self: Pin<&mut Self>);
        #[qinvokable]
        fn pause(&self, value: bool);
        #[qinvokable]
        fn refresh(&self);
        #[qinvokable]
        fn connect_hardware(&self);
        #[qinvokable]
        fn disconnect_hardware(&self);
        #[qinvokable]
        fn apply_profile(&self, level: i32);
        #[qinvokable]
        fn capture_baseline(&self);
        #[qinvokable]
        fn clear_baseline(&self);
        #[qinvokable]
        fn import_firmware(&self, url: &QUrl);
        #[qinvokable]
        fn load_baseline(&self, url: &QUrl);
        #[qinvokable]
        fn save_snapshot(&self, url: &QUrl);
        #[qinvokable]
        fn smoke_check(&self, passed: bool, message: &QString);
        #[qinvokable]
        fn shutdown(self: Pin<&mut Self>);
    }
    impl cxx_qt::Threading for Backend {}
}
pub struct BackendRust {
    view_json: QString,
    smoke_test: bool,
    initial_page: i32,
    tx: Option<Sender<Command>>,
}
impl Default for BackendRust {
    fn default() -> Self {
        let config = crate::config();
        Self {
            view_json: QString::from(
                &State {
                    demo: config.demo,
                    ..State::default()
                }
                .render(),
            ),
            smoke_test: config.smoke,
            initial_page: config.page,
            tx: None,
        }
    }
}
impl BackendRust {
    fn send(&self, command: Command) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(command);
        }
    }
}
impl Drop for BackendRust {
    fn drop(&mut self) {
        self.send(Command::Stop);
    }
}
impl qobject::Backend {
    pub fn start(mut self: Pin<&mut Self>) {
        if self.rust().tx.is_some() {
            return;
        }
        let tx = worker::spawn(self.qt_thread(), crate::config().demo);
        let mut inner = self.as_mut().rust_mut();
        inner.tx = Some(tx);
    }
    pub fn pause(&self, value: bool) {
        self.rust().send(Command::Pause(value));
    }
    pub fn refresh(&self) {
        self.rust().send(Command::Refresh);
    }
    pub fn connect_hardware(&self) {
        self.rust().send(Command::Connect);
    }
    pub fn disconnect_hardware(&self) {
        self.rust().send(Command::Disconnect);
    }
    pub fn apply_profile(&self, level: i32) {
        self.rust().send(Command::Apply(level));
    }
    pub fn capture_baseline(&self) {
        self.rust().send(Command::Baseline);
    }
    pub fn clear_baseline(&self) {
        self.rust().send(Command::ClearBaseline);
    }
    pub fn import_firmware(&self, url: &QUrl) {
        if let Some(path) = url.to_local_file() {
            self.rust()
                .send(Command::ImportFirmware(path.to_string().into()));
        }
    }
    pub fn load_baseline(&self, url: &QUrl) {
        if let Some(path) = url.to_local_file() {
            self.rust()
                .send(Command::LoadBaseline(path.to_string().into()));
        }
    }
    pub fn save_snapshot(&self, url: &QUrl) {
        if let Some(path) = url.to_local_file() {
            self.rust().send(Command::Save(path.to_string().into()));
        }
    }
    pub fn smoke_check(&self, passed: bool, message: &QString) {
        SMOKE_CHECKS.fetch_add(1, Ordering::SeqCst);
        if !passed {
            SMOKE_FAILED.store(true, Ordering::SeqCst);
            eprintln!("GUI smoke check failed: {message}");
        }
    }
    pub fn shutdown(mut self: Pin<&mut Self>) {
        let mut inner = self.as_mut().rust_mut();
        inner.send(Command::Stop);
        inner.tx = None;
    }
}
