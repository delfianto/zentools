pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import ZenTools

ScrollView {
    id: overviewScroll

    required property var viewState
    function timeText(value) {
        return value ? new Date(value).toLocaleTimeString(Qt.locale(), "HH:mm:ss") : "—";
    }
    clip: true
    contentWidth: availableWidth
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ColumnLayout {
        width: overviewScroll.availableWidth
        spacing: 18
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 26
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            ColumnLayout {
                Layout.fillWidth: true
                spacing: 6
                Text {
                    text: overviewScroll.viewState.cpu
                    color: "#263448"
                    font.pixelSize: 16
                    font.weight: Font.DemiBold
                }
                Text {
                    text: overviewScroll.viewState.codename + "  ·  " + overviewScroll.viewState.source
                    color: "#8090a3"
                    font.pixelSize: 10
                }
            }
            Text {
                text: "Updated " + overviewScroll.timeText(overviewScroll.viewState.timestamp)
                color: "#8a97a7"
                font.pixelSize: 10
            }
        }
        GridLayout {
            columns: 4
            columnSpacing: 14
            rowSpacing: 14
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            MetricCard {
                Layout.fillWidth: true
                title: "CPU temperature"
                reading: overviewScroll.viewState.metrics.temperature
                accent: "#d29249"
            }
            MetricCard {
                Layout.fillWidth: true
                title: "Package power"
                reading: overviewScroll.viewState.metrics.power
                accent: "#088674"
            }
            MetricCard {
                Layout.fillWidth: true
                title: "Peak core clock"
                reading: overviewScroll.viewState.metrics.frequency
                accent: "#6484bf"
            }
            MetricCard {
                Layout.fillWidth: true
                title: "CPU utilization"
                reading: overviewScroll.viewState.metrics.usage
                accent: "#8e76b3"
            }
        }
        EmptyState {
            visible: !overviewScroll.viewState.demo && !overviewScroll.viewState.elevated && overviewScroll.viewState.metrics.power.value === null
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: 104
            title: "More sensors are available with hardware access"
            description: "CPU clocks and usage work now. Authorize hardware access above to read temperature, package power and memory timings."
        }
        GridLayout {
            columns: 2
            columnSpacing: 16
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            HistoryChart {
                Layout.fillWidth: true
                title: "Package power"
                unit: "W"
                samples: overviewScroll.viewState.history
                field: "power"
                latest: overviewScroll.viewState.metrics.power.value || 0
                available: overviewScroll.viewState.metrics.power.value !== null
            }
            HistoryChart {
                Layout.fillWidth: true
                title: "CPU temperature"
                unit: "°C"
                samples: overviewScroll.viewState.history
                field: "temperature"
                accent: "#d29249"
                latest: overviewScroll.viewState.metrics.temperature.value || 0
                available: overviewScroll.viewState.metrics.temperature.value !== null
            }
        }
        Panel {
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: 84
            RowLayout {
                anchors.fill: parent
                anchors.margins: 18
                spacing: 22
                Repeater {
                    model: overviewScroll.viewState.clocks
                    delegate: ColumnLayout {
                        id: dataDelegate1
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 7
                        Text {
                            text: dataDelegate1.modelData.name
                            color: "#8491a3"
                            font.pixelSize: 10
                        }
                        Text {
                            text: dataDelegate1.modelData.value + (dataDelegate1.modelData.value === "N/A" ? "" : " " + dataDelegate1.modelData.unit)
                            color: "#324256"
                            font.pixelSize: 14
                            font.weight: Font.DemiBold
                        }
                    }
                }
            }
        }
        Panel {
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: coresColumn.implicitHeight + 40
            ColumnLayout {
                id: coresColumn
                anchors.fill: parent
                anchors.margins: 20
                spacing: 18
                RowLayout {
                    Layout.fillWidth: true
                    Text {
                        text: "Per-core telemetry"
                        color: "#243246"
                        font.pixelSize: 14
                        font.weight: Font.DemiBold
                        Layout.fillWidth: true
                    }
                    Text {
                        text: "SAMPLED CLOCKS · C0 ACTIVITY WHEN AVAILABLE"
                        color: "#8b98a7"
                        font.pixelSize: 8
                        font.letterSpacing: 1
                    }
                }
                GridLayout {
                    Layout.fillWidth: true
                    columns: 4
                    columnSpacing: 10
                    rowSpacing: 10
                    Repeater {
                        model: overviewScroll.viewState.cores
                        delegate: Rectangle {
                            id: coreTile
                            required property var modelData
                            Layout.fillWidth: true
                            implicitHeight: 79
                            radius: 8
                            color: "#f6f8fb"
                            border.color: "#edf0f4"
                            ColumnLayout {
                                anchors.fill: parent
                                anchors.margins: 12
                                spacing: 7
                                RowLayout {
                                    Layout.fillWidth: true
                                    Text {
                                        text: "C" + coreTile.modelData.id.toString().padStart(2, "0")
                                        color: "#8a98a9"
                                        font.pixelSize: 10
                                        Layout.fillWidth: true
                                    }
                                    Text {
                                        text: coreTile.modelData.clock + (coreTile.modelData.clock === "N/A" ? "" : " MHz")
                                        color: "#2c4258"
                                        font.pixelSize: 12
                                        font.weight: Font.DemiBold
                                    }
                                }
                                Text {
                                    text: coreTile.modelData.temperature + " °C   /   " + coreTile.modelData.power + " W"
                                    color: "#8a97a7"
                                    font.pixelSize: 9
                                }
                                Rectangle {
                                    Layout.fillWidth: true
                                    implicitHeight: 3
                                    radius: 2
                                    color: "#e6ebf1"
                                    Rectangle {
                                        height: parent.height
                                        width: coreTile.modelData.activity === null ? 0 : parent.width * Math.min(1, Math.max(0, coreTile.modelData.activity / 100))
                                        radius: 2
                                        color: "#65bea7"
                                    }
                                }
                            }
                        }
                    }
                }
                Text {
                    visible: overviewScroll.viewState.cores.length === 0
                    text: "Per-core readings are not available on this system."
                    color: "#8997a8"
                    font.pixelSize: 12
                }
            }
        }
        Panel {
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: 106
            RowLayout {
                anchors.fill: parent
                anchors.margins: 20
                spacing: 30
                Repeater {
                    model: overviewScroll.viewState.limits
                    delegate: ColumnLayout {
                        id: dataDelegate3
                        required property var modelData
                        Layout.fillWidth: true
                        spacing: 9
                        RowLayout {
                            Text {
                                text: dataDelegate3.modelData.name
                                color: "#718096"
                                font.pixelSize: 11
                                Layout.fillWidth: true
                            }
                            Text {
                                text: dataDelegate3.modelData.value + " / " + dataDelegate3.modelData.limit + " " + dataDelegate3.modelData.unit
                                color: "#52667c"
                                font.pixelSize: 11
                            }
                        }
                        Rectangle {
                            Layout.fillWidth: true
                            implicitHeight: 4
                            radius: 2
                            color: "#edf1f5"
                            Rectangle {
                                height: 4
                                width: Number.isFinite(Number(dataDelegate3.modelData.value)) && Number(dataDelegate3.modelData.limit) > 0 ? parent.width * Math.min(1, Math.max(0, Number(dataDelegate3.modelData.value) / Number(dataDelegate3.modelData.limit))) : 0
                                radius: 2
                                color: "#80bba9"
                            }
                        }
                        Text {
                            text: "Reported consumption / limit"
                            color: "#9aa5b3"
                            font.pixelSize: 9
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
