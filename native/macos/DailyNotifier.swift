import AppKit
import UserNotifications

let bundleID = "com.ctdoteam1110.ctdozlite.notifications"
struct Payload: Decodable {
    let title: String
    let body: String
    let id: String
    let sound: Bool
    func validate() -> Bool {
        // Sender/group title and message preview are shown by owner request. The Rust caller
        // flattens control characters and caps length; re-check those bounds here so a
        // malformed caller cannot post multi-line or oversized banners. The id stays opaque.
        func boundedText(_ text: String, _ limit: Int) -> Bool {
            !text.isEmpty && text.count <= limit && !text.unicodeScalars.contains { $0.properties.generalCategory == .control }
        }
        return boundedText(title, 120) && boundedText(body, 240) && id.count == 64 && id.utf8.allSatisfy { (48...57).contains($0) || (97...102).contains($0) }
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
let args = Array(CommandLine.arguments.dropFirst())
if args == ["--self-test"] {
    guard Bundle.main.bundleIdentifier == bundleID, Bundle.main.bundleURL.pathExtension == "app" else { exit(1) }
    let valid = Payload(title: "Nhóm Gia đình", body: "Tin nhắn \"mới\"", id: String(repeating: "a", count: 64), sound: false)
    let badID = Payload(title: "Name", body: "Body", id: "raw-message", sound: true)
    let multiline = Payload(title: "Name", body: "line1\nline2", id: String(repeating: "a", count: 64), sound: false)
    let oversized = Payload(title: String(repeating: "x", count: 121), body: "Body", id: String(repeating: "a", count: 64), sound: false)
    let emoji = Payload(title: "👨‍👩‍👧 Gia đình", body: "ok", id: String(repeating: "b", count: 64), sound: false)
    guard valid.validate(), emoji.validate(), !badID.validate(), !multiline.validate(), !oversized.validate(), authorized(.authorized), !authorized(.denied), statusName(.notDetermined) == "notDetermined" else { exit(1) }
    print("SELF_TEST: bundle identity, bounded single-line payload, opaque identity, and authorization gates PASS")
    exit(0)
}
guard Bundle.main.bundleIdentifier == bundleID, Bundle.main.bundleURL.pathExtension == "app" else {
    fputs("Notification helper bundle identity required\n", stderr); exit(2)
}
guard args.isEmpty || args == ["--status"] || args == ["--request"] || args == ["--notify"] else { exit(2) }
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
} else if args == ["--notify"] {
    let data = FileHandle.standardInput.readData(ofLength: 4097)
    guard data.count <= 4096, let payload = try? JSONDecoder().decode(Payload.self, from: data), payload.validate() else { exit(2) }
    center.getNotificationSettings { settings in
        guard authorized(settings.authorizationStatus) else { finish("notAuthorized"); return }
        let content = UNMutableNotificationContent()
        content.title = payload.title
        content.body = payload.body
        if payload.sound { content.sound = .default }
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
