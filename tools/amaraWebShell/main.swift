import AppKit
import Foundation
import WebKit
import Darwin

private let audioSampleRate = 48_000
private let maximumRequestBytes = 32 * 1_024
private let audioChunkBytes = 16 * 1_024
private let audioStartupTimeout: TimeInterval = 12

private enum ShellError: LocalizedError {
    case missingRuntime(URL)
    case missingHelper(URL)
    case socket(String)

    var errorDescription: String? {
        switch self {
        case .missingRuntime(let url):
            return "The bundled web runtime is incomplete at \(url.path). Rebuild the app with Resources/runtime/web and Resources/runtime/src."
        case .missingHelper(let url):
            return "The bundled Mac audio helper is missing or is not executable at \(url.path)."
        case .socket(let detail):
            return "The local-only visualiser server could not start: \(detail)"
        }
    }
}

private enum CaptureStartError: Error {
    case busy
}

private func posixMessage(_ operation: String) -> String {
    "\(operation): \(String(cString: strerror(errno)))"
}

private func closeSocket(_ descriptor: Int32) {
    guard descriptor >= 0 else { return }
    _ = Darwin.shutdown(descriptor, SHUT_RDWR)
    _ = Darwin.close(descriptor)
}

private func sendAll(_ data: Data, to descriptor: Int32) -> Bool {
    data.withUnsafeBytes { raw in
        guard let base = raw.baseAddress else { return true }
        var sent = 0
        while sent < raw.count {
            let count = Darwin.send(descriptor, base.advanced(by: sent), raw.count - sent, 0)
            if count > 0 {
                sent += count
            } else if count < 0 && errno == EINTR {
                continue
            } else {
                return false
            }
        }
        return true
    }
}

private struct HTTPRequest {
    let method: String
    let target: String
    let headers: [String: String]

    var path: String {
        let withoutQuery = target.split(separator: "?", maxSplits: 1, omittingEmptySubsequences: false).first.map(String.init) ?? target
        return withoutQuery.removingPercentEncoding ?? withoutQuery
    }
}

private func readRequest(from descriptor: Int32) -> HTTPRequest? {
    var bytes = Data()
    var buffer = [UInt8](repeating: 0, count: 4_096)
    let marker = Data("\r\n\r\n".utf8)
    while bytes.count < maximumRequestBytes {
        let count = buffer.withUnsafeMutableBytes { raw in
            Darwin.recv(descriptor, raw.baseAddress, raw.count, 0)
        }
        if count > 0 {
            bytes.append(buffer, count: count)
            if bytes.range(of: marker) != nil { break }
        } else if count < 0 && errno == EINTR {
            continue
        } else {
            return nil
        }
    }
    guard let headerEnd = bytes.range(of: marker)?.lowerBound,
          let text = String(data: bytes[..<headerEnd], encoding: .utf8) else { return nil }
    let lines = text.components(separatedBy: "\r\n")
    guard let first = lines.first else { return nil }
    let requestLine = first.split(separator: " ", omittingEmptySubsequences: true)
    guard requestLine.count == 3, requestLine[2].hasPrefix("HTTP/1.") else { return nil }
    var headers: [String: String] = [:]
    for line in lines.dropFirst() {
        guard let colon = line.firstIndex(of: ":") else { return nil }
        let name = line[..<colon].trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let value = line[line.index(after: colon)...].trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, headers[name] == nil else { return nil }
        headers[name] = value
    }
    return HTTPRequest(method: String(requestLine[0]), target: String(requestLine[1]), headers: headers)
}

private func statusText(_ status: Int) -> String {
    switch status {
    case 200: return "OK"
    case 400: return "Bad Request"
    case 403: return "Forbidden"
    case 404: return "Not Found"
    case 405: return "Method Not Allowed"
    case 409: return "Conflict"
    case 503: return "Service Unavailable"
    case 504: return "Gateway Timeout"
    default: return "Error"
    }
}

private func responseHead(status: Int, headers: [String: String]) -> Data {
    var lines = ["HTTP/1.1 \(status) \(statusText(status))"]
    for key in headers.keys.sorted() {
        if let value = headers[key] { lines.append("\(key): \(value)") }
    }
    lines.append("")
    lines.append("")
    return Data(lines.joined(separator: "\r\n").utf8)
}

