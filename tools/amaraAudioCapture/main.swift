import Foundation
import CoreAudio
import CoreMedia
import CoreGraphics
import ScreenCaptureKit
import Darwin

// stdout is exclusively mono float32 little-endian PCM at 48,000 Hz. Lifecycle
// diagnostics are compact JSON on stderr and never identify the playing music.
func diagnostic(_ backend: String, _ state: String, category: String? = nil,
                detail: String? = nil, code: Int? = nil, sourceRate: Double? = nil) {
    var values: [String: Any] = ["backend": backend, "state": state, "rate": 48_000, "channels": 1]
    if let category { values["category"] = category }
    if let detail { values["detail"] = String(detail.prefix(800)) }
    if let code { values["code"] = code }
    if let sourceRate { values["source_rate"] = sourceRate }
    if let bytes = try? JSONSerialization.data(withJSONObject: values, options: [.sortedKeys]),
       let text = String(data: bytes, encoding: .utf8) {
        FileHandle.standardError.write(Data("AMARA_CAPTURE \(text)\n".utf8))
    }
}

enum CaptureFailure: Error, LocalizedError {
    case status(String, OSStatus)
    case unsupported(String)
    case transport(String)

    var category: String {
        switch self {
        case .status(_, let code):
            // Do not mislabel generic 'nope'/illegal-operation errors as TCC.
            return code == kAudioDevicePermissionsError || code == -54 || code == OSStatus(0x7065726d)
                ? "permission" : "transport"
        case .unsupported: return "unsupported"
        case .transport: return "transport"
        }
    }

    var code: Int? {
        if case .status(_, let code) = self { return Int(code) }
        return nil
    }

    var errorDescription: String? {
        switch self {
        case .status(let operation, let code):
            return "\(operation) failed (OSStatus \(code)). System audio access may need permission in Privacy & Security."
        case .unsupported(let detail), .transport(let detail): return detail
        }
    }
}

func checked(_ code: OSStatus, _ operation: String) throws {
    if code != noErr { throw CaptureFailure.status(operation, code) }
}

func clean(_ sample: Float) -> Float {
    sample.isFinite ? min(1, max(-1, sample)) : 0
}

// Resampling belongs to the pipe worker, never the HAL callback. Linear
// interpolation is sufficient for analysis input; this is not an audio player.
final class MonoResampler {
    private let ratio: Double
    private var position = 0.0
    private var pending: [Float] = []

    init(sourceRate: Double) {
        ratio = sourceRate / 48_000
        pending.reserveCapacity(8_192)
    }

    func convert(_ input: [Float]) -> [Float] {
        if abs(ratio - 1) < 0.000_001 { return input }
        pending.append(contentsOf: input)
        var output: [Float] = []
        output.reserveCapacity(Int(Double(input.count) / ratio) + 2)
        while position + 1 < Double(pending.count) {
            let index = Int(position)
            let fraction = Float(position - Double(index))
            output.append(pending[index] + (pending[index + 1] - pending[index]) * fraction)
            position += ratio
        }
        let consumed = min(Int(position), max(0, pending.count - 1))
        if consumed > 0 { pending.removeFirst(consumed); position -= Double(consumed) }
        return output
    }
}

// One bounded, preallocated mono ring separates realtime capture from pipe
// writes and resampling. The callback never waits for a lock or stdout, creates
// no arrays and retains no AudioBufferList pointers beyond the call.
final class PCMBridge: @unchecked Sendable {
    private let lock = NSLock()
    private var ring = [Float](repeating: 0, count: 96_000)
    private var readIndex = 0
    private var writeIndex = 0
    private var count = 0
    private var invalidLayout = false
    private var stopped = false
    private var writerStarted = false
    private let writerFinished = DispatchSemaphore(value: 0)
    private let channels: Int
    let sampleRate: Double
    private let backend: String

