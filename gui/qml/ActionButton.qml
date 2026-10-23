pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls

Button {
    id: button
    property bool primary: false
    property bool quiet: false
    implicitHeight: 38
    leftPadding: 15
    rightPadding: 15
    font.pixelSize: 17
    font.weight: Font.DemiBold
    hoverEnabled: true
    contentItem: Text {
        text: button.text
        color: !button.enabled ? "#9aa5b3" : button.primary ? "white" : "#263448"
        font: button.font
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
    }
    background: Rectangle {
        radius: 8
        color: !button.enabled ? "#e9edf2" : button.primary ? (button.down ? "#056254" : button.hovered ? "#087667" : "#088674") : (button.hovered ? "#e9eef3" : button.quiet ? "transparent" : "white")
        border.color: button.primary || button.quiet ? "transparent" : "#d8e0e9"
        border.width: button.activeFocus ? 2 : 1
    }
    ToolTip.visible: hovered && ToolTip.text.length > 0
    ToolTip.delay: 600
}
