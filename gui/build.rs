use cxx_qt_build::{CxxQtBuilder, QmlModule};
fn main() {
    let module = QmlModule::new("ZenTools")
        .qml_file("qml/Main.qml")
        .qml_file("qml/OverviewPage.qml")
        .qml_file("qml/MemoryPage.qml")
        .qml_file("qml/ProfilesPage.qml")
        .qml_file("qml/CapturesPage.qml")
        .qml_file("qml/Panel.qml")
        .qml_file("qml/ActionButton.qml")
        .qml_file("qml/Badge.qml")
        .qml_file("qml/Glyph.qml")
        .qml_file("qml/MetricCard.qml")
        .qml_file("qml/HistoryChart.qml")
        .qml_file("qml/EmptyState.qml");
    CxxQtBuilder::new_qml_module(module)
        .qt_module("Quick")
        .files(["src/backend.rs"])
        .build();
}