    init(format: AudioStreamBasicDescription, backend: String) throws {
        guard format.mFormatID == kAudioFormatLinearPCM,
              format.mFormatFlags & kAudioFormatFlagIsFloat != 0,
              format.mFormatFlags & kAudioFormatFlagIsBigEndian == 0,
              format.mBitsPerChannel == 32,
              format.mSampleRate.isFinite,
              (8_000...192_000).contains(format.mSampleRate),
              (1...32).contains(format.mChannelsPerFrame) else {
            throw CaptureFailure.unsupported("Capture requires little-endian float32 PCM, 1–32 channels and an 8–192 kHz source rate.")
        }
        channels = Int(format.mChannelsPerFrame)
        sampleRate = format.mSampleRate
        self.backend = backend
    }

    @discardableResult
    func enqueue(_ list: UnsafePointer<AudioBufferList>, frames requestedFrames: Int? = nil) -> Bool {
        guard lock.try() else { return false }
        defer { lock.unlock() }
        guard !stopped else { return false }
        let buffers = UnsafeMutableAudioBufferListPointer(UnsafeMutablePointer(mutating: list))
        guard !buffers.isEmpty, buffers.count <= 32 else { invalidLayout = true; return false }
        var frames = requestedFrames ?? Int.max
        var presentChannels = 0
        for buffer in buffers {
            let n = Int(buffer.mNumberChannels)
            guard n > 0, n <= channels else { invalidLayout = true; return false }
            let available = Int(buffer.mDataByteSize) / MemoryLayout<Float>.size / n
            if available == 0 { return false }
            guard buffer.mData != nil else { invalidLayout = true; return false }
            frames = min(frames, available)
            presentChannels += n
        }
        guard frames > 0, frames <= 8_192, presentChannels == channels,
              requestedFrames == nil || frames == requestedFrames else {
            invalidLayout = true; return false
        }
        for frame in 0..<frames {
            var mono: Float = 0
            for buffer in buffers {
                let samples = buffer.mData!.assumingMemoryBound(to: Float.self)
                let n = Int(buffer.mNumberChannels)
                for channel in 0..<n { mono += clean(samples[frame * n + channel]) }
            }
            if count == ring.count { readIndex = (readIndex + 1) % ring.count; count -= 1 }
            ring[writeIndex] = mono / Float(channels)
            writeIndex = (writeIndex + 1) % ring.count
            count += 1
        }
        return true
    }

    func takeBlock() -> [Float] {
        lock.lock()
        defer { lock.unlock() }
        let available = min(count, 4_096)
        var block = [Float](repeating: 0, count: available)
        for index in 0..<available {
            block[index] = ring[readIndex]
            readIndex = (readIndex + 1) % ring.count
        }
        count -= available
        return block
    }

    func startWriter() {
        lock.lock()
        guard !writerStarted, !stopped else { lock.unlock(); return }
        writerStarted = true
        lock.unlock()
        Thread.detachNewThread { [self] in
            defer { writerFinished.signal() }
            let resampler = MonoResampler(sourceRate: sampleRate)
            while true {
                lock.lock(); let failed = invalidLayout; let cancelled = stopped; lock.unlock()
                if cancelled { return }
                if failed {
                    diagnostic(backend, "error", category: "unsupported", detail: "The capture callback returned an unsupported PCM buffer layout.")
                    exit(3)
                }
                let input = takeBlock()
                if input.isEmpty { Thread.sleep(forTimeInterval: 0.005); continue }
                let mono = resampler.convert(input)
                mono.withUnsafeBytes { bytes in
                    var offset = 0
                    while offset < bytes.count {
                        let written = Darwin.write(STDOUT_FILENO, bytes.baseAddress!.advanced(by: offset), bytes.count - offset)
                        if written > 0 { offset += written }
                        else if written < 0 && errno == EINTR { continue }
                        else { exit(0) } // Parent closed its pipe; private HAL objects die with us.
                    }
                }
            }
        }
    }

    func stopWriter() {
        lock.lock()
        let started = writerStarted
        stopped = true
        count = 0
        lock.unlock()
        if started { _ = writerFinished.wait(timeout: .now() + 0.25) }
    }
}

