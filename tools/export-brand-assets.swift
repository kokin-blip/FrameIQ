// Run from the repository: swift tools/export-brand-assets.swift
// Export the selected app icon to the fixed bitmap sizes required by NSIS.
import AppKit
import Foundation

let root = URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
let icons = root.appendingPathComponent("apps/desktop/src-tauri/icons")
guard let source = NSImage(contentsOf: icons.appendingPathComponent("icon.png")) else {
    fatalError("Generate the native app icons from public/brand/app-icon.svg first.")
}

func bitmap(width: Int, height: Int, background: NSColor, rectangle: NSRect, name: String) throws {
    guard let image = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: width * 4, bitsPerPixel: 32
    ), let context = NSGraphicsContext(bitmapImageRep: image) else {
        fatalError("Could not initialize the installer bitmap renderer.")
    }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = context
    context.imageInterpolation = .high
    background.setFill()
    NSRect(x: 0, y: 0, width: width, height: height).fill()
    source.draw(in: rectangle, from: .zero, operation: .sourceOver, fraction: 1)
    context.flushGraphics()
    NSGraphicsContext.restoreGraphicsState()
    // NSIS uses opaque bitmap resources. Encode RGB24 rather than an alpha DIB.
    guard let opaque = NSBitmapImageRep(
        bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
        bitsPerSample: 8, samplesPerPixel: 3, hasAlpha: false, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: width * 3, bitsPerPixel: 24
    ), let sourceBytes = image.bitmapData, let outputBytes = opaque.bitmapData else {
        fatalError("Could not initialize the RGB bitmap export.")
    }
    for y in 0..<height {
        for x in 0..<width {
            for channel in 0..<3 {
                outputBytes[y * opaque.bytesPerRow + x * 3 + channel] = sourceBytes[y * image.bytesPerRow + x * 4 + channel]
            }
        }
    }
    guard let bytes = opaque.representation(using: .bmp, properties: [:]) else {
        fatalError("Could not encode the installer bitmap.")
    }
    try bytes.write(to: icons.appendingPathComponent(name), options: .atomic)
    print("Exported \(name): \(width)×\(height)")
}

try bitmap(width: 150, height: 57, background: .white,
           rectangle: NSRect(x: 47, y: 0.5, width: 56, height: 56), name: "installer-header.bmp")
try bitmap(width: 164, height: 314, background: NSColor(srgbRed: 27/255, green: 27/255, blue: 27/255, alpha: 1),
           rectangle: NSRect(x: 10, y: 112, width: 144, height: 144), name: "installer-sidebar.bmp")
