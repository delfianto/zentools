pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts

Panel {
    id: card
    property string title: ""
    property var reading: ({
            display: "N/A",
            unit: "",
            value: null,
            detail: ""
        })
    property color accent: "#088674"
    implicitHeight: 139
    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 18
        spacing: 9
        RowLayout {
            Layout.fillWidth: true
            Text {
                text: card.title
                color: "#526278"
                font.pixelSize: 17
                font.weight: Font.Medium
                Layout.fillWidth: true
            }
            Rectangle {
                implicitWidth: 6
                implicitHeight: 6
                radius: 3
                color: card.reading.value === null ? "#b2bdca" : card.accent
            }
        }
        RowLayout {
            spacing: 7
            Text {
                text: card.reading.display
                color: "#152235"
                font.pixelSize: 45
                font.weight: Font.DemiBold
                font.family: "Inter"
            }
            Text {
                text: card.reading.value === null ? "" : card.reading.unit
                color: "#708097"
                font.pixelSize: 21
                Layout.alignment: Qt.AlignBottom
                Layout.bottomMargin: 4
            }
            Item {
                Layout.fillWidth: true
            }
        }
        Text {
            text: card.reading.detail
            color: "#7c8999"
            font.pixelSize: 14
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }
}