private func sendResponse(
    to descriptor: Int32,
    status: Int,
    contentType: String,
    body: Data,
    headOnly: Bool = false,
    extraHeaders: [String: String] = [:]
) {
    var headers = [
        "Cache-Control": "no-store",
        "Connection": "close",
        "Content-Length": String(body.count),
        "Content-Type": contentType,
        "X-Content-Type-Options": "nosniff",
    ]
    for (key, value) in extraHeaders { headers[key] = value }
    guard sendAll(responseHead(status: status, headers: headers), to: descriptor), !headOnly else { return }
    _ = sendAll(body, to: descriptor)
}

private func sendJSON(to descriptor: Int32, status: Int, value: [String: Any]) {
    let body = (try? JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])) ?? Data("{}".utf8)
    sendResponse(to: descriptor, status: status, contentType: "application/json; charset=utf-8", body: body)
}

private final class CaptureSession: @unchecked Sendable {
    let process: Process
    let stdout: FileHandle
    let stderr: FileHandle
    private let stopLock = NSLock()
    private var reaped = false

    init(process: Process, stdout: FileHandle, stderr: FileHandle) {
        self.process = process
        self.stdout = stdout
        self.stderr = stderr
    }

    var isRunning: Bool { process.isRunning }

    func terminateAndReap() {
        stopLock.lock()
        defer { stopLock.unlock() }
        guard !reaped else { return }
        if process.isRunning { process.terminate() }
        let deadline = Date().addingTimeInterval(3)
        while process.isRunning && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.02)
        }
        if process.isRunning {
            _ = Darwin.kill(process.processIdentifier, SIGKILL)
        }
        process.waitUntilExit()
        reaped = true
    }

    func closePipes() {
        try? stdout.close()
        try? stderr.close()
    }
}

private final class CaptureCoordinator: @unchecked Sendable {
    private let helperURL: URL
    private let lock = NSLock()
    private var current: CaptureSession?

    init(helperURL: URL) {
        self.helperURL = helperURL
    }

    var helperAvailable: Bool {
        FileManager.default.isExecutableFile(atPath: helperURL.path)
    }

    var isActive: Bool {
        lock.lock()
        defer { lock.unlock() }
        return current?.isRunning == true
    }

    func begin() throws -> CaptureSession {
        lock.lock()
        defer { lock.unlock() }
        // The stream-owning connection clears this slot after it has reaped
        // the helper and closed both pipes. Do not let a fast retry overlap
        // that cleanup just because the child has already exited.
        if current != nil { throw CaptureStartError.busy }
        guard helperAvailable else { throw ShellError.missingHelper(helperURL) }
        let output = Pipe()
        let diagnostics = Pipe()
        let process = Process()
        process.executableURL = helperURL
        process.standardOutput = output
        process.standardError = diagnostics
        try process.run()
        let session = CaptureSession(
            process: process,
            stdout: output.fileHandleForReading,
            stderr: diagnostics.fileHandleForReading
        )
        current = session
        return session
    }

    func finish(_ session: CaptureSession) {
        session.terminateAndReap()
        session.closePipes()
        lock.lock()
        if current === session { current = nil }
        lock.unlock()
    }

    func stop() {
        lock.lock()
        let session = current
        lock.unlock()
        // The streaming connection owns its pipe descriptors and will close
        // them in finish(). Closing them here could race a poll/read on that
        // connection. Termination is enough to wake it with EOF/HUP.
        session?.terminateAndReap()
    }
}

private final class ConnectionRegistry: @unchecked Sendable {
    private let lock = NSLock()
    private var sockets = Set<Int32>()

    func insert(_ descriptor: Int32) {
        lock.lock()
        sockets.insert(descriptor)
        lock.unlock()
    }

    func remove(_ descriptor: Int32) {
        lock.lock()
        sockets.remove(descriptor)
        lock.unlock()
    }

    func interruptAll() {
        lock.lock()
        let snapshot = sockets
        lock.unlock()
        for descriptor in snapshot { _ = Darwin.shutdown(descriptor, SHUT_RDWR) }
    }
}

private final class LoopbackServer: @unchecked Sendable {
    private let stableOrigin: StableOrigin
    private let runtimeRoot: URL
    private let webRoot: URL
    private let shaderRoot: URL
    private let capture: CaptureCoordinator
    private let acceptQueue = DispatchQueue(label: "com.amara.musicvisualiser.web.accept")
    private let connectionQueue = DispatchQueue(label: "com.amara.musicvisualiser.web.connections", attributes: .concurrent)
    private let stateLock = NSLock()
    private let connections = ConnectionRegistry()
    private var listener: Int32 = -1
    private var reservation: Int32 = -1
    private var stopping = false
    private(set) var port: UInt16 = 0

