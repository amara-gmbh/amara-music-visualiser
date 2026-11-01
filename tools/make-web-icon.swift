import AppKit
import Foundation

guard CommandLine.arguments.count == 5 else {
    FileHandle.standardError.write(Data("usage: make-web-icon.swift background.png mark.svg iconset-dir source.png\n".utf8))
    exit(2)
}

let backgroundSource = URL(fileURLWithPath: CommandLine.arguments[1])
let markSource = URL(fileURLWithPath: CommandLine.arguments[2])
let destination = URL(fileURLWithPath: CommandLine.arguments[3], isDirectory: true)
let sourcePNG = URL(fileURLWithPath: CommandLine.arguments[4])
guard let background = NSImage(contentsOf: backgroundSource) else {
    FileHandle.standardError.write(Data("could not load icon background: \(backgroundSource.path)\n".utf8))
    exit(1)
}
guard let mark = NSImage(contentsOf: markSource) else {
    FileHandle.standardError.write(Data("could not load official mark: \(markSource.path)\n".utf8))
    exit(1)
}
try FileManager.default.createDirectory(at: destination, withIntermediateDirectories: true)

func render(_ pixels: Int, to path: URL) throws {
    guard let bitmap = NSBitmapImageRep(
        bitmapDataPlanes: nil,
        pixelsWide: pixels,
        pixelsHigh: pixels,
        bitsPerSample: 8,
        samplesPerPixel: 4,
        hasAlpha: true,
        isPlanar: false,
        colorSpaceName: .deviceRGB,
        bytesPerRow: 0,
        bitsPerPixel: 0
    ) else { throw CocoaError(.fileWriteUnknown) }
    bitmap.size = NSSize(width: pixels, height: pixels)
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)
    NSGraphicsContext.current?.imageInterpolation = .high
    background.draw(
        in: NSRect(x: 0, y: 0, width: pixels, height: pixels),
        from: .zero,
        operation: .copy,
        fraction: 1,
        respectFlipped: true,
        hints: [.interpolation: NSImageInterpolation.high]
    )
    mark.draw(
        in: NSRect(x: 0, y: 0, width: pixels, height: pixels),
        from: .zero,
        operation: .sourceOver,
        fraction: 1,
        respectFlipped: true,
        hints: [.interpolation: NSImageInterpolation.high]
    )
    NSGraphicsContext.restoreGraphicsState()
    guard let png = bitmap.representation(using: .png, properties: [:]) else {
        throw CocoaError(.fileWriteUnknown)
    }
    try png.write(to: path, options: .atomic)
}

for size in [16, 32, 128, 256, 512] {
    try render(size, to: destination.appendingPathComponent("icon_\(size)x\(size).png"))
    try render(size * 2, to: destination.appendingPathComponent("icon_\(size)x\(size)@2x.png"))
}
try render(1_024, to: sourcePNG)