@available(macOS 14.2, *)
final class CoreAudioCapture {
    private var tap = AudioObjectID(kAudioObjectUnknown)
    private var aggregate = AudioObjectID(kAudioObjectUnknown)
    private var ioProc: AudioDeviceIOProcID?
    private var bridge: PCMBridge?
    private var format = AudioStreamBasicDescription()

    func start() throws {
        diagnostic("coreaudio", "starting", detail: "Connecting to Mac playback; macOS asks for audio access only when needed.")
        let description = CATapDescription(stereoGlobalTapButExcludeProcesses: [])
        description.name = "amaraMusicVisualiser playback"
        description.isPrivate = true
        description.muteBehavior = .unmuted
        try checked(AudioHardwareCreateProcessTap(description, &tap), "Create system audio tap")

        var tapAddress = AudioObjectPropertyAddress(mSelector: kAudioTapPropertyUID,
            mScope: kAudioObjectPropertyScopeGlobal, mElement: kAudioObjectPropertyElementMain)
        var uid: Unmanaged<CFString>?
        var uidSize = UInt32(MemoryLayout<Unmanaged<CFString>?>.stride)
        let uidStatus = withUnsafeMutablePointer(to: &uid) { pointer in
            AudioObjectGetPropertyData(tap, &tapAddress, 0, nil, &uidSize, UnsafeMutableRawPointer(pointer))
        }
        try checked(uidStatus, "Read tap UID")
        guard let uid else { throw CaptureFailure.transport("The system audio tap returned no UID.") }
        let tapUID = uid.takeRetainedValue()
        // Tap-only composition: never include a hardware input, change the
        // default output, mute playback or publish a persistent virtual device.
        let composition: [String: Any] = [
            kAudioAggregateDeviceNameKey: "amaraMusicVisualiser private capture",
            kAudioAggregateDeviceUIDKey: UUID().uuidString,
            kAudioAggregateDeviceIsPrivateKey: true,
            kAudioAggregateDeviceIsStackedKey: false,
            kAudioAggregateDeviceTapAutoStartKey: false,
            kAudioAggregateDeviceTapListKey: [[
                kAudioSubTapUIDKey: tapUID,
                kAudioSubTapDriftCompensationKey: true
            ]]
        ]
        try checked(AudioHardwareCreateAggregateDevice(composition as CFDictionary, &aggregate), "Create private capture device")
        format = try inputFormat()
        let bridge = try PCMBridge(format: format, backend: "coreaudio")
        self.bridge = bridge
        let queue = DispatchQueue(label: "com.amara.musicvisualiser.tap", qos: .userInteractive)
        diagnostic("coreaudio", "starting", detail: "Audio-only tap is ready; connecting the playback stream. Complete any macOS audio-access alert if shown.")
        try checked(AudioDeviceCreateIOProcIDWithBlock(&ioProc, aggregate, queue) { _, input, _, _, _ in
            bridge.enqueue(input)
        }, "Register capture callback")
        try checked(AudioDeviceStart(aggregate, ioProc), "Start audio-only system capture")
        bridge.startWriter()
        diagnostic("coreaudio", "running", detail: "Audio-only engine started; only PCM signal levels prove audible input.", sourceRate: format.mSampleRate)
    }

    private func inputFormat() throws -> AudioStreamBasicDescription {
        var address = AudioObjectPropertyAddress(mSelector: kAudioDevicePropertyStreamFormat,
            mScope: kAudioDevicePropertyScopeInput, mElement: kAudioObjectPropertyElementMain)
        var value = AudioStreamBasicDescription()
        var size = UInt32(MemoryLayout<AudioStreamBasicDescription>.stride)
        try checked(AudioObjectGetPropertyData(aggregate, &address, 0, nil, &size, &value), "Read private capture PCM format")
        return value
    }

