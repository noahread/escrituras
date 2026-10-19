// Describe a screenshot as text, so the app's layout can be reviewed from a
// CI log: the Escrituras window's frame, then every line of text Vision
// recognizes, with its position (x, y from the top-left, width), top to bottom.
//
// Usage: swift scripts/ci-describe-screen.swift screenshot.png
// macOS only. Run while the app is open so its window can be listed.

import AppKit
import Foundation
import Vision

guard CommandLine.arguments.count > 1,
      let image = NSImage(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1])),
      let cgImage = image.cgImage(forProposedRect: nil, context: nil, hints: nil)
else {
    print("usage: ci-describe-screen.swift <screenshot.png>")
    exit(1)
}

let width = Double(cgImage.width)
let height = Double(cgImage.height)
print("screen \(Int(width))x\(Int(height)) pixels")

// Window frames are in points; screenshots may be at 2x on Retina displays
let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
    as? [[String: Any]] ?? []
for window in windows where (window[kCGWindowOwnerName as String] as? String) == "Escrituras" {
    if let bounds = window[kCGWindowBounds as String] as? [String: Double] {
        let layer = window[kCGWindowLayer as String] as? Int ?? 0
        print(String(
            format: "window x=%.0f y=%.0f w=%.0f h=%.0f layer=%d (points)",
            bounds["X"] ?? 0, bounds["Y"] ?? 0, bounds["Width"] ?? 0, bounds["Height"] ?? 0, layer
        ))
    }
}

let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = false
do {
    try VNImageRequestHandler(cgImage: cgImage).perform([request])
} catch {
    print("text recognition failed: \(error)")
    exit(1)
}

struct Line {
    let x: Double, y: Double, w: Double, text: String
}

let lines = (request.results ?? []).compactMap { observation -> Line? in
    guard let text = observation.topCandidates(1).first?.string else { return nil }
    let box = observation.boundingBox // normalized, origin bottom-left
    return Line(x: box.minX * width, y: (1 - box.maxY) * height, w: box.width * width, text: text)
}
.sorted { ($0.y.rounded(), $0.x) < ($1.y.rounded(), $1.x) }

print("text (x y width: text)")
for line in lines {
    print(String(format: "%5.0f %5.0f %5.0f: %@", line.x, line.y, line.w, line.text))
}
