pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import ZenTools

ScrollView {
    id: capturesScroll

    required property var viewState
    required property Backend bridge
    property bool onlyChanges: true
    property int comparisonCategory: 0
    property string comparisonFilter: ""
    readonly property var categories: ["All fields", "Telemetry", "Memory registers", "PM table raw words", "Firmware settings"]
    readonly property var differences: viewState.differences.filter(row => (!onlyChanges || row.changed) && (comparisonCategory === 0 || row.category === categories[comparisonCategory]) && (row.name + " " + row.category).toLowerCase().includes(comparisonFilter.toLowerCase()))
    function dateText(value) {
        return value ? new Date(value).toLocaleString(Qt.locale(), "dd MMM yyyy · HH:mm:ss") : "No capture";
    }
    signal saveRequested
    signal openRequested
    clip: true
    contentWidth: availableWidth
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ColumnLayout {
        width: capturesScroll.availableWidth
        spacing: 18
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 26
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            spacing: 10
            ActionButton {
                text: "Capture baseline"
                primary: true
                enabled: capturesScroll.viewState.ready
                onClicked: capturesScroll.bridge.captureBaseline()
            }
            ActionButton {
                text: "Open baseline…"
                onClicked: capturesScroll.openRequested()
            }
            Item {
                Layout.fillWidth: true
            }
            ActionButton {
                text: "Export current…"
                enabled: capturesScroll.viewState.ready
                onClicked: capturesScroll.saveRequested()
            }
            ActionButton {
                text: "Clear"
                quiet: true
                enabled: capturesScroll.viewState.baseline.loaded
                onClicked: capturesScroll.bridge.clearBaseline()
            }
        }
        GridLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            columns: 2
            columnSpacing: 16
            Panel {
                Layout.fillWidth: true
                implicitHeight: 112
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 22
                    spacing: 9
                    Text {
                        text: "BASELINE"
                        color: "#98a5b4"
                        font.pixelSize: 13
                        font.letterSpacing: 1.3
                    }
                    Text {
                        text: capturesScroll.viewState.baseline.loaded ? capturesScroll.dateText(capturesScroll.viewState.baseline.timestamp) : "No baseline captured"
                        color: "#415b72"
                        font.pixelSize: 22
                        font.weight: Font.DemiBold
                    }
                    Text {
                        text: capturesScroll.viewState.baseline.loaded ? "Preserved until you capture or open another baseline" : "Capture the current reading before starting your workload"
                        color: "#8f9cab"
                        font.pixelSize: 14
                    }
                }
            }
            Panel {
                Layout.fillWidth: true
                implicitHeight: 112
                ColumnLayout {
                    anchors.fill: parent
                    anchors.margins: 22
                    spacing: 9
                    Text {
                        text: "CURRENT SAMPLE"
                        color: "#98a5b4"
                        font.pixelSize: 13
                        font.letterSpacing: 1.3
                    }
                    Text {
                        text: capturesScroll.dateText(capturesScroll.viewState.timestamp)
                        color: "#415b72"
                        font.pixelSize: 22
                        font.weight: Font.DemiBold
                    }
                    Text {
                        text: capturesScroll.viewState.baseline.loaded ? capturesScroll.viewState.differences.filter(row => row.changed).length + " changed / " + capturesScroll.viewState.differences.length + " compared fields" : "Includes memory registers, firmware settings and raw PM-table words"
                        color: "#8f9cab"
                        font.pixelSize: 14
                    }
                }
            }
        }
        EmptyState {
            visible: !capturesScroll.viewState.baseline.loaded
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            title: "Make changes visible"
            description: "Capture a baseline while idle, run your workload, then compare the incoming readings. Export either state to keep it. Refresh memory timings to recapture UMC registers."
        }
        Text {
            visible: !!capturesScroll.viewState.compareError
            Layout.leftMargin: 30
            text: capturesScroll.viewState.compareError || ""
            color: "#a1493b"
            font.pixelSize: 17
        }
        Panel {
            visible: capturesScroll.viewState.baseline.loaded
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: comparisonColumn.implicitHeight + 40
            ColumnLayout {
                id: comparisonColumn
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: 20
                spacing: 16
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12
                    Text {
                        text: "Field comparison"
                        color: "#30485e"
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                        Layout.fillWidth: true
                    }
                    TextField {
                        placeholderText: "Filter name or address"
                        onTextChanged: capturesScroll.comparisonFilter = text
                        implicitWidth: 190
                        font.pixelSize: 15
                    }
                    ComboBox {
                        model: capturesScroll.categories
                        currentIndex: capturesScroll.comparisonCategory
                        onActivated: capturesScroll.comparisonCategory = currentIndex
                        implicitWidth: 170
                        font.pixelSize: 15
                    }
                    CheckBox {
                        text: "Changes only"
                        checked: capturesScroll.onlyChanges
                        onToggled: capturesScroll.onlyChanges = checked
                        font.pixelSize: 15
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "FIELD"
                        color: "#94a0b0"
                        font.pixelSize: 13
                        font.letterSpacing: 1
                        Layout.fillWidth: true
                    }
                    Text {
                        text: "BASELINE"
                        color: "#94a0b0"
                        font.pixelSize: 13
                        font.letterSpacing: 1
                        Layout.preferredWidth: 190
                    }
                    Text {
                        text: "CURRENT"
                        color: "#94a0b0"
                        font.pixelSize: 13
                        font.letterSpacing: 1
                        Layout.preferredWidth: 190
                    }
                }
                ListView {
                    id: comparisonList
                    Layout.fillWidth: true
                    implicitHeight: Math.min(540, Math.max(100, capturesScroll.differences.length * 46))
                    model: capturesScroll.differences
                    clip: true
                    ScrollBar.vertical: ScrollBar {}
                    delegate: Item {
                        id: comparisonRow
                        required property var modelData
                        width: comparisonList.width
                        height: 46
                        Rectangle {
                            anchors.bottom: parent.bottom
                            width: parent.width
                            height: 1
                            color: "#eef2f6"
                        }
                        RowLayout {
                            anchors.fill: parent
                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 3
                                Text {
                                    text: comparisonRow.modelData.name
                                    color: "#516b81"
                                    font.pixelSize: 15
                                }
                                Text {
                                    text: comparisonRow.modelData.category
                                    color: "#a5b0bd"
                                    font.pixelSize: 11
                                }
                            }
                            Text {
                                text: comparisonRow.modelData.before
                                color: "#8b9aaa"
                                font.family: "monospace"
                                font.pixelSize: 15
                                Layout.preferredWidth: 190
                                elide: Text.ElideRight
                            }
                            Text {
                                text: comparisonRow.modelData.after
                                color: comparisonRow.modelData.changed ? "#088674" : "#60778e"
                                font.family: "monospace"
                                font.pixelSize: 15
                                font.weight: Font.Medium
                                Layout.preferredWidth: 190
                                elide: Text.ElideRight
                            }
                        }
                    }
                    Text {
                        anchors.centerIn: parent
                        visible: capturesScroll.differences.length === 0
                        text: "No differences match this view."
                        color: "#8a97a7"
                        font.pixelSize: 17
                    }
                }
            }
        }
        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 30
            text: "Raw words are compared only when PM-table version and size match. An unchanged timing table during CPU load is expected; a changed value does not establish its meaning."
            color: "#91a0b0"
            font.pixelSize: 15
            wrapMode: Text.Wrap
        }
        Item {
            implicitHeight: 20
        }
    }
}