    func validateFormat() throws {
        let current = try inputFormat()
        guard current.mSampleRate == format.mSampleRate,
              current.mChannelsPerFrame == format.mChannelsPerFrame,
              current.mBitsPerChannel == format.mBitsPerChannel,
              current.mFormatFlags == format.mFormatFlags else {
            throw CaptureFailure.transport("The playback device format changed. Select Mac audio again to reconnect safely.")
        }
    }

    func stop() {
        if let ioProc, aggregate != kAudioObjectUnknown {
            AudioDeviceStop(aggregate, ioProc)
            AudioDeviceDestroyIOProcID(aggregate, ioProc)
        }
        ioProc = nil
        if aggregate != kAudioObjectUnknown { AudioHardwareDestroyAggregateDevice(aggregate) }
        aggregate = AudioObjectID(kAudioObjectUnknown)
        if tap != kAudioObjectUnknown { AudioHardwareDestroyProcessTap(tap) }
        tap = AudioObjectID(kAudioObjectUnknown)
        bridge?.stopWriter()
        bridge = nil
    }

    deinit { stop() }
}

@available(macOS 13.0, *)
final class ScreenAudioOutput: NSObject, SCStreamOutput, SCStreamDelegate {
    private let bridge: PCMBridge

    init(bridge: PCMBridge) { self.bridge = bridge }

    func stream(_ stream: SCStream, didOutputSampleBuffer sample: CMSampleBuffer,
                of outputType: SCStreamOutputType) {
        guard outputType == .audio, sample.isValid, sample.numSamples > 0 else { return }
        guard let description = sample.formatDescription,
              let format = CMAudioFormatDescriptionGetStreamBasicDescription(description)?.pointee,
              format.mFormatID == kAudioFormatLinearPCM,
              format.mFormatFlags & kAudioFormatFlagIsFloat != 0,
              format.mBitsPerChannel == 32,
              abs(format.mSampleRate - bridge.sampleRate) < 1,
              format.mChannelsPerFrame == 2 else {
            diagnostic("screencapturekit", "error", category: "unsupported", detail: "Legacy system capture returned a different PCM format.")
            exit(3)
        }
        do {
            try sample.withAudioBufferList { buffers, _ in
                _ = bridge.enqueue(UnsafePointer(buffers.unsafeMutablePointer), frames: sample.numSamples)
            }
        } catch {
            diagnostic("screencapturekit", "error", category: "transport", detail: error.localizedDescription)
            exit(2)
        }
    }

    func stream(_ stream: SCStream, didStopWithError error: Error) {
        let value = error as NSError
        diagnostic("screencapturekit", "error", category: value.code == -3801 ? "permission" : "transport",
                   detail: error.localizedDescription, code: value.code)
        exit(2)
    }
}

@available(macOS 13.0, *)
func screenCapture() async throws {
    diagnostic("screencapturekit", "starting", detail: "Legacy audio capture uses existing screen-capture authorization.")
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    guard let display = content.displays.first else { throw CaptureFailure.transport("No display is available for legacy system capture.") }
    let filter = SCContentFilter(display: display, excludingApplications: [], exceptingWindows: [])
    let configuration = SCStreamConfiguration()
    configuration.capturesAudio = true
    configuration.excludesCurrentProcessAudio = true
    configuration.sampleRate = 48_000
    configuration.channelCount = 2
    configuration.width = 2
    configuration.height = 2
    configuration.minimumFrameInterval = CMTime(value: 1, timescale: 2)
    configuration.queueDepth = 1
    configuration.showsCursor = false
    let format = AudioStreamBasicDescription(mSampleRate: 48_000, mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 8, mFramesPerPacket: 1, mBytesPerFrame: 8,
        mChannelsPerFrame: 2, mBitsPerChannel: 32, mReserved: 0)
    let bridge = try PCMBridge(format: format, backend: "screencapturekit")
    let output = ScreenAudioOutput(bridge: bridge)
    let stream = SCStream(filter: filter, configuration: configuration, delegate: output)
    let queue = DispatchQueue(label: "com.amara.musicvisualiser.legacy-audio", qos: .userInteractive)
    try stream.addStreamOutput(output, type: .audio, sampleHandlerQueue: queue)
    try await stream.startCapture()
    bridge.startWriter()
    diagnostic("screencapturekit", "running", sourceRate: 48_000)
    while true { try await Task.sleep(nanoseconds: 1_000_000_000) }
}

