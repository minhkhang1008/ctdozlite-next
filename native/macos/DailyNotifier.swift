import AppKit
import UserNotifications

let bundleID = "com.ctdoteam1110.ctdozlite.notifications"
struct Payload: Decodable {
    let title: String
    let body: String
    let id: String
    let sound: Bool
    let attachment: String?
    func validate() -> Bool {
        // Sender/group title and message preview are shown by owner request. The Rust caller
        // flattens control characters and caps length; re-check those bounds here so a
        // malformed caller cannot post multi-line or oversized banners. The id stays opaque.
        func boundedText(_ text: String, _ limit: Int) -> Bool {
            !text.isEmpty && text.count <= limit && !text.unicodeScalars.contains { $0.properties.generalCategory == .control }
        }
        let attachmentValid = attachment.map {
            $0.count <= 1024 && $0.first == "/" && !$0.unicodeScalars.contains { $0.properties.generalCategory == .control }
        } ?? true
        return boundedText(title, 120) && boundedText(body, 240) && id.count == 64 && id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) } && attachmentValid
    }
}
func statusName(_ status: UNAuthorizationStatus) -> String {
    switch status {
    case .notDetermined: return "notDetermined"
    case .denied: return "denied"
    case .authorized: return "authorized"
    case .provisional: return "provisional"
    @unknown default: return "unavailable"
    }
}
func authorized(_ status: UNAuthorizationStatus) -> Bool { status == .authorized || status == .provisional }

final class NotificationDelegate: NSObject, UNUserNotificationCenterDelegate {
    func userNotificationCenter(_ center: UNUserNotificationCenter, didReceive response: UNNotificationResponse, withCompletionHandler completionHandler: @escaping () -> Void) {
        guard response.actionIdentifier == UNNotificationDefaultActionIdentifier else { completionHandler(); return }
        // A notification click may start the parent, but must not steal foreground focus.
        let helper = Bundle.main.bundleURL
        let parent = helper.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        guard parent.pathExtension == "app", parent != helper else { completionHandler(); exit(0) }
        let configuration = NSWorkspace.OpenConfiguration()
        configuration.activates = false
        configuration.addsToRecentItems = false
        NSWorkspace.shared.openApplication(at: parent, configuration: configuration) { _, _ in
            completionHandler()
            exit(0)
        }
    }
}