    var authority: String { "127.0.0.1:\(port)" }
    var origin: String { "http://\(authority)" }
    var startURL: URL { URL(string: "\(origin)/web/")! }

    init(bundle: Bundle = .main) throws {
        let support = try FileManager.default.url(for: .applicationSupportDirectory, in: .userDomainMask, appropriateFor: nil, create: true)
        stableOrigin = try StableOrigin(directory: support.appendingPathComponent("com.amara.musicvisualiser.web", isDirectory: true))
        guard let resources = bundle.resourceURL else {
            throw ShellError.missingRuntime(bundle.bundleURL)
        }
        runtimeRoot = resources.appendingPathComponent("runtime", isDirectory: true).resolvingSymlinksInPath()
        webRoot = runtimeRoot.appendingPathComponent("web", isDirectory: true).resolvingSymlinksInPath()
        shaderRoot = runtimeRoot.appendingPathComponent("src", isDirectory: true).resolvingSymlinksInPath()
        let index = webRoot.appendingPathComponent("index.html")
        let shader = shaderRoot.appendingPathComponent("shader.wgsl")
        guard FileManager.default.isReadableFile(atPath: index.path),
              FileManager.default.isReadableFile(atPath: shader.path) else {
            throw ShellError.missingRuntime(runtimeRoot)
        }
        let helper = bundle.bundleURL
            .appendingPathComponent("Contents", isDirectory: true)
            .appendingPathComponent("Helpers", isDirectory: true)
            .appendingPathComponent("amaraAudioCapture")
        capture = CaptureCoordinator(helperURL: helper)
    }