func selfTest() throws {
    let rate = 44_100.0
    let source = (0..<4_410).map { Float(sin(Double($0) * 2 * .pi * 1_000 / rate)) }
    let whole = MonoResampler(sourceRate: rate).convert(source)
    let chunked = MonoResampler(sourceRate: rate)
    var parts: [Float] = []
    for start in stride(from: 0, to: source.count, by: 317) {
        parts += chunked.convert(Array(source[start..<min(source.count, start + 317)]))
    }
    guard abs(whole.count - 4_800) <= 2, parts.count == whole.count,
          zip(parts, whole).allSatisfy({ abs($0 - $1) < 0.000_1 }) else {
        throw CaptureFailure.transport("Self-test failed: streaming 44.1→48 kHz resampling.")
    }
    let format = AudioStreamBasicDescription(mSampleRate: 48_000, mFormatID: kAudioFormatLinearPCM,
        mFormatFlags: kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked,
        mBytesPerPacket: 8, mFramesPerPacket: 1, mBytesPerFrame: 8,
        mChannelsPerFrame: 2, mBitsPerChannel: 32, mReserved: 0)
    let bridge = try PCMBridge(format: format, backend: "self-test")
    var samples: [Float] = [1, -1, 0.8, 0.4, .nan, .infinity]
    let accepted = samples.withUnsafeMutableBytes { bytes in
        var list = AudioBufferList(mNumberBuffers: 1, mBuffers: AudioBuffer(
            mNumberChannels: 2, mDataByteSize: UInt32(bytes.count), mData: bytes.baseAddress))
        return bridge.enqueue(&list, frames: 3)
    }
    let mixed = bridge.takeBlock()
    guard accepted, mixed.count == 3, abs(mixed[0]) < 0.000_1,
          abs(mixed[1] - 0.6) < 0.000_1, mixed[2] == 0 else {
        throw CaptureFailure.transport("Self-test failed: bounded stereo downmix and invalid-sample sanitization.")
    }
    diagnostic("self-test", "passed", detail: "Streaming resampling, channel mixing and finite PCM checks passed; no capture API was invoked.")
}

@main
struct AmaraAudioCapture {
    static func main() async {
        signal(SIGPIPE, SIG_IGN)
        if CommandLine.arguments.contains("--self-test") {
            do { try selfTest(); return }
            catch { diagnostic("self-test", "error", category: "transport", detail: error.localizedDescription); exit(1) }
        }
        if #available(macOS 14.2, *) {
            let capture = CoreAudioCapture()
            do {
                try capture.start()
                while true {
                    try await Task.sleep(nanoseconds: 1_000_000_000)
                    try capture.validateFormat()
                }
            } catch {
                capture.stop()
                let failure = error as? CaptureFailure
                let category = failure?.category ?? "transport"
                // Never expand a denied audio-only request into a new screen
                // permission request. Legacy fallback uses an existing grant.
                if category != "permission" && CGPreflightScreenCaptureAccess() {
                    diagnostic("coreaudio", "fallback", category: category, detail: error.localizedDescription, code: failure?.code)
                } else {
                    diagnostic("coreaudio", "error", category: category, detail: error.localizedDescription, code: failure?.code)
                    exit(1)
                }
            }
        }
        if #available(macOS 13.0, *) {
            do { try await screenCapture() }
            catch {
                let value = error as NSError
                diagnostic("screencapturekit", "error", category: value.code == -3801 ? "permission" : "transport",
                           detail: error.localizedDescription, code: value.code)
                exit(1)
            }
        } else {
            diagnostic("none", "error", category: "unsupported", detail: "System playback capture requires macOS 13 or later.")
            exit(1)
        }
    }
}
