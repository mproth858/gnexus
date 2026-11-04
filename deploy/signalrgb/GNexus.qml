import QtQuick 2.15
import QtQuick.Controls 2.15
Item {
    Column {
        width: parent.width
        spacing: 12
        Label { text: "G Nexus Lighting Bridge"; font.pixelSize: 24 }
        Label {
            width: parent.width
            wrapMode: Text.WordWrap
            text: "Enable the bridge in G Nexus Settings → Lighting. Place the G Nexus devices on your SignalRGB layout. Disable their original physical USB devices in SignalRGB to prevent conflicts. G Nexus chooses between the canvas and your saved colors."
        }
    }
}