    func start() throws {
        stateLock.lock()
        defer { stateLock.unlock() }
        guard listener < 0 else { return }
        stopping = false
        let savedPort = try stableOrigin.loadPort()
        // Reserve the port on all interfaces without listening there. This
        // detects existing wildcard listeners and prevents new wildcard
        // listeners while the actual HTTP server listens on loopback only.
        let reserve = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard reserve >= 0 else { throw ShellError.socket(posixMessage("reservation socket")) }
        let descriptor = Darwin.socket(AF_INET, SOCK_STREAM, 0)
        guard descriptor >= 0 else { closeSocket(reserve); throw ShellError.socket(posixMessage("socket")) }
        var committed = false
        defer { if !committed { closeSocket(descriptor); closeSocket(reserve) } }
        for socket in [reserve, descriptor] {
            var reuse: Int32 = 1
            let result = withUnsafePointer(to: &reuse) {
                setsockopt(socket, SOL_SOCKET, SO_REUSEADDR, $0, socklen_t(MemoryLayout<Int32>.size))
            }
            guard result == 0 else { throw ShellError.socket(posixMessage("setsockopt SO_REUSEADDR")) }
        }
        var noPipe: Int32 = 1
        _ = withUnsafePointer(to: &noPipe) {
            setsockopt(descriptor, SOL_SOCKET, SO_NOSIGPIPE, $0, socklen_t(MemoryLayout<Int32>.size))
        }
        var address = sockaddr_in()
        address.sin_len = UInt8(MemoryLayout<sockaddr_in>.size)
        address.sin_family = sa_family_t(AF_INET)
        address.sin_port = in_port_t(savedPort ?? 0).bigEndian
        address.sin_addr = in_addr(s_addr: inet_addr("0.0.0.0"))
        let reserved = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(reserve, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard reserved == 0 else { throw ShellError.socket("The saved local address is in use or unavailable. Quit the other instance or free the port, then retry. \(posixMessage("reserve"))") }
        var length = socklen_t(MemoryLayout<sockaddr_in>.size)
        let named = withUnsafeMutablePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                getsockname(reserve, $0, &length)
            }
        }
        guard named == 0 else { throw ShellError.socket(posixMessage("getsockname")) }
        address.sin_addr = in_addr(s_addr: inet_addr("127.0.0.1"))
        let bound = withUnsafePointer(to: &address) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                Darwin.bind(descriptor, $0, socklen_t(MemoryLayout<sockaddr_in>.size))
            }
        }
        guard bound == 0 else { throw ShellError.socket(posixMessage("loopback bind")) }
        guard Darwin.listen(descriptor, 32) == 0 else { throw ShellError.socket(posixMessage("loopback listen")) }
        let actualPort = UInt16(bigEndian: address.sin_port)
        if savedPort == nil { try stableOrigin.saveInitialPort(actualPort) }
        reservation = reserve
        committed = true
        listener = descriptor
        port = actualPort
        acceptQueue.async { [weak self] in self?.acceptConnections() }
    }

    func stop() {
        stateLock.lock()
        stopping = true
        let descriptor = listener
        let reserved = reservation
        listener = -1
        reservation = -1
        stateLock.unlock()
        closeSocket(descriptor)
        closeSocket(reserved)
        capture.stop()
        connections.interruptAll()
    }

    private func acceptConnections() {
        while true {
            stateLock.lock()
            let descriptor = listener
            let shouldStop = stopping
            stateLock.unlock()
            if shouldStop || descriptor < 0 { return }
            var address = sockaddr_in()
            var length = socklen_t(MemoryLayout<sockaddr_in>.size)
            let client = withUnsafeMutablePointer(to: &address) {
                $0.withMemoryRebound(to: sockaddr.self, capacity: 1) {
                    Darwin.accept(descriptor, $0, &length)
                }
            }
            if client < 0 {
                if errno == EINTR { continue }
                stateLock.lock()
                let done = stopping || listener < 0
                stateLock.unlock()
                if done { return }
                continue
            }
            var noPipe: Int32 = 1
            _ = withUnsafePointer(to: &noPipe) {
                setsockopt(client, SOL_SOCKET, SO_NOSIGPIPE, $0, socklen_t(MemoryLayout<Int32>.size))
            }
            var receiveTimeout = timeval(tv_sec: 5, tv_usec: 0)
            _ = withUnsafePointer(to: &receiveTimeout) {
                setsockopt(client, SOL_SOCKET, SO_RCVTIMEO, $0, socklen_t(MemoryLayout<timeval>.size))
            }
            var sendTimeout = timeval(tv_sec: 2, tv_usec: 0)
            _ = withUnsafePointer(to: &sendTimeout) {
                setsockopt(client, SOL_SOCKET, SO_SNDTIMEO, $0, socklen_t(MemoryLayout<timeval>.size))
            }
            connections.insert(client)
            connectionQueue.async { [weak self] in
                defer {
                    self?.connections.remove(client)
                    closeSocket(client)
                }
                self?.handle(client)
            }
        }
    }

    private func handle(_ descriptor: Int32) {
        guard let request = readRequest(from: descriptor) else {
            sendResponse(to: descriptor, status: 400, contentType: "text/plain; charset=utf-8", body: Data("Bad request".utf8))
            return
        }
        switch request.method {
        case "GET", "HEAD": serveAsset(request, to: descriptor)
        case "POST": serveAPI(request, to: descriptor)
        default:
            sendResponse(to: descriptor, status: 405, contentType: "text/plain; charset=utf-8", body: Data("Method not allowed".utf8))
        }
    }

    private func serveAsset(_ request: HTTPRequest, to descriptor: Int32) {
        guard request.headers["host"] == authority else {
            sendResponse(to: descriptor, status: 403, contentType: "text/plain; charset=utf-8", body: Data("Forbidden".utf8), headOnly: request.method == "HEAD")
            return
        }
        var path = request.path
        if path == "/" || path == "/web/" { path = "/web/index.html" }
        guard !path.contains("\0"), !path.split(separator: "/").contains(".."),
              let asset = allowedAsset(for: path),
              let data = try? Data(contentsOf: asset, options: [.mappedIfSafe]) else {
            sendResponse(to: descriptor, status: 404, contentType: "text/plain; charset=utf-8", body: Data("Not found".utf8), headOnly: request.method == "HEAD")
            return
        }
        sendResponse(
            to: descriptor,
            status: 200,
            contentType: contentType(for: asset.pathExtension),
            body: data,
            headOnly: request.method == "HEAD",
            extraHeaders: [
                "Content-Security-Policy": "frame-ancestors 'none'",
                "X-Frame-Options": "DENY",
            ]
        )
    }

    private func allowedAsset(for path: String) -> URL? {
        let trimmed = path.hasPrefix("/") ? String(path.dropFirst()) : path
        let components = trimmed.split(separator: "/", omittingEmptySubsequences: false)
        guard components.count >= 2 else { return nil }
        let relative = components.dropFirst().joined(separator: "/")
        let root: URL
        let extensions: Set<String>
        switch components[0] {
        case "web":
            root = webRoot
            extensions = ["html", "js", "css", "svg", "png", "json"]
        case "src":
            root = shaderRoot
            extensions = ["wgsl"]
        default:
            return nil
        }
        let candidate = root.appendingPathComponent(relative).standardizedFileURL.resolvingSymlinksInPath()
        let rootPath = root.path.hasSuffix("/") ? root.path : root.path + "/"
        guard candidate.path.hasPrefix(rootPath), extensions.contains(candidate.pathExtension.lowercased()),
              FileManager.default.isReadableFile(atPath: candidate.path) else { return nil }
        return candidate
    }

    private func contentType(for extensionName: String) -> String {
        switch extensionName.lowercased() {
        case "html": return "text/html; charset=utf-8"
        case "js": return "text/javascript; charset=utf-8"
        case "css": return "text/css; charset=utf-8"
        case "svg": return "image/svg+xml"
        case "png": return "image/png"
        case "json": return "application/json; charset=utf-8"
        case "wgsl": return "text/plain; charset=utf-8"
        default: return "application/octet-stream"
        }
    }

    private func serveAPI(_ request: HTTPRequest, to descriptor: Int32) {
        let authorised = request.headers["host"] == authority
            && request.headers["origin"] == origin
            && request.headers["content-length"] == "0"
            && request.headers["transfer-encoding"] == nil
        guard authorised else {
            sendJSON(to: descriptor, status: 403, value: ["error": "Request origin is not allowed."])
            return
        }
        switch request.path {
        case "/api/system-audio/status", "/api/mac-audio/status":
            sendJSON(to: descriptor, status: 200, value: [
                "active": capture.isActive,
                "available": capture.helperAvailable,
                "channels": 1,
                "format": "f32le",
                "sampleRate": audioSampleRate,
            ])
        case "/api/system-audio/stop", "/api/mac-audio/stop":
            capture.stop()
            sendJSON(to: descriptor, status: 200, value: ["stopped": true])
        case "/api/system-audio/stream", "/api/mac-audio/stream":
            streamAudio(to: descriptor)
        default:
            sendJSON(to: descriptor, status: 404, value: ["error": "Unknown endpoint."])
        }
    }

    private func streamAudio(to descriptor: Int32) {
        let session: CaptureSession
        do {
            session = try capture.begin()
        } catch CaptureStartError.busy {
            sendJSON(to: descriptor, status: 409, value: ["error": "Computer audio is already in use by this app."])
            return
        } catch ShellError.missingHelper {
            sendJSON(to: descriptor, status: 503, value: ["error": "The signed Mac audio helper is not installed."])
            return
        } catch {
            sendJSON(to: descriptor, status: 503, value: ["error": "The Mac audio helper could not be started."])
            return
        }
        defer { capture.finish(session) }
        let stdoutFD = session.stdout.fileDescriptor
        let stderrFD = session.stderr.fileDescriptor
        let deadline = Date().addingTimeInterval(audioStartupTimeout)
        var started = false
        var stdoutOpen = true
        var buffer = [UInt8](repeating: 0, count: audioChunkBytes)
        while session.isRunning || stdoutOpen {
            if !started && Date() >= deadline {
                sendJSON(to: descriptor, status: 504, value: ["error": "Computer audio permission is not enabled. Add amara music visualiser in System Settings › Privacy & Security › Screen & System Audio Recording › System Audio Recording Only, then quit and reopen the app."])
                return
            }
            var watched = [
                pollfd(fd: stdoutFD, events: Int16(POLLIN | POLLHUP | POLLERR), revents: 0),
                pollfd(fd: stderrFD, events: Int16(POLLIN | POLLHUP | POLLERR), revents: 0),
                pollfd(fd: descriptor, events: Int16(POLLIN | POLLHUP | POLLERR), revents: 0),
            ]
            let result = watched.withUnsafeMutableBufferPointer {
                Darwin.poll($0.baseAddress, nfds_t($0.count), 250)
            }
            if result < 0 {
                if errno == EINTR { continue }
                return
            }
            if watched[2].revents & Int16(POLLHUP | POLLERR | POLLNVAL) != 0 { return }
            if watched[2].revents & Int16(POLLIN) != 0 {
                var byte: UInt8 = 0
                let count = Darwin.recv(descriptor, &byte, 1, MSG_PEEK | MSG_DONTWAIT)
                if count == 0 { return }
            }
            if watched[1].revents & Int16(POLLIN) != 0 {
                var discarded = [UInt8](repeating: 0, count: 4_096)
                _ = discarded.withUnsafeMutableBytes { Darwin.read(stderrFD, $0.baseAddress, $0.count) }
            }
            if watched[0].revents & Int16(POLLIN) != 0 {
                let count = buffer.withUnsafeMutableBytes { Darwin.read(stdoutFD, $0.baseAddress, $0.count) }
                if count > 0 {
                    if !started {
                        let head = responseHead(status: 200, headers: [
                            "Cache-Control": "no-store",
                            "Connection": "close",
                            "Content-Type": "application/octet-stream",
                            "Transfer-Encoding": "chunked",
                        ])
                        guard sendAll(head, to: descriptor) else { return }
                        started = true
                    }
                    let prefix = Data(String(count, radix: 16).utf8) + Data("\r\n".utf8)
                    guard sendAll(prefix, to: descriptor),
                          sendAll(Data(buffer.prefix(count)), to: descriptor),
                          sendAll(Data("\r\n".utf8), to: descriptor) else { return }
                } else if count == 0 {
                    stdoutOpen = false
                }
            }
            if watched[0].revents & Int16(POLLHUP | POLLERR | POLLNVAL) != 0 {
                stdoutOpen = false
            }
            if !session.isRunning && !stdoutOpen { break }
        }
        if started {
            _ = sendAll(Data("0\r\n\r\n".utf8), to: descriptor)
        } else {
            sendJSON(to: descriptor, status: 503, value: ["error": "Computer audio permission is not enabled. Add amara music visualiser in System Settings › Privacy & Security › Screen & System Audio Recording › System Audio Recording Only, then quit and reopen the app."])
        }
    }
}

