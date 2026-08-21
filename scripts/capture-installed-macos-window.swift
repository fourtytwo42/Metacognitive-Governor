import CoreGraphics
import Foundation

enum CaptureError: Error, CustomStringConvertible {
    case failed(String)

    var description: String {
        switch self {
        case .failed(let message): return message
        }
    }
}

func run() throws {
    guard CommandLine.arguments.count == 3, let processId = Int32(CommandLine.arguments[1]) else {
        throw CaptureError.failed("Usage: capture-installed-macos-window.swift PROCESS_ID OUTPUT_PATH")
    }
    let output = URL(fileURLWithPath: CommandLine.arguments[2]).standardizedFileURL
    guard output.isFileURL else {
        throw CaptureError.failed("The screenshot output must be a local file path.")
    }

    guard let windowInfo = CGWindowListCopyWindowInfo(
        [.optionOnScreenOnly, .excludeDesktopElements],
        kCGNullWindowID
    ) as? [[String: Any]] else {
        throw CaptureError.failed("macOS did not return an on-screen window list.")
    }

    let candidates: [(id: CGWindowID, area: CGFloat)] = windowInfo.compactMap { window in
        guard (window[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value == processId,
              (window[kCGWindowLayer as String] as? NSNumber)?.intValue == 0,
              let number = (window[kCGWindowNumber as String] as? NSNumber)?.uint32Value,
              let bounds = window[kCGWindowBounds as String] as? NSDictionary,
              let rectangle = CGRect(dictionaryRepresentation: bounds as CFDictionary),
              rectangle.width >= 760,
              rectangle.height >= 540 else {
            return nil
        }
        return (CGWindowID(number), rectangle.width * rectangle.height)
    }
    guard let selected = candidates.max(by: { $0.area < $1.area }) else {
        throw CaptureError.failed("No qualifying on-screen MCG window belongs to process \(processId).")
    }

    try FileManager.default.createDirectory(
        at: output.deletingLastPathComponent(),
        withIntermediateDirectories: true
    )
    let capture = Process()
    capture.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
    capture.arguments = ["-x", "-l", String(selected.id), output.path]
    try capture.run()
    capture.waitUntilExit()
    guard capture.terminationReason == .exit, capture.terminationStatus == 0 else {
        throw CaptureError.failed("screencapture failed with status \(capture.terminationStatus).")
    }
    let attributes = try FileManager.default.attributesOfItem(atPath: output.path)
    guard (attributes[.size] as? NSNumber)?.uint64Value ?? 0 > 0 else {
        throw CaptureError.failed("The captured MCG window image is empty.")
    }
    print("Captured MCG window \(selected.id): \(output.path)")
}

do {
    try run()
} catch {
    FileHandle.standardError.write(Data("\(error)\n".utf8))
    exit(1)
}
