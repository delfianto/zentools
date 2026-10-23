pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import ZenTools

ScrollView {
    id: memoryScroll

    required property var viewState
    required property Backend bridge
    function timeText(value) {
        return value ? new Date(value).toLocaleTimeString(Qt.locale(), "HH:mm:ss") : "—";
    }
    property int channelSelection: 0
    property bool showRegisters: false
    property bool showAllFirmware: false
    property bool showCandidates: false
    readonly property var channel: viewState.channels.length > channelSelection ? viewState.channels[channelSelection] : null
    signal importRequested
    clip: true
    contentWidth: availableWidth
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ColumnLayout {
        width: memoryScroll.availableWidth
        spacing: 18
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 26
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            spacing: 10
            ComboBox {
                model: memoryScroll.viewState.channels.map(c => c.label)
                currentIndex: memoryScroll.channelSelection
                onActivated: memoryScroll.channelSelection = currentIndex
                implicitWidth: 150
                enabled: count > 0
            }
            Text {
                text: memoryScroll.viewState.memoryTimestamp ? "Read at " + memoryScroll.timeText(memoryScroll.viewState.memoryTimestamp) : "No memory capture"
                color: "#8997a7"
                font.pixelSize: 15
                Layout.fillWidth: true
            }
            ActionButton {
                text: "Import firmware capture"
                onClicked: memoryScroll.importRequested()
            }
            ActionButton {
                text: "Refresh timings"
                onClicked: memoryScroll.bridge.refresh()
                enabled: memoryScroll.viewState.ready
            }
        }
        EmptyState {
            visible: memoryScroll.channel === null
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            title: "Memory timings are unavailable"
            description: memoryScroll.viewState.memoryError || "Waiting for a memory-controller capture."
        }
        Panel {
            visible: memoryScroll.channel !== null
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: 155
            ColumnLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 20
                RowLayout {
                    spacing: 12
                    Layout.fillWidth: true
                    Badge {
                        text: memoryScroll.channel ? memoryScroll.channel.type : ""
                    }
                    Text {
                        text: memoryScroll.channel ? memoryScroll.channel.speed : "N/A"
                        color: "#1d3144"
                        font.pixelSize: 42
                        font.weight: Font.DemiBold
                    }
                    Text {
                        text: "MT/s"
                        color: "#8796a6"
                        font.pixelSize: 18
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    ColumnLayout {
                        spacing: 4
                        Text {
                            text: "PRIMARY TIMINGS"
                            color: "#8b98a7"
                            font.pixelSize: 13
                            font.letterSpacing: 1.2
                        }
                        Text {
                            text: memoryScroll.channel ? memoryScroll.channel.primaries : "N/A"
                            color: "#30445a"
                            font.pixelSize: 29
                            font.weight: Font.DemiBold
                        }
                    }
                }
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 24
                    Repeater {
                        model: memoryScroll.channel ? [
                            {
                                name: "MCLK",
                                value: memoryScroll.channel.mclk + " MHz"
                            },
                            {
                                name: "Command rate",
                                value: memoryScroll.channel.commandRate
                            },
                            {
                                name: "Gear-down mode",
                                value: memoryScroll.channel.gdm
                            },
                            {
                                name: "Power down",
                                value: memoryScroll.channel.powerDown
                            },
                            {
                                name: "Nitro RX / TX / CTL",
                                value: memoryScroll.channel.nitro
                            }
                        ] : []
                        delegate: RowLayout {
                            id: dataDelegate4
                            required property var modelData
                            spacing: 7
                            Layout.fillWidth: true
                            Text {
                                text: dataDelegate4.modelData.name
                                color: "#8b98a7"
                                font.pixelSize: 14
                            }
                            Text {
                                text: dataDelegate4.modelData.value
                                color: "#3c5267"
                                font.pixelSize: 15
                                font.weight: Font.DemiBold
                            }
                        }
                    }
                }
            }
        }
        GridLayout {
            visible: memoryScroll.channel !== null
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            columns: 2
            columnSpacing: 16
            rowSpacing: 16
            Repeater {
                model: memoryScroll.channel ? memoryScroll.channel.groups : []
                delegate: Panel {
                    id: timingGroup
                    required property var modelData
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    implicitHeight: timingColumn.implicitHeight + 38
                    ColumnLayout {
                        id: timingColumn
                        anchors.top: parent.top
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.margins: 20
                        spacing: 0
                        RowLayout {
                            Layout.fillWidth: true
                            Layout.bottomMargin: 12
                            Text {
                                text: timingGroup.modelData.title
                                color: "#263a4e"
                                font.pixelSize: 18
                                font.weight: Font.DemiBold
                                Layout.fillWidth: true
                            }
                            Text {
                                text: "CYCLES"
                                color: "#98a3b0"
                                font.pixelSize: 11
                                font.letterSpacing: 1
                            }
                        }
                        Repeater {
                            model: timingGroup.modelData.rows
                            delegate: Item {
                                id: dataDelegate6
                                required property var modelData
                                Layout.fillWidth: true
                                implicitHeight: 30
                                Rectangle {
                                    anchors.bottom: parent.bottom
                                    width: parent.width
                                    height: 1
                                    color: "#f1f4f7"
                                }
                                Text {
                                    anchors.left: parent.left
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: dataDelegate6.modelData.name
                                    color: "#718096"
                                    font.pixelSize: 15
                                }
                                Text {
                                    anchors.right: parent.right
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: dataDelegate6.modelData.value
                                    color: dataDelegate6.modelData.value === "N/A" ? "#a2adba" : "#2d4359"
                                    font.family: "monospace"
                                    font.pixelSize: 17
                                    font.weight: Font.Medium
                                }
                            }
                        }
                    }
                }
            }
        }
        Panel {
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: firmwareColumn.implicitHeight + 40
            ColumnLayout {
                id: firmwareColumn
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: 20
                spacing: 14
                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "Firmware settings"
                        color: "#263a4e"
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                        Layout.fillWidth: true
                    }
                    Badge {
                        text: "Configured values"
                        tone: "#ad7e37"
                    }
                }
                Text {
                    text: memoryScroll.viewState.firmware.loaded ? memoryScroll.viewState.firmware.source : "Import the aod-getters.txt file produced by the audited AOD probe to see electrical settings."
                    color: "#8a97a7"
                    font.pixelSize: 15
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                Text {
                    visible: memoryScroll.viewState.firmware.loaded
                    text: "Electrical decoding is provisional. These are firmware settings, not live voltage measurements."
                    color: "#b18441"
                    font.pixelSize: 14
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                Text {
                    visible: memoryScroll.viewState.firmware.loaded
                    text: memoryScroll.viewState.firmware.metadataMatches ? "Board / BIOS / AML metadata matches the audited profile." : "No matching board / BIOS / AML metadata was supplied with this historical capture."
                    color: "#8a97a7"
                    font.pixelSize: 14
                    Layout.fillWidth: true
                    wrapMode: Text.Wrap
                }
                CheckBox {
                    visible: memoryScroll.viewState.firmware.loaded
                    text: "Show all " + memoryScroll.viewState.firmware.rows.length + " firmware objects"
                    checked: memoryScroll.showAllFirmware
                    onToggled: memoryScroll.showAllFirmware = checked
                    font.pixelSize: 15
                }
                Repeater {
                    model: memoryScroll.viewState.firmware.rows.filter(row => memoryScroll.showAllFirmware || row.electrical)
                    delegate: Rectangle {
                        id: fwRow
                        required property var modelData
                        Layout.fillWidth: true
                        implicitHeight: 37
                        color: "#f8fafc"
                        radius: 5
                        RowLayout {
                            anchors.fill: parent
                            anchors.leftMargin: 12
                            anchors.rightMargin: 12
                            spacing: 12
                            Text {
                                text: fwRow.modelData.name
                                color: "#6c7f93"
                                font.pixelSize: 15
                                Layout.fillWidth: true
                                Layout.preferredWidth: 220
                                elide: Text.ElideRight
                            }
                            Text {
                                text: fwRow.modelData.value
                                color: "#344e64"
                                font.pixelSize: 15
                                Layout.preferredWidth: 250
                            }
                            Text {
                                text: "raw " + fwRow.modelData.raw
                                color: "#8f9eaf"
                                font.pixelSize: 14
                                font.family: "monospace"
                                Layout.preferredWidth: 80
                            }
                            Text {
                                text: fwRow.modelData.id
                                color: "#9eabb9"
                                font.pixelSize: 14
                                font.family: "monospace"
                            }
                        }
                        HoverHandler {
                            id: fwHover
                        }
                        ToolTip.visible: fwHover.hovered
                        ToolTip.text: fwRow.modelData.name + "\n" + fwRow.modelData.note
                        ToolTip.delay: 500
                    }
                }
            }
        }
        ActionButton {
            visible: memoryScroll.channel !== null
            Layout.leftMargin: 30
            text: memoryScroll.showRegisters ? "Hide UMC registers" : "Inspect captured UMC registers"
            quiet: true
            onClicked: memoryScroll.showRegisters = !memoryScroll.showRegisters
        }
        Panel {
            visible: memoryScroll.showRegisters && memoryScroll.channel !== null
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: registerGrid.implicitHeight + 40
            GridLayout {
                id: registerGrid
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: 20
                columns: 3
                columnSpacing: 20
                rowSpacing: 12
                Repeater {
                    model: memoryScroll.channel ? memoryScroll.channel.registers : []
                    delegate: RowLayout {
                        id: dataDelegate8
                        required property var modelData
                        Layout.fillWidth: true
                        Text {
                            text: dataDelegate8.modelData.address
                            font.family: "monospace"
                            font.pixelSize: 15
                            color: "#8a97a7"
                            Layout.fillWidth: true
                        }
                        Text {
                            text: dataDelegate8.modelData.value
                            font.family: "monospace"
                            font.pixelSize: 15
                            color: dataDelegate8.modelData.changed ? "#b37d30" : "#3f566d"
                        }
                    }
                }
            }
        }
        ActionButton {
            Layout.leftMargin: 30
            text: memoryScroll.showCandidates ? "Hide experimental PM fields" : "Inspect experimental PM fields"
            quiet: true
            onClicked: memoryScroll.showCandidates = !memoryScroll.showCandidates
        }
        Panel {
            visible: memoryScroll.showCandidates
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: candidateColumn.implicitHeight + 40
            ColumnLayout {
                id: candidateColumn
                anchors.top: parent.top
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.margins: 20
                spacing: 12
                Text {
                    text: "Research observations"
                    color: "#3f566d"
                    font.pixelSize: 20
                    font.weight: Font.DemiBold
                }
                Text {
                    text: "Raw floating-point fields from the audited 0x620205 / 2452-byte table. Units and idle/filtering semantics remain unresolved; these values are not used as sensors."
                    color: "#b18441"
                    font.pixelSize: 15
                    wrapMode: Text.Wrap
                    Layout.fillWidth: true
                }
                Text {
                    visible: memoryScroll.viewState.candidates.length === 0
                    text: "No matching PM-table capture is available."
                    color: "#8a97a7"
                    font.pixelSize: 17
                }
                Repeater {
                    model: memoryScroll.viewState.candidates
                    delegate: RowLayout {
                        id: dataDelegate9
                        required property var modelData
                        Layout.fillWidth: true
                        Text {
                            text: dataDelegate9.modelData.offset
                            color: "#8a97a7"
                            font.family: "monospace"
                            Layout.preferredWidth: 80
                        }
                        Text {
                            text: dataDelegate9.modelData.label
                            color: "#718096"
                            Layout.fillWidth: true
                        }
                        Text {
                            text: dataDelegate9.modelData.value
                            color: "#3f566d"
                            font.family: "monospace"
                        }
                    }
                }
            }
        }
        Item {
            implicitHeight: 12
        }
    }
}