@MainActor
private final class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, WKNavigationDelegate, WKUIDelegate {
    private var window: NSWindow!
    private var rootView: NSView!
    private var webView: WKWebView?
    private var failureView: NSVisualEffectView!
    private var failureTitle: NSTextField!
    private var failureMessage: NSTextField!
    private var server: LoopbackServer?
    private var navigationGeneration = 0

    func applicationDidFinishLaunching(_ notification: Notification) {
        configureMenu()
        configureWindow()
        startExperience()
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool { true }

    func applicationWillTerminate(_ notification: Notification) {
        webView?.stopLoading()
        server?.stop()
    }

    private func configureMenu() {
        let menu = NSMenu()
        let appItem = NSMenuItem()
        let appMenu = NSMenu(title: "amara music visualiser")
        let about = NSMenuItem(title: "About amara music visualiser", action: #selector(showAbout), keyEquivalent: "")
        about.target = self
        appMenu.addItem(about)
        let website = NSMenuItem(title: "Visit amara.gmbh", action: #selector(openCompanyWebsite), keyEquivalent: "")
        website.target = self
        appMenu.addItem(website)
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Hide amara music visualiser", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")
        let hideOthers = appMenu.addItem(withTitle: "Hide Others", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        appMenu.addItem(withTitle: "Show All", action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")
        appMenu.addItem(.separator())
        appMenu.addItem(withTitle: "Quit amara music visualiser", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q")
        appItem.submenu = appMenu
        menu.addItem(appItem)

        let editItem = NSMenuItem()
        let editMenu = NSMenu(title: "Edit")
        editMenu.addItem(withTitle: "Undo", action: Selector(("undo:")), keyEquivalent: "z")
        let redo = editMenu.addItem(withTitle: "Redo", action: Selector(("redo:")), keyEquivalent: "z")
        redo.keyEquivalentModifierMask = [.command, .shift]
        editMenu.addItem(.separator())
        editMenu.addItem(withTitle: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x")
        editMenu.addItem(withTitle: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c")
        editMenu.addItem(withTitle: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v")
        editMenu.addItem(withTitle: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a")
        editItem.submenu = editMenu
        menu.addItem(editItem)

        let windowItem = NSMenuItem()
        let windowMenu = NSMenu(title: "Window")
        windowMenu.addItem(withTitle: "Minimise", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m")
        windowMenu.addItem(withTitle: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: "")
        windowItem.submenu = windowMenu
        menu.addItem(windowItem)
        NSApp.mainMenu = menu
        NSApp.windowsMenu = windowMenu
    }

    @objc private func showAbout() {
        let credits = NSMutableAttributedString(string: "Web experience 20261031.19\n")
        let website = URL(string: "https://amara.gmbh")!
        credits.append(NSAttributedString(string: "amara.gmbh", attributes: [
            .link: website,
            .foregroundColor: NSColor.linkColor,
            .underlineStyle: NSUnderlineStyle.single.rawValue
        ]))
        NSApp.orderFrontStandardAboutPanel(options: [
            .applicationName: "amara music visualiser",
            .credits: credits
        ])
    }

    @objc private func openCompanyWebsite() {
        guard let website = URL(string: "https://amara.gmbh") else { return }
        NSWorkspace.shared.open(website)
    }

    private func configureWindow() {
        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1_440, height: 900),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.title = "amara music visualiser"
        window.minSize = NSSize(width: 960, height: 640)
        window.center()
        window.delegate = self
        rootView = NSView(frame: window.contentView?.bounds ?? .zero)
        rootView.autoresizingMask = [.width, .height]
        window.contentView = rootView
        configureFailureView()
        window.makeKeyAndOrderFront(nil)
    }

    private func configureFailureView() {
        failureView = NSVisualEffectView()
        failureView.material = .hudWindow
        failureView.blendingMode = .withinWindow
        failureView.state = .active
        failureView.wantsLayer = true
        failureView.layer?.cornerRadius = 18
        failureView.translatesAutoresizingMaskIntoConstraints = false
        failureView.isHidden = true

        failureTitle = NSTextField(labelWithString: "amara music visualiser could not start")
        failureTitle.font = .systemFont(ofSize: 22, weight: .semibold)
        failureTitle.alignment = .center
        failureMessage = NSTextField(wrappingLabelWithString: "")
        failureMessage.font = .systemFont(ofSize: 14)
        failureMessage.alignment = .center
        failureMessage.maximumNumberOfLines = 8

        let retry = NSButton(title: "Retry", target: self, action: #selector(retryExperience))
        retry.keyEquivalent = "\r"
        let quit = NSButton(title: "Quit", target: NSApp, action: #selector(NSApplication.terminate(_:)))
        let buttons = NSStackView(views: [retry, quit])
        buttons.orientation = .horizontal
        buttons.spacing = 12
        buttons.alignment = .centerY

        let stack = NSStackView(views: [failureTitle, failureMessage, buttons])
        stack.orientation = .vertical
        stack.alignment = .centerX
        stack.spacing = 16
        stack.edgeInsets = NSEdgeInsets(top: 30, left: 34, bottom: 30, right: 34)
        stack.translatesAutoresizingMaskIntoConstraints = false
        failureView.addSubview(stack)
        rootView.addSubview(failureView)
        NSLayoutConstraint.activate([
            failureView.centerXAnchor.constraint(equalTo: rootView.centerXAnchor),
            failureView.centerYAnchor.constraint(equalTo: rootView.centerYAnchor),
            failureView.widthAnchor.constraint(lessThanOrEqualToConstant: 620),
            failureView.widthAnchor.constraint(greaterThanOrEqualToConstant: 460),
            stack.leadingAnchor.constraint(equalTo: failureView.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: failureView.trailingAnchor),
            stack.topAnchor.constraint(equalTo: failureView.topAnchor),
            stack.bottomAnchor.constraint(equalTo: failureView.bottomAnchor),
            failureMessage.widthAnchor.constraint(lessThanOrEqualToConstant: 540),
        ])
    }

    private func startExperience() {
        failureView.isHidden = true
        do {
            if server == nil {
                let created = try LoopbackServer()
                try created.start()
                server = created
            }
            guard let server else { return }
            if webView == nil { configureWebView(server: server) }
            navigationGeneration += 1
            webView?.load(URLRequest(url: server.startURL, cachePolicy: .reloadIgnoringLocalCacheData))
        } catch {
            showFailure(
                title: "The local visualiser could not start",
                message: "\(error.localizedDescription)\n\nNo external server was opened and no audio was captured."
            )
        }
    }

    private func configureWebView(server: LoopbackServer) {
        let configuration = WKWebViewConfiguration()
        configuration.preferences.isElementFullscreenEnabled = true
        configuration.websiteDataStore = .default()
        configuration.mediaTypesRequiringUserActionForPlayback = [.audio]
        let view = WKWebView(frame: .zero, configuration: configuration)
        view.navigationDelegate = self
        view.uiDelegate = self
        view.allowsMagnification = false
        view.translatesAutoresizingMaskIntoConstraints = false
        rootView.addSubview(view, positioned: .below, relativeTo: failureView)
        NSLayoutConstraint.activate([
            view.leadingAnchor.constraint(equalTo: rootView.leadingAnchor),
            view.trailingAnchor.constraint(equalTo: rootView.trailingAnchor),
            view.topAnchor.constraint(equalTo: rootView.topAnchor),
            view.bottomAnchor.constraint(equalTo: rootView.bottomAnchor),
        ])
        webView = view
    }

    @objc private func retryExperience() {
        if server == nil { startExperience() }
        else {
            failureView.isHidden = true
            navigationGeneration += 1
            webView?.reloadFromOrigin()
        }
    }

    private func showFailure(title: String, message: String) {
        failureTitle.stringValue = title
        failureMessage.stringValue = message
        failureView.isHidden = false
        rootView.addSubview(failureView, positioned: .above, relativeTo: webView)
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        let generation = navigationGeneration
        webView.evaluateJavaScript("Boolean(navigator.gpu)") { [weak self] result, error in
            guard let self, generation == self.navigationGeneration else { return }
            if let error {
                self.showFailure(title: "WebGPU check failed", message: "The embedded page loaded, but its graphics capability could not be checked.\n\n\(error.localizedDescription)")
            } else if (result as? Bool) != true {
                self.showFailure(title: "WebGPU is unavailable", message: "This Mac's embedded WebKit view did not expose WebGPU. Update macOS or use the native amaraMusicVisualiser build.")
            }
        }
        DispatchQueue.main.asyncAfter(deadline: .now() + 30) { [weak self, weak webView] in
            guard let self, let webView, generation == self.navigationGeneration else { return }
            let script = "JSON.stringify({gpu:Boolean(navigator.gpu),status:(document.querySelector('#status')?.textContent||'').trim()})"
            webView.evaluateJavaScript(script) { result, error in
                guard generation == self.navigationGeneration else { return }
                if let error {
                    self.showFailure(title: "Visualiser startup could not be verified", message: error.localizedDescription)
                    return
                }
                guard let text = result as? String,
                      let data = text.data(using: .utf8),
                      let state = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
                    self.showFailure(title: "Visualiser startup could not be verified", message: "The embedded page did not return a valid startup state.")
                    return
                }
                let status = state["status"] as? String ?? ""
                if state["gpu"] as? Bool != true {
                    self.showFailure(title: "WebGPU is unavailable", message: "The embedded WebKit renderer cannot start the visualiser on this Mac.")
                } else if status.hasPrefix("Visualiser could not start:") || status.hasPrefix("Graphics device lost:") || status == "Preparing your visualiser…" {
                    self.showFailure(title: "amara music visualiser could not start", message: status.isEmpty ? "The visualiser did not report a usable graphics state." : status)
                }
            }
        }
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        showFailure(title: "The visualiser page failed", message: error.localizedDescription)
    }

    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        showFailure(title: "The local visualiser could not load", message: error.localizedDescription)
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        guard let url = navigationAction.request.url else { decisionHandler(.cancel); return }
        if let server,
           (url.scheme == "http" && url.host == "127.0.0.1" && url.port == Int(server.port)
            || url.scheme == "blob" || url.scheme == "data" || url.scheme == "about") {
            decisionHandler(.allow)
        } else {
            decisionHandler(.cancel)
        }
    }

    func webView(
        _ webView: WKWebView,
        runOpenPanelWith parameters: WKOpenPanelParameters,
        initiatedByFrame frame: WKFrameInfo,
        completionHandler: @escaping ([URL]?) -> Void
    ) {
        guard webView === self.webView, frame.isMainFrame, let server,
              frame.securityOrigin.protocol == "http",
              frame.securityOrigin.host == "127.0.0.1",
              frame.securityOrigin.port == Int(server.port),
              let window, window.isVisible else {
            completionHandler(nil)
            return
        }
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = parameters.allowsDirectories
        panel.allowsMultipleSelection = parameters.allowsMultipleSelection
        panel.beginSheetModal(for: window) { response in
            completionHandler(response == .OK ? panel.urls : nil)
        }
    }

    @available(macOS 12.0, *)
    func webView(
        _ webView: WKWebView,
        requestMediaCapturePermissionFor origin: WKSecurityOrigin,
        initiatedByFrame frame: WKFrameInfo,
        type: WKMediaCaptureType,
        decisionHandler: @escaping (WKPermissionDecision) -> Void
    ) {
        guard let server,
              origin.protocol == "http",
              origin.host == "127.0.0.1",
              origin.port == Int(server.port) else {
            decisionHandler(.deny)
            return
        }
        decisionHandler(.grant)
    }
}

@main
private struct AmaraWebShell {
    @MainActor
    static func main() {
        let app = NSApplication.shared
        app.setActivationPolicy(.regular)
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
        withExtendedLifetime(delegate) {}
    }
}
