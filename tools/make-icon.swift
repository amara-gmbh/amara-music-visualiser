// Original vector artwork rendered locally at every macOS icon size.
import AppKit
import Foundation

let destination = URL(fileURLWithPath: CommandLine.arguments[1])
try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)

func render(_ pixels: Int, to path: URL) throws {
    let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: pixels, pixelsHigh: pixels,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true,
        isPlanar: false, colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0
    )!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    let context = NSGraphicsContext.current!.cgContext
    context.scaleBy(x: CGFloat(pixels) / 1024, y: CGFloat(pixels) / 1024)
    let bounds = NSRect(x: 56, y: 56, width: 912, height: 912)
    let background = NSBezierPath(roundedRect: bounds, xRadius: 208, yRadius: 208)
    NSGradient(colors: [
        NSColor(srgbRed: 0.03, green: 0.025, blue: 0.10, alpha: 1),
        NSColor(srgbRed: 0.13, green: 0.045, blue: 0.24, alpha: 1)
    ])!.draw(in: background, angle: -45)
    background.addClip()
    let centre = NSPoint(x: 512, y: 512)
    for ring in stride(from: 10, through: 0, by: -1) {
        let radius = CGFloat(180 + ring * 24)
        let path = NSBezierPath(ovalIn: NSRect(
            x: centre.x - radius, y: centre.y - radius * 0.78,
            width: radius * 2, height: radius * 1.56
        ))
        let transform = AffineTransform(rotationByDegrees: 24)
        var shifted = AffineTransform(translationByX: -512, byY: -512)
        path.transform(using: shifted)
        path.transform(using: transform)
        shifted = AffineTransform(translationByX: 512, byY: 512)
        path.transform(using: shifted)
        let hue = CGFloat(0.5 + Double(ring) * 0.029)
        NSColor(calibratedHue: hue, saturation: 0.72, brightness: 1, alpha: 0.12).setStroke()
        path.lineWidth = 34
        path.stroke()
        NSColor(calibratedHue: hue, saturation: 0.62, brightness: 1, alpha: 0.8 - CGFloat(ring) * 0.045).setStroke()
        path.lineWidth = 6
        path.stroke()
    }
    let wave = NSBezierPath()
    wave.move(to: NSPoint(x: 212, y: 512))
    for x in stride(from: 214, through: 812, by: 2) {
        let t = Double(x - 512) / 300
        let envelope = exp(-t * t * 7)
        let y = 512 + sin(t * Double.pi * 5) * envelope * 105
        wave.line(to: NSPoint(x: Double(x), y: y))
    }
    NSColor(srgbRed: 0.91, green: 0.86, blue: 1, alpha: 0.95).setStroke()
    wave.lineCapStyle = .round
    wave.lineJoinStyle = .round
    wave.lineWidth = 13
    wave.stroke()
    NSGraphicsContext.restoreGraphicsState()
    try bitmap.representation(using: .png, properties: [:])!.write(to: path)
}

for size in [16, 32, 128, 256, 512] {
    try render(size, to: destination.appendingPathComponent("icon_\(size)x\(size).png"))
    try render(size * 2, to: destination.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
