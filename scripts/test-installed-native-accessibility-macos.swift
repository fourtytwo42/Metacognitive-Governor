import ApplicationServices
import Foundation

private let canary = "MCG_NATIVE_SECRET_CANARY"
private let timeoutSeconds: TimeInterval = 15
private let maximumNodes = 2_000

private struct Snapshot {
    let element: AXUIElement
    let role: String
    let subrole: String
    let name: String
    let focusable: Bool
    let enabled: Bool
}

private enum QualificationError: Error, CustomStringConvertible {
    case failed(String)

    var description: String {
        switch self {
        case .failed(let message): return message
        }
    }
}

private func copyAttribute(_ element: AXUIElement, _ attribute: CFString) -> CFTypeRef? {
    var value: CFTypeRef?
    guard AXUIElementCopyAttributeValue(element, attribute, &value) == .success else { return nil }
    return value
}

private func stringAttribute(_ element: AXUIElement, _ attribute: CFString) -> String {
    (copyAttribute(element, attribute) as? String) ?? ""
}

private func boolAttribute(_ element: AXUIElement, _ attribute: CFString) -> Bool {
    (copyAttribute(element, attribute) as? Bool) ?? false
}

private func isFocusable(_ element: AXUIElement) -> Bool {
    var settable = DarwinBoolean(false)
    if AXUIElementIsAttributeSettable(element, kAXFocusedAttribute as CFString, &settable) == .success {
        return settable.boolValue
    }
    return false
}

private func accessibleName(_ element: AXUIElement) -> String {
    for attribute in [kAXTitleAttribute, kAXDescriptionAttribute, kAXHelpAttribute] {
        let value = stringAttribute(element, attribute as CFString).trimmingCharacters(in: .whitespacesAndNewlines)
        if !value.isEmpty { return value }
    }
    return ""
}

private func collectSnapshots(_ root: AXUIElement) throws -> [Snapshot] {
    var snapshots: [Snapshot] = []
    var pending: [AXUIElement] = [root]
    var index = 0
    while index < pending.count {
        if pending.count > maximumNodes {
            throw QualificationError.failed("macOS AX tree exceeded the \(maximumNodes)-node safety bound.")
        }
        let element = pending[index]
        index += 1
        snapshots.append(Snapshot(
            element: element,
            role: stringAttribute(element, kAXRoleAttribute as CFString),
            subrole: stringAttribute(element, kAXSubroleAttribute as CFString),
            name: accessibleName(element),
            focusable: isFocusable(element),
            enabled: boolAttribute(element, kAXEnabledAttribute as CFString)
        ))
        if let children = copyAttribute(element, kAXChildrenAttribute as CFString) as? [AXUIElement] {
            pending.append(contentsOf: children)
        }
    }
    return snapshots
}

private func waitForSnapshots(
    _ root: AXUIElement,
    _ predicate: ([Snapshot]) -> Bool
) throws -> [Snapshot] {
    let deadline = Date().addingTimeInterval(timeoutSeconds)
    repeat {
        let snapshots = try collectSnapshots(root)
        if predicate(snapshots) { return snapshots }
        Thread.sleep(forTimeInterval: 0.1)
    } while Date() < deadline
    return try collectSnapshots(root)
}

private func find(
    _ root: AXUIElement,
    names: Set<String>,
    roles: Set<String>
) throws -> Snapshot {
    let snapshots = try waitForSnapshots(root) { values in
        values.contains { names.contains($0.name) && (roles.isEmpty || roles.contains($0.role)) }
    }
    guard let match = snapshots.first(where: {
        names.contains($0.name) && (roles.isEmpty || roles.contains($0.role))
    }) else {
        let hints = snapshots.filter { $0.name.localizedCaseInsensitiveContains("mode") }
            .prefix(12)
            .map { "\($0.role):\($0.name)" }
            .joined(separator: ", ")
        let suffix = hints.isEmpty ? "" : " AX mode candidates: \(hints)."
        throw QualificationError.failed("macOS AX did not expose \(names.sorted().joined(separator: " or ")).\(suffix)")
    }
    return match
}

private func setFocused(_ element: AXUIElement) throws {
    let result = AXUIElementSetAttributeValue(element, kAXFocusedAttribute as CFString, kCFBooleanTrue)
    guard result == .success else {
        throw QualificationError.failed("macOS AX could not focus the required control (error \(result.rawValue)).")
    }
}

private func waitForFocus(_ element: AXUIElement, _ name: String) throws {
    let deadline = Date().addingTimeInterval(timeoutSeconds)
    repeat {
        if boolAttribute(element, kAXFocusedAttribute as CFString) { return }
        Thread.sleep(forTimeInterval: 0.05)
    } while Date() < deadline
    throw QualificationError.failed("Native keyboard focus did not reach '\(name)'.")
}

private func postKey(_ virtualKey: CGKeyCode) {
    let source = CGEventSource(stateID: .hidSystemState)
    CGEvent(keyboardEventSource: source, virtualKey: virtualKey, keyDown: true)?.post(tap: .cghidEventTap)
    CGEvent(keyboardEventSource: source, virtualKey: virtualKey, keyDown: false)?.post(tap: .cghidEventTap)
}

private func typeText(_ text: String) {
    let utf16 = Array(text.utf16)
    let source = CGEventSource(stateID: .hidSystemState)
    let down = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true)
    utf16.withUnsafeBufferPointer { buffer in
        down?.keyboardSetUnicodeString(stringLength: buffer.count, unicodeString: buffer.baseAddress)
    }
    down?.post(tap: .cghidEventTap)
    CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false)?.post(tap: .cghidEventTap)
}

