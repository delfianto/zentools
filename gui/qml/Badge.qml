pragma ComponentBehavior: Bound
import QtQuick

Rectangle {
    id: badge
    property string text: ""
    property color tone: "#087f71"
    implicitWidth: label.implicitWidth + 20
    implicitHeight: 26
    radius: 7
    color: Qt.rgba(tone.r, tone.g, tone.b, 0.09)
    Text {
        id: label
        anchors.centerIn: parent
        text: badge.text
        color: badge.tone
        font.pixelSize: 11
        font.weight: Font.DemiBold
    }
}
