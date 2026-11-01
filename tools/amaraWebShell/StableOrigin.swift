import Foundation
import Darwin

// Owns an exclusive lifetime lock for a private app-specific directory.
final class StableOrigin {
    enum Failure: LocalizedError {
        case unsafeDirectory, unsafeFile, busy, invalidPort, io
        var errorDescription: String? {
            switch self {
            case .unsafeDirectory, .unsafeFile: return "The local settings location is unsafe or inaccessible. Check its ownership and permissions."
            case .busy: return "Another instance is using the local visualiser settings. Quit it and retry."
            case .invalidPort: return "The saved local address is invalid. Preserve the settings and contact support."
            case .io: return "The local address could not be saved. Check disk space and permissions, then retry."
            }
        }
    }
    let directory: URL
    private var lockFD: Int32 = -1
    private let fm = FileManager.default
    init(directory: URL) throws {
        self.directory = directory
        try fm.createDirectory(at: directory, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        var st = stat()
        guard lstat(directory.path, &st) == 0, (st.st_mode & S_IFMT) == S_IFDIR,
              st.st_uid == getuid(), st.st_mode & 0o077 == 0 else { throw Failure.unsafeDirectory }
        let path = directory.appendingPathComponent("origin.lock").path
        lockFD = open(path, O_RDWR | O_CREAT | O_NOFOLLOW, 0o600)
        guard lockFD >= 0 else { throw Failure.unsafeFile }
        guard fstat(lockFD, &st) == 0, (st.st_mode & S_IFMT) == S_IFREG,
              st.st_uid == getuid(), st.st_mode & 0o077 == 0 else { close(lockFD); lockFD = -1; throw Failure.unsafeFile }
        guard flock(lockFD, LOCK_EX | LOCK_NB) == 0 else { close(lockFD); lockFD = -1; throw Failure.busy }
    }
    deinit { if lockFD >= 0 { flock(lockFD, LOCK_UN); close(lockFD) } }
    func loadPort() throws -> UInt16? {
        let path = directory.appendingPathComponent("origin.port").path
        let fd = open(path, O_RDONLY | O_NOFOLLOW)
        if fd < 0 { if errno == ENOENT { return nil }; throw Failure.unsafeFile }
        defer { close(fd) }
        var st = stat()
        guard fstat(fd, &st) == 0, (st.st_mode & S_IFMT) == S_IFREG,
              st.st_uid == getuid(), st.st_mode & 0o077 == 0, st.st_size > 0, st.st_size <= 6 else { throw Failure.unsafeFile }
        var bytes = [UInt8](repeating: 0, count: 8)
        let count = read(fd, &bytes, bytes.count)
        guard count > 0, let value = String(bytes: bytes.prefix(count), encoding: .utf8),
              let port = UInt16(value.trimmingCharacters(in: .whitespacesAndNewlines)), port > 0 else { throw Failure.invalidPort }
        return port
    }
    func saveInitialPort(_ port: UInt16) throws {
        guard port > 0, try loadPort() == nil else { throw Failure.invalidPort }
        let path = directory.appendingPathComponent("origin.port").path
        let temporary = directory.appendingPathComponent(".origin-" + UUID().uuidString + ".tmp").path
        let fd = open(temporary, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0o600)
        guard fd >= 0 else { throw Failure.io }
        defer { close(fd); unlink(temporary) }
        let data = Array("\(port)\n".utf8)
        guard data.withUnsafeBytes({ write(fd, $0.baseAddress, data.count) }) == data.count,
              fsync(fd) == 0 else { throw Failure.io }
        // Hard-link publication is atomic and fails if the final name exists.
        // A crash before this step leaves only an unused private temporary file.
        guard link(temporary, path) == 0 else { throw Failure.io }
        let directoryFD = open(directory.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW)
        guard directoryFD >= 0 else { throw Failure.io }
        defer { close(directoryFD) }
        guard fsync(directoryFD) == 0 else { throw Failure.io }
    }
}
