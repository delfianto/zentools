pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Layouts

Panel {
    id: empty
    property string title: ""
    property string description: ""
    implicitHeight: 170
    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(parent.width - 48, 550)
        spacing: 12
        Text {
            text: empty.title
            color: "#263448"
            font.pixelSize: 22
            font.weight: Font.DemiBold
            Layout.alignment: Qt.AlignHCenter
        }
        Text {
            text: empty.description
            color: "#718096"
            font.pixelSize: 17
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
    }
}
