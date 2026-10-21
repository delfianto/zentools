pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import ZenTools

ApplicationWindow {
    id: window
    width: 1380
    height: 920
    minimumWidth: 1080
    minimumHeight: 720
    visible: true
    title: "ZenTools · Ryzen workstation"
    color: "#f3f5f8"
    font.family: "Inter"
    font.pixelSize: 12
    palette.highlight: "#088674"
    palette.buttonText: "#263448"
    palette.text: "#263448"
    palette.windowText: "#263448"
    palette.base: "white"

    Backend {
        id: backendBridge
        Component.onCompleted: start()
    }
    readonly property var state: JSON.parse(backendBridge.viewJson)
    property int page: backendBridge.initialPage
    property double wallTime: Date.now()
    readonly property bool stale: state.ready && !state.paused && wallTime - state.timestamp > 5000
    readonly property var titles: ["Overview", "Memory", "Power profiles", "Captures & comparison"]
    readonly property var subtitles: ["A clear view of your Ryzen workstation.", "Controller timings, subtimings and firmware settings.", "Choose how your CPU balances speed and efficiency.", "See what changes between idle, load and recovery."]
    function navigateTo(index: int) {
        const button = navigationItems.itemAt(index) as Button;
        button.clicked();
    }
    function check(value, message) {
        backendBridge.smokeCheck(value, message);
        if (!value)
            Qt.quit();
    }
    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: window.wallTime = Date.now()
    }
    onClosing: backendBridge.shutdown()

    FileDialog {
        id: firmwareDialog
        title: "Import audited AOD getter capture"
        nameFilters: ["AOD getter captures (*.txt)", "All files (*)"]
        onAccepted: backendBridge.importFirmware(selectedFile)
    }
    FileDialog {
        id: saveDialog
        title: "Save hardware snapshot"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "json"
        nameFilters: ["ZenTools snapshots (*.json)"]
        onAccepted: backendBridge.saveSnapshot(selectedFile)
    }
    FileDialog {
        id: baselineDialog
        title: "Open a baseline snapshot"
        nameFilters: ["ZenTools snapshots (*.json)"]
        onAccepted: backendBridge.loadBaseline(selectedFile)
    }

    RowLayout {
        anchors.fill: parent
        spacing: 0
        Rectangle {
            Layout.preferredWidth: 224
            Layout.fillHeight: true
            color: "#142132"
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 18
                spacing: 8
                RowLayout {
                    Layout.topMargin: 12
                    Layout.bottomMargin: 32
                    spacing: 12
                    Rectangle {
                        implicitWidth: 34
                        implicitHeight: 34
                        radius: 9
                        color: "#72dfbe"
                        Text {
                            anchors.centerIn: parent
                            text: "Z"
                            color: "#112d2c"
                            font.pixelSize: 23
                            font.weight: Font.Bold
                            font.italic: true
                        }
                    }
                    Text {
                        text: "ZenTools"
                        color: "#f3f8fc"
                        font.pixelSize: 21
                        font.weight: Font.DemiBold
                        font.letterSpacing: -0.6
                    }
                }
                Text {
                    text: "WORKSPACE"
                    color: "#65788e"
                    font.pixelSize: 9
                    font.weight: Font.DemiBold
                    font.letterSpacing: 1.8
                    Layout.leftMargin: 12
                    Layout.bottomMargin: 10
                }
                Repeater {
                    id: navigationItems
                    model: ["Overview", "Memory", "Power profiles", "Captures"]
                    delegate: Button {
                        id: nav
                        required property int index
                        required property string modelData
                        objectName: "navigation" + index
                        Accessible.name: modelData
                        Layout.fillWidth: true
                        implicitHeight: 47
                        hoverEnabled: true
                        onClicked: window.page = nav.index
                        background: Rectangle {
                            color: window.page === nav.index ? "#26394e" : nav.hovered ? "#1c2d42" : "transparent"
                            radius: 8
                        }
                        contentItem: RowLayout {
                            spacing: 12
                            Glyph {
                                symbol: nav.index
                                ink: window.page === nav.index ? "#80e5c7" : "#91a2b7"
                                Layout.leftMargin: 12
                            }
                            Text {
                                text: nav.modelData
                                color: window.page === nav.index ? "#f1f7fa" : "#a5b3c4"
                                font.pixelSize: 12
                                font.weight: window.page === nav.index ? Font.DemiBold : Font.Normal
                                Layout.fillWidth: true
                            }
                            Rectangle {
                                implicitWidth: 5
                                implicitHeight: 5
                                radius: 3
                                color: "#80e5c7"
                                visible: window.page === nav.index
                                Layout.rightMargin: 10
                            }
                        }
                    }
                }
                Item {
                    Layout.fillHeight: true
                }
                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: 132
                    radius: 10
                    color: "#1c2e43"
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 15
                        spacing: 8
                        Text {
                            text: "YOUR PROCESSOR"
                            color: "#7890a9"
                            font.pixelSize: 9
                            font.letterSpacing: 1.2
                        }
                        Text {
                            text: window.state.cpu.replace("AMD ", "").replace(" 16-Core Processor", "")
                            color: "#e1eaf3"
                            font.pixelSize: 13
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        Text {
                            text: window.state.physicalCores ? window.state.physicalCores + " cores  /  " + window.state.logicalCpus + " threads" : "Identifying processor…"
                            color: "#91a5ba"
                            font.pixelSize: 10
                        }
                    }
                }
                RowLayout {
                    Layout.topMargin: 17
                    Layout.bottomMargin: 6
                    Layout.leftMargin: 5
                    Rectangle {
                        implicitWidth: 5
                        implicitHeight: 5
                        radius: 3
                        color: window.state.demo ? "#dfaf66" : "#72dfbe"
                    }
                    Text {
                        text: window.state.demo ? "PREVIEW MODE" : "LOCAL WORKSTATION"
                        color: "#768ba2"
                        font.pixelSize: 8
                        font.letterSpacing: 1.3
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    Text {
                        text: "0.3"
                        color: "#60768e"
                        font.pixelSize: 9
                    }
                }
            }
        }
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0
            Rectangle {
                Layout.fillWidth: true
                implicitHeight: 104
                color: "#ffffff"
                Rectangle {
                    anchors.bottom: parent.bottom
                    width: parent.width
                    height: 1
                    color: "#e2e8ef"
                }
                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 30
                    anchors.rightMargin: 30
                    spacing: 10
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 7
                        Text {
                            text: window.titles[window.page]
                            color: "#17263a"
                            font.pixelSize: 24
                            font.weight: Font.DemiBold
                            font.letterSpacing: -0.7
                        }
                        Text {
                            text: window.subtitles[window.page]
                            color: "#7b899b"
                            font.pixelSize: 11
                        }
                    }
                    Badge {
                        text: window.state.paused ? "Paused" : window.stale ? "Stale" : window.state.ready ? (window.state.demo ? "Preview · 1s" : "Live · 1s") : "Connecting"
                        tone: window.stale ? "#b56d21" : window.state.paused ? "#6e7c8f" : "#087f71"
                    }
                    ActionButton {
                        text: window.state.paused ? "Resume" : "Pause"
                        quiet: true
                        enabled: window.state.ready
                        onClicked: backendBridge.pause(!window.state.paused)
                    }
                    ActionButton {
                        text: window.state.authorizing ? "Cancel authorization" : window.state.elevated ? "Disconnect hardware" : "Enable hardware access"
                        primary: !window.state.elevated
                        enabled: !window.state.demo
                        onClicked: window.state.elevated || window.state.authorizing ? backendBridge.disconnectHardware() : backendBridge.connectHardware()
                        ToolTip.text: "Authorize additional sensors and EPP changes through your system authentication dialog."
                    }
                }
            }
            Rectangle {
                Layout.fillWidth: true
                visible: window.state.demo || window.stale
                implicitHeight: 38
                color: "#fff4df"
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    anchors.leftMargin: 30
                    text: window.state.demo ? "Preview · Telemetry is simulated. Memory uses the recorded DDR5-6000 fixture. Hardware changes are disabled." : "The last sample is over 5 seconds old. Check the hardware connection or refresh."
                    color: "#94601f"
                    font.pixelSize: 11
                }
            }
            Rectangle {
                Layout.fillWidth: true
                visible: messageText.text.length > 0
                implicitHeight: messageText.implicitHeight + 20
                color: window.state.messageError ? "#fcecea" : "#e8f4ef"
                Text {
                    id: messageText
                    anchors.fill: parent
                    anchors.leftMargin: 30
                    anchors.rightMargin: 30
                    anchors.topMargin: 10
                    anchors.bottomMargin: 10
                    text: window.state.message
                    color: window.state.messageError ? "#a1493b" : "#287562"
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }
            }
            StackLayout {
                currentIndex: window.page
                Layout.fillWidth: true
                Layout.fillHeight: true
                OverviewPage {
                    viewState: window.state
                }
                MemoryPage {
                    viewState: window.state
                    bridge: backendBridge
                    onImportRequested: firmwareDialog.open()
                }
                ProfilesPage {
                    viewState: window.state
                    bridge: backendBridge
                }
                CapturesPage {
                    id: capturesPage
                    viewState: window.state
                    bridge: backendBridge
                    onOpenRequested: baselineDialog.open()
                    onSaveRequested: saveDialog.open()
                }
            }
        }
    }
    Timer {
        id: smokeTimer
        interval: 250
        running: backendBridge.smokeTest
        repeat: true
        property int step: 0
        property int attempts: 0
        onTriggered: {
            attempts++;
            if (attempts > 60) {
                window.check(false, "Timed out waiting for interface state");
                return;
            }
            if (!window.state.ready)
                return;
            switch (step) {
            case 0:
                window.check(window.state.demo && window.state.cores.length === 16, "Preview initializes 16 cores");
                window.navigateTo(1);
                break;
            case 1:
                window.check(window.state.channels.length === 2 && window.state.channels[0].groups.length === 4, "Memory channels and timing groups render");
                backendBridge.pause(true);
                break;
            case 2:
                window.check(window.state.paused, "Pause reaches worker");
                backendBridge.captureBaseline();
                break;
            case 3:
                if (!window.state.baseline.loaded)
                    return;
                window.check(true, "Baseline captured while paused");
                window.navigateTo(2);
                break;
            case 4:
                window.check(window.state.profiles.length === 4, "Four EPP preferences render");
                backendBridge.applyProfile(0);
                break;
            case 5:
                if (!window.state.message.includes("Preview mode"))
                    return;
                window.check(!window.state.elevated, "Preview rejects hardware changes");
                backendBridge.pause(false);
                window.navigateTo(3);
                break;
            case 6:
                window.check(!window.state.paused, "Live updates resume");
                capturesPage.onlyChanges = false;
                backendBridge.refresh();
                break;
            case 7:
                window.check(window.state.differences.some(row => row.category === "Memory registers"), "Comparison includes UMC register words");
                window.navigateTo(0);
                break;
            case 8:
                window.check(window.state.firmware.rows.length === 81, "All audited AOD objects are available");
                backendBridge.shutdown();
                Qt.quit();
                break;
            }
            step++;
        }
    }
}
