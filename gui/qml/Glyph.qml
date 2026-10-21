pragma ComponentBehavior: Bound
import QtQuick

Canvas {
    id: glyph
    property int symbol: 0
    property color ink: "#9aaabd"
    implicitWidth: 22
    implicitHeight: 22
    onInkChanged: requestPaint()
    onSymbolChanged: requestPaint()
    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        ctx.strokeStyle = ink;
        ctx.lineWidth = 1.6;
        ctx.lineCap = "round";
        ctx.lineJoin = "round";
        const lines = symbol === 0 ? [[2, 12, 6, 12, 9, 5, 13, 18, 16, 10, 20, 10]] : symbol === 1 ? [[4, 6, 18, 6, 18, 16, 4, 16, 4, 6], [7, 3, 7, 6], [11, 3, 11, 6], [15, 3, 15, 6], [7, 16, 7, 19], [11, 16, 11, 19], [15, 16, 15, 19], [8, 10, 14, 10], [8, 13, 14, 13]] : symbol === 2 ? [[3, 6, 19, 6], [3, 11, 19, 11], [3, 16, 19, 16], [8, 3, 8, 9], [15, 8, 15, 14], [7, 13, 7, 19]] : [[3, 4, 10, 4, 10, 18, 3, 18, 3, 4], [14, 4, 20, 4, 20, 18, 14, 18, 14, 4]];
        for (const points of lines) {
            ctx.beginPath();
            ctx.moveTo(points[0], points[1]);
            for (let i = 2; i < points.length; i += 2)
                ctx.lineTo(points[i], points[i + 1]);
            ctx.stroke();
        }
    }
}