private func press(_ element: AXUIElement, _ name: String) throws {
    let result = AXUIElementPerformAction(element, kAXPressAction as CFString)
    guard result == .success else {
        throw QualificationError.failed("macOS AX exposed no press action for '\(name)' (error \(result.rawValue)).")
    }
}

private func run() throws {
    guard CommandLine.arguments.count == 3, let processId = pid_t(CommandLine.arguments[1]) else {
        throw QualificationError.failed("Usage: test-installed-native-accessibility-macos.swift PROCESS_ID OUTPUT_PATH")
    }
    guard AXIsProcessTrusted() else {
        throw QualificationError.failed("The macOS AX verifier is not trusted; run package UAT from the authorized Terminal session.")
    }

    let outputPath = URL(fileURLWithPath: CommandLine.arguments[2]).standardizedFileURL
    let app = AXUIElementCreateApplication(processId)
    let initial = try waitForSnapshots(app) { values in
        values.contains { $0.role == kAXWindowRole as String && $0.name == "MCG" }
            && values.contains { $0.role == "AXWebArea" }
            && values.contains { $0.name == "Primary navigation" }
            && values.contains { $0.name == "Settings" }
    }
    guard initial.contains(where: { $0.role == kAXWindowRole as String && $0.name == "MCG" }) else {
        throw QualificationError.failed("macOS AX did not expose the named MCG window.")
    }
    guard initial.contains(where: { $0.role == "AXWebArea" }) else {
        throw QualificationError.failed("macOS AX did not expose the web document.")
    }
    guard initial.contains(where: { $0.name == "Primary navigation" }) else {
        throw QualificationError.failed("macOS AX did not expose Primary navigation.")
    }
    guard initial.contains(where: { $0.name == "Settings" }) else {
        throw QualificationError.failed("macOS AX did not expose Settings.")
    }

    let goal = try find(app, names: ["Goal"], roles: [kAXTextAreaRole as String, kAXTextFieldRole as String])
    let openFolder = try find(app, names: ["Open folder"], roles: [kAXButtonRole as String])
    let createTask = try find(app, names: ["Create task"], roles: [kAXButtonRole as String])
    let openAI = try find(app, names: ["Continue with OpenAI", "Connect with OpenAI"], roles: [kAXButtonRole as String])
    let addProvider = try find(app, names: ["Add API or local provider"], roles: [kAXButtonRole as String])
    for required in [goal, openFolder, createTask, openAI, addProvider] {
        guard required.focusable && required.enabled else {
            throw QualificationError.failed("Required native control '\(required.name)' is not focusable and enabled.")
        }
    }

    let interactiveRoles: Set<String> = [
        kAXButtonRole as String, kAXCheckBoxRole as String, kAXComboBoxRole as String,
        "AXLink", kAXRadioButtonRole as String, kAXSliderRole as String,
        kAXTextAreaRole as String, kAXTextFieldRole as String
    ]
    let unnamed = Set(initial.filter {
        $0.focusable && interactiveRoles.contains($0.role) && $0.name.isEmpty
    }.map { $0.role })
    guard unnamed.isEmpty else {
        throw QualificationError.failed("macOS AX exposed unnamed focusable controls: \(unnamed.sorted().joined(separator: ", ")).")
    }

    try setFocused(goal.element)
    try waitForFocus(goal.element, "Goal")
    postKey(0x30)
    try waitForFocus(openFolder.element, "Open folder")
    postKey(0x30)
    try waitForFocus(createTask.element, "Create task")

    let modeRoles: Set<String> = [kAXButtonRole as String, kAXCheckBoxRole as String]
    let advancedMode = try find(app, names: ["Advanced mode"], roles: modeRoles)
    try press(advancedMode.element, advancedMode.name)
    _ = try find(app, names: ["Simple mode"], roles: modeRoles)
    let refreshedProvider = try find(app, names: ["Add API or local provider"], roles: [kAXButtonRole as String])
    try press(refreshedProvider.element, refreshedProvider.name)
    let password = try find(app, names: ["API key"], roles: [kAXTextFieldRole as String])
    guard password.focusable && password.subrole == kAXSecureTextFieldSubrole as String else {
        throw QualificationError.failed("The API key control is not a focusable AX secure text field.")
    }
    try setFocused(password.element)
    typeText(canary)
    Thread.sleep(forTimeInterval: 0.1)

    let accessibleValue = stringAttribute(password.element, kAXValueAttribute as CFString)
    guard accessibleValue != canary else {
        throw QualificationError.failed("macOS AX exposed the API-key cleartext.")
    }
    let finalSnapshots = try collectSnapshots(app)
    guard !finalSnapshots.contains(where: { $0.name.contains(canary) }) else {
        throw QualificationError.failed("The API-key cleartext leaked into an accessible name.")
    }

    let report: [String: Any] = [
        "schema_version": 1,
        "platform": "macos",
        "backend": "ax",
        "application_exposed": true,
        "document_exposed": true,
        "primary_navigation_exposed": true,
        "required_controls_named": true,
        "required_controls_focusable": true,
        "unnamed_focusable_control_roles": [],
        "keyboard_focus_sequence": ["Goal", "Open folder", "Create task"],
        "simple_to_advanced_native_input": true,
        "credential_control_is_password": true,
        "credential_accessible_value_redacted": accessibleValue != canary,
        "node_count": finalSnapshots.count
    ]
    let bytes = try JSONSerialization.data(withJSONObject: report, options: [.sortedKeys])
    try FileManager.default.createDirectory(at: outputPath.deletingLastPathComponent(), withIntermediateDirectories: true)
    try bytes.write(to: outputPath, options: .atomic)
    print("macOS native accessibility qualification passed: \(outputPath.path)")
}

do {
    try run()
} catch {
    FileHandle.standardError.write(Data("\(error)\n".utf8))
    exit(1)
}
