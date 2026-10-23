pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts

Panel {
    id: chart
    property string title: ""
    property string unit: ""
    property string field: "power"
    property color accent: "#088674"
    property var samples: []
    property real latest: 0
    property bool available: false
    implicitHeight: 226
    onSamplesChanged: graph.requestPaint()
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 20
        spacing: 12
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: chart.title
                color: "#243246"
                font.pixelSize: 18
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
            Text {
                text: chart.available ? chart.latest.toFixed(1) + " " + chart.unit : "N/A"
                color: chart.accent
                font.pixelSize: 18
                font.weight: Font.DemiBold
            }
        }
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Canvas {
                id: graph
                anchors.fill: parent
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()
                onPaint: {
                    const ctx = getContext("2d");
                    ctx.reset();
                    let maximum = chart.field === "temperature" ? 100 : 50;
                    for (const sample of chart.samples)
                        if (sample[chart.field] !== null)
                            maximum = Math.max(maximum, sample[chart.field] * 1.15);
                    ctx.fillStyle = "#9ca9b7";
                    ctx.font = "13px sans-serif";
                    ctx.fillText(maximum.toFixed(0) + " " + chart.unit, 0, 11);
                    ctx.fillText("0", 0, height - 3);
                    ctx.lineWidth = 1;
                    ctx.strokeStyle = "#edf1f5";
                    for (let i = 0; i < 4; i++) {
                        ctx.beginPath();
                        ctx.moveTo(0, 4 + (height - 8) * i / 3);
                        ctx.lineTo(width, 4 + (height - 8) * i / 3);
                        ctx.stroke();
                    }
                    const count = chart.samples.length;
                    const capacity = Math.max(30, Math.min(120, count));
                    ctx.lineWidth = 2;
                    ctx.strokeStyle = chart.accent;
                    let open = false;
                    ctx.beginPath();
                    for (let i = 0; i < count; i++) {
                        const value = chart.samples[i][chart.field];
                        if (value === null) {
                            open = false;
                            continue;
                        }
                        const x = width * (capacity - count + i) / (capacity - 1);
                        const y = height - 5 - (height - 10) * Math.min(1, Math.max(0, value / maximum));
                        if (!open) {
                            ctx.moveTo(x, y);
                            open = true;
                        } else
                            ctx.lineTo(x, y);
                    }
                    ctx.stroke();
                }
            }
            Text {
                anchors.centerIn: parent
                visible: !chart.available
                text: "Waiting for this sensor"
                color: "#8a97a7"
                font.pixelSize: 17
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: "UP TO 2 MINUTES"
                color: "#8a97a7"
                font.pixelSize: 13
                font.letterSpacing: 1
            }
            Item {
                Layout.fillWidth: true
            }
            Text {
                text: "NOW"
                color: "#8a97a7"
                font.pixelSize: 13
                font.letterSpacing: 1
            }
        }
    }
}