// MARK: - Custom toast
// macOS always draws the notifying app's icon in native banners, so a sender avatar can only be an
// attachment next to it. The toast shows the avatar (or a lettered fallback) as the only icon.
func toastInitial(_ title: String) -> String {
    let conversation = title.components(separatedBy: " · ").last ?? title
    return String(conversation.trimmingCharacters(in: .whitespaces).prefix(1)).uppercased()
}
func toastColor(_ title: String) -> NSColor {
    let palette: [NSColor] = [
        NSColor(red: 0.00, green: 0.41, blue: 1.00, alpha: 1), NSColor(red: 0.18, green: 0.65, blue: 0.45, alpha: 1),
        NSColor(red: 0.86, green: 0.36, blue: 0.30, alpha: 1), NSColor(red: 0.62, green: 0.40, blue: 0.84, alpha: 1),
        NSColor(red: 0.93, green: 0.60, blue: 0.14, alpha: 1), NSColor(red: 0.20, green: 0.62, blue: 0.78, alpha: 1)
    ]
    let hash = title.unicodeScalars.reduce(UInt32(7)) { ($0 &* 31) &+ $1.value }
    return palette[Int(hash % UInt32(palette.count))]
}
final class ToastView: NSVisualEffectView {
    var onClick: (() -> Void)?
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }
    override func mouseDown(with event: NSEvent) { onClick?() }
}
func openParentApp(activate: Bool) {
    let helper = Bundle.main.bundleURL
    let parent = helper.deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
    guard parent.pathExtension == "app", parent != helper else { exit(0) }
    let configuration = NSWorkspace.OpenConfiguration()
    configuration.activates = activate
    configuration.addsToRecentItems = false
    NSWorkspace.shared.openApplication(at: parent, configuration: configuration) { _, _ in exit(0) }
}
/// Bottom edge (Cocoa coordinates) of the lowest toast already on screen from another helper process.
func lowestExistingToastEdge() -> CGFloat? {
    guard let windows = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]],
          let primary = NSScreen.screens.first else { return nil }
    let ownName = NSRunningApplication.current.localizedName ?? ProcessInfo.processInfo.processName
    var lowest: CGFloat?
    for info in windows {
        // CoreGraphics reports the localized app name ("Zalo Notifications"), not the executable name.
        guard (info[kCGWindowOwnerName as String] as? String) == ownName,
              (info[kCGWindowOwnerPID as String] as? NSNumber)?.int32Value != getpid(),
              let bounds = info[kCGWindowBounds as String] as? [String: CGFloat],
              let y = bounds["Y"], let h = bounds["Height"], h < 200 else { continue }
        let bottom = primary.frame.height - (y + h)
        lowest = min(lowest ?? bottom, bottom)
    }
    return lowest
}
func showToast(_ payload: Payload) -> NSPanel? {
    guard let screen = NSScreen.main else { return nil }
    let width: CGFloat = 360, height: CGFloat = 76, margin: CGFloat = 12
    let visible = screen.visibleFrame
    let top = lowestExistingToastEdge().map { $0 - 8 } ?? (visible.maxY - margin)
    let frame = NSRect(x: visible.maxX - width - margin, y: max(visible.minY, top - height), width: width, height: height)
    let panel = NSPanel(contentRect: frame, styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: false)
    panel.level = .statusBar
    panel.isOpaque = false
    panel.backgroundColor = .clear
    panel.hasShadow = true
    panel.isMovable = false
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
    let view = ToastView(frame: NSRect(origin: .zero, size: frame.size))
    view.material = .hudWindow
    view.state = .active
    view.blendingMode = .behindWindow
    // A stretchable rounded mask makes the window shadow follow the rounded shape.
    let radius: CGFloat = 14
    let mask = NSImage(size: NSSize(width: radius * 2 + 1, height: radius * 2 + 1), flipped: false) { rect in
        NSColor.black.setFill()
        NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
        return true
    }
    mask.capInsets = NSEdgeInsets(top: radius, left: radius, bottom: radius, right: radius)
    mask.resizingMode = .stretch
    view.maskImage = mask
    view.onClick = { openParentApp(activate: true) }

    let avatarFrame = NSRect(x: 14, y: (height - 46) / 2, width: 46, height: 46)
    if let path = payload.attachment, FileManager.default.fileExists(atPath: path), let image = NSImage(contentsOfFile: path) {
        let imageView = NSImageView(frame: avatarFrame)
        imageView.image = image
        imageView.imageScaling = .scaleProportionallyUpOrDown
        imageView.wantsLayer = true
        imageView.layer?.cornerRadius = 23
        imageView.layer?.masksToBounds = true
        view.addSubview(imageView)
    } else {
        let circle = NSView(frame: avatarFrame)
        circle.wantsLayer = true
        circle.layer?.cornerRadius = 23
        circle.layer?.backgroundColor = toastColor(payload.title).cgColor
        let letter = NSTextField(labelWithString: toastInitial(payload.title))
        letter.font = .systemFont(ofSize: 20, weight: .semibold)
        letter.textColor = .white
        letter.alignment = .center
        letter.frame = NSRect(x: 0, y: (46 - 24) / 2, width: 46, height: 24)
        circle.addSubview(letter)
        view.addSubview(circle)
    }
    let textX: CGFloat = 72
    let title = NSTextField(labelWithString: payload.title)
    title.font = .systemFont(ofSize: 13, weight: .semibold)
    title.textColor = .labelColor
    title.lineBreakMode = .byTruncatingTail
    title.frame = NSRect(x: textX, y: height - 30, width: width - textX - 30, height: 18)
    let body = NSTextField(labelWithString: payload.body)
    body.font = .systemFont(ofSize: 12.5)
    body.textColor = .secondaryLabelColor
    body.lineBreakMode = .byTruncatingTail
    body.maximumNumberOfLines = 2
    body.cell?.wraps = true
    body.frame = NSRect(x: textX, y: 10, width: width - textX - 16, height: 34)
    let close = NSButton(title: "✕", target: nil, action: nil)
    close.isBordered = false
    close.font = .systemFont(ofSize: 11)
    close.contentTintColor = .tertiaryLabelColor
    close.frame = NSRect(x: width - 28, y: height - 28, width: 20, height: 20)
    close.target = view
    close.action = #selector(ToastView.dismissToast)
    view.addSubview(title); view.addSubview(body); view.addSubview(close)
    panel.contentView = view
    panel.alphaValue = 0
    panel.orderFrontRegardless()
    NSAnimationContext.runAnimationGroup { $0.duration = 0.2; panel.animator().alphaValue = 1 }
    DispatchQueue.main.asyncAfter(deadline: .now() + 6) {
        NSAnimationContext.runAnimationGroup({ $0.duration = 0.3; panel.animator().alphaValue = 0 }) { exit(0) }
    }
    return panel
}
extension ToastView {
    @objc func dismissToast() { exit(0) }
}
let args = Array(CommandLine.arguments.dropFirst())
if args == ["--self-test"] {
    guard Bundle.main.bundleIdentifier == bundleID, Bundle.main.bundleURL.pathExtension == "app" else { exit(1) }
    let valid = Payload(title: "Nhóm Gia đình", body: "Tin nhắn \"mới\"", id: String(repeating: "a", count: 64), sound: false, attachment: "/tmp/avatar.png")
    let badID = Payload(title: "Name", body: "Body", id: "raw-message", sound: true, attachment: nil)
    let multiline = Payload(title: "Name", body: "line1\nline2", id: String(repeating: "a", count: 64), sound: false, attachment: nil)
    let oversized = Payload(title: String(repeating: "x", count: 121), body: "Body", id: String(repeating: "a", count: 64), sound: false, attachment: nil)
    let emoji = Payload(title: "👨‍👩‍👧 Gia đình", body: "ok", id: String(repeating: "b", count: 64), sound: false, attachment: nil)
    guard valid.validate(), emoji.validate(), !badID.validate(), !multiline.validate(), !oversized.validate(), authorized(.authorized), !authorized(.denied), statusName(.notDetermined) == "notDetermined" else { exit(1) }
    guard toastInitial("Đức Đạt · CSĐH_HS") == "C", toastInitial("Nhóm") == "N", toastColor("a") == toastColor("a") else { exit(1) }
    print("SELF_TEST: bundle identity, bounded single-line payload, opaque identity, and authorization gates PASS")
    exit(0)
}
guard Bundle.main.bundleIdentifier == bundleID, Bundle.main.bundleURL.pathExtension == "app" else {
    fputs("Notification helper bundle identity required\n", stderr); exit(2)
}
guard args.isEmpty || args == ["--status"] || args == ["--request"] || args == ["--notify"] || args == ["--toast"] else { exit(2) }
let app = NSApplication.shared
app.setActivationPolicy(.prohibited)
let center = UNUserNotificationCenter.current()
let delegate = NotificationDelegate()
center.delegate = delegate
var finished = false
var resultCode: Int32 = 0
func finish(_ output: String, code: Int32 = 0) {
    DispatchQueue.main.async { print(output); resultCode = code; finished = true }
}
if args == ["--status"] {
    center.getNotificationSettings { settings in finish(statusName(settings.authorizationStatus)) }
} else if args == ["--request"] {
    // Only the explicit Rust user-action API launches this mode.
    center.requestAuthorization(options: [.alert, .sound]) { _, error in
        if error != nil { finish("unavailable", code: 3); return }
        center.getNotificationSettings { settings in finish(statusName(settings.authorizationStatus)) }
    }
} else if args == ["--toast"] {
    let data = FileHandle.standardInput.readData(ofLength: 4097)
    guard data.count <= 4096, let payload = try? JSONDecoder().decode(Payload.self, from: data), payload.validate() else { exit(2) }
    app.setActivationPolicy(.accessory)
    guard let panel = showToast(payload) else { print("failed"); exit(3) }
    withExtendedLifetime(panel) {
        print("submitted")
        fflush(stdout)
        DispatchQueue.main.asyncAfter(deadline: .now() + 8) { exit(0) }
        app.run()
    }
    exit(0)
} else if args == ["--notify"] {
    let data = FileHandle.standardInput.readData(ofLength: 4097)
    guard data.count <= 4096, let payload = try? JSONDecoder().decode(Payload.self, from: data), payload.validate() else { exit(2) }
    center.getNotificationSettings { settings in
        guard authorized(settings.authorizationStatus) else { finish("notAuthorized"); return }
        let content = UNMutableNotificationContent()
        content.title = payload.title
        content.body = payload.body
        if payload.sound { content.sound = .default }
        if let path = payload.attachment,
           FileManager.default.fileExists(atPath: path),
           let image = try? UNNotificationAttachment(identifier: "sender-avatar", url: URL(fileURLWithPath: path)) {
            content.attachments = [image]
        }
        let request = UNNotificationRequest(identifier: payload.id, content: content, trigger: nil)
        center.add(request) { error in finish(error == nil ? "submitted" : "failed", code: error == nil ? 0 : 3) }
    }
} else {
    // LaunchServices starts this bundle on notification click. No window or activation occurs.
    DispatchQueue.main.asyncAfter(deadline: .now() + 15) { NSApp.terminate(nil) }
    app.run()
    exit(0)
}
let timeout = Date().addingTimeInterval(args == ["--request"] ? 115 : 20)
while !finished && Date() < timeout { RunLoop.main.run(until: Date().addingTimeInterval(0.05)) }
exit(finished ? resultCode : 4)
