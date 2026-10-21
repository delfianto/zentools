pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import ZenTools

ScrollView {
    id: profilesScroll

    required property var viewState
    required property Backend bridge
    property int selectedProfile: -1
    clip: true
    contentWidth: availableWidth
    ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
    ColumnLayout {
        width: profilesScroll.availableWidth
        spacing: 20
        RowLayout {
            Layout.topMargin: 30
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            Layout.fillWidth: true
            ColumnLayout {
                spacing: 7
                Layout.fillWidth: true
                Text {
                    text: "Energy performance preference"
                    color: "#263a4e"
                    font.pixelSize: 17
                    font.weight: Font.DemiBold
                }
                Text {
                    text: "Apply one preference to every CPU. Changes take effect immediately after Apply."
                    color: "#8492a4"
                    font.pixelSize: 11
                }
            }
            Badge {
                text: profilesScroll.viewState.eppAvailable ? profilesScroll.viewState.activeProfile === "mixed" ? "Mixed preferences" : "EPP available" : "EPP unavailable"
                tone: profilesScroll.viewState.eppAvailable ? "#087f71" : "#8a97a7"
            }
        }
        GridLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            columns: 2
            columnSpacing: 16
            rowSpacing: 16
            Repeater {
                model: profilesScroll.viewState.profiles
                delegate: Panel {
                    id: profileCard
                    required property var modelData
                    Layout.fillWidth: true
                    implicitHeight: 186
                    border.color: profilesScroll.selectedProfile === profileCard.modelData.level ? "#088674" : profileCard.modelData.active ? "#9cd4c4" : "#e2e8ef"
                    border.width: profilesScroll.selectedProfile === profileCard.modelData.level ? 2 : 1
                    color: profileHover.hovered || profilesScroll.selectedProfile === profileCard.modelData.level ? "#f6fbf9" : "white"
                    HoverHandler {
                        id: profileHover
                    }
                    TapHandler {
                        onTapped: profilesScroll.selectedProfile = profileCard.modelData.level
                    }
                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 22
                        spacing: 12
                        RowLayout {
                            Layout.fillWidth: true
                            Rectangle {
                                implicitWidth: 32
                                implicitHeight: 32
                                radius: 9
                                color: "#e9f4ef"
                                Text {
                                    anchors.centerIn: parent
                                    text: String(profileCard.modelData.level + 1)
                                    color: "#498d7c"
                                    font.pixelSize: 14
                                    font.weight: Font.DemiBold
                                }
                            }
                            Item {
                                Layout.fillWidth: true
                            }
                            Badge {
                                visible: profileCard.modelData.active
                                text: "Current"
                            }
                            RadioButton {
                                checked: profilesScroll.selectedProfile === profileCard.modelData.level
                                onClicked: profilesScroll.selectedProfile = profileCard.modelData.level
                                Accessible.name: "Select " + profileCard.modelData.title
                            }
                        }
                        Text {
                            text: profileCard.modelData.title
                            color: "#2d4357"
                            font.pixelSize: 16
                            font.weight: Font.DemiBold
                        }
                        Text {
                            text: profileCard.modelData.description
                            color: "#8391a2"
                            font.pixelSize: 11
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                        }
                    }
                }
            }
        }
        Panel {
            Layout.fillWidth: true
            Layout.leftMargin: 30
            Layout.rightMargin: 30
            implicitHeight: 102
            RowLayout {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 20
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 7
                    Text {
                        text: profilesScroll.selectedProfile < 0 ? "Choose a profile above" : profilesScroll.viewState.profiles[profilesScroll.selectedProfile].title
                        color: "#324a60"
                        font.pixelSize: 14
                        font.weight: Font.DemiBold
                    }
                    Text {
                        text: profilesScroll.viewState.demo ? "Preview mode · profile changes are disabled" : !profilesScroll.viewState.eppAvailable ? (profilesScroll.viewState.eppError || "The CPU driver does not expose EPP preferences.") : "Writes are verified on every CPU. A failed change attempts to restore the previous values."
                        color: "#8a97a7"
                        font.pixelSize: 10
                        wrapMode: Text.Wrap
                        Layout.fillWidth: true
                    }
                }
                ActionButton {
                    primary: true
                    text: profilesScroll.viewState.elevated ? "Apply profile" : "Authorize changes"
                    enabled: profilesScroll.selectedProfile >= 0 && profilesScroll.viewState.eppAvailable && !profilesScroll.viewState.demo && !profilesScroll.viewState.authorizing
                    onClicked: profilesScroll.viewState.elevated ? profilesScroll.bridge.applyProfile(profilesScroll.selectedProfile) : profilesScroll.bridge.connectHardware()
                }
            }
        }
        Text {
            Layout.fillWidth: true
            Layout.leftMargin: 32
            Layout.rightMargin: 30
            text: "EPP controls CPU performance preference. Memory timings and firmware electrical settings are shown as read-only values. Other power-management tools may update EPP independently."
            color: "#91a0b0"
            font.pixelSize: 11
            wrapMode: Text.Wrap
        }
        Item {
            implicitHeight: 20
        }
    }
}
