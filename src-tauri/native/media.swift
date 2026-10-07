import AVFoundation
import Foundation
import Speech

private struct CapabilityReport: Encodable {
    let schema: Int
    let locale: String
    let api: String
    let moduleAvailable: Bool
    let assetStatus: String
    let detail: String
}

private struct TranscriptSegment: Encodable {
    let startMs: Int64
    let endMs: Int64
    let text: String
    let confidence: Double?
    let alternatives: [String]
    let speaker: String?
    let speakerState: String
    let final: Bool
}

private struct TranscriptionReport: Encodable {
    let schema: Int
    let requestedStartMs: Int64
    let requestedDurationMs: Int64
    let processedDurationMs: Int64
    let mediaDurationMs: Int64
    let state: String
    let coverage: String
    let detail: String
    let segments: [TranscriptSegment]
}

private struct AudioInfo: Encodable {
    let schema: Int
    let format: String
    let durationMs: Int64
    let sampleRate: Double
    let channels: Int
    let codec: String
}

private struct ExtractedAudioSegment: Encodable {
    let processedDurationMs: Int64
    let mediaDurationMs: Int64
}

private enum MediaError: Error, CustomStringConvertible {
    case usage
    case invalidRange
    case unsupportedLocale
    case assetsUnavailable(String)
    case invalidAudio(String)

    var description: String {
        switch self {
        case .usage:
                return "Usage: media-helper capabilities | install | inspect <path> | extract <path> <start-ms> <duration-ms> <wav-path> | transcribe <path> <start-ms> <duration-ms>"
        case .invalidRange:
            return "The requested audio interval is outside the source or exceeds the 30-second processing limit."
        case .unsupportedLocale:
            return "The on-device SpeechTranscriber does not support the en_US locale on this Mac."
        case .assetsUnavailable(let status):
            return "The local speech model is not installed (asset status: \(status))."
        case .invalidAudio(let detail):
            return "The audio segment could not be decoded: \(detail)"
        }
    }
}

private let maxSegmentDurationMs: Int64 = 30_000
private let readBufferDurationSeconds: Double = 2

@main
private struct MediaHelper {
    static func main() async {
        do {
            let arguments = Array(CommandLine.arguments.dropFirst())
            guard let command = arguments.first else { throw MediaError.usage }

            switch command {
            case "capabilities":
                try await writeJSON(capabilityReport())
            case "install":
                let report = try await installSpeechAssets()
                try await writeJSON(report)
            case "inspect":
                guard arguments.count == 2 else { throw MediaError.usage }
                try await writeJSON(try inspectAudio(URL(fileURLWithPath: arguments[1])))
            case "extract":
                guard arguments.count == 5,
                      let startMs = Int64(arguments[2]),
                      let durationMs = Int64(arguments[3]) else { throw MediaError.usage }
                let report = try extractAudioSegment(
                    URL(fileURLWithPath: arguments[1]),
                    startMs: startMs,
                    durationMs: durationMs,
                    outputURL: URL(fileURLWithPath: arguments[4])
                )
                try await writeJSON(report)
            case "transcribe":
                guard arguments.count == 4,
                      let startMs = Int64(arguments[2]),
                      let durationMs = Int64(arguments[3]) else {
                    throw MediaError.usage
                }
                let report = try await transcribe(
                    URL(fileURLWithPath: arguments[1]),
                    startMs: startMs,
                    durationMs: durationMs
                )
                try await writeJSON(report)
            default:
                throw MediaError.usage
            }
        } catch {
            let failure: [String: String] = ["error": String(describing: error)]
            if let data = try? JSONSerialization.data(withJSONObject: failure, options: [.sortedKeys]),
               let text = String(data: data, encoding: .utf8) {
                FileHandle.standardError.write(Data((text + "\n").utf8))
            }
            exit(1)
        }
    }

    private static func transcriber() async throws -> SpeechTranscriber {
        guard let locale = await SpeechTranscriber.supportedLocale(equivalentTo: Locale(identifier: "en_US")) else {
            throw MediaError.unsupportedLocale
        }
        return SpeechTranscriber(
            locale: locale,
            transcriptionOptions: [],
            reportingOptions: [.alternativeTranscriptions],
            attributeOptions: [.audioTimeRange, .transcriptionConfidence]
        )
    }

    private static func assetStatus(for transcriber: SpeechTranscriber) async -> String {
        String(describing: await AssetInventory.status(forModules: [transcriber]))
    }

    private static func capabilityReport() async throws -> CapabilityReport {
        let transcriber = try await transcriber()
        let status = await assetStatus(for: transcriber)
        let installed = status == "installed"
        return CapabilityReport(
            schema: 1,
            locale: "en_US",
            api: "SpeechAnalyzer/SpeechTranscriber",
            moduleAvailable: SpeechTranscriber.isAvailable,
            assetStatus: status,
            detail: installed
                ? "The local transcription module and its required model assets are installed."
                : "The module is supported, but the local model assets are not installed. The original recording remains usable while setup or transcription is pending."
        )
    }

    private static func installSpeechAssets() async throws -> CapabilityReport {
        let transcriber = try await transcriber()
        let before = await assetStatus(for: transcriber)
        guard before != "unsupported" else {
            throw MediaError.assetsUnavailable(before)
        }
        if before != "installed" {
            guard let request = try await AssetInventory.assetInstallationRequest(supporting: [transcriber]) else {
                throw MediaError.assetsUnavailable(before)
            }
            try await request.downloadAndInstall()
        }
        return try await capabilityReport()
    }

    private static func inspectAudio(_ url: URL) throws -> AudioInfo {
        let file = try AVAudioFile(forReading: url)
        let sampleRate = file.fileFormat.sampleRate
        guard sampleRate.isFinite, sampleRate > 0 else {
            throw MediaError.invalidAudio("The source has no usable audio sample rate.")
        }
        let durationMs = Int64((Double(file.length) / sampleRate * 1_000).rounded())
        return AudioInfo(
            schema: 1,
            format: url.pathExtension.lowercased(),
            durationMs: durationMs,
            sampleRate: sampleRate,
            channels: Int(file.fileFormat.channelCount),
            codec: file.fileFormat.settings[AVFormatIDKey].map(String.init(describing:)) ?? "unknown"
        )
    }

    private static func extractAudioSegment(
        _ url: URL,
        startMs: Int64,
        durationMs: Int64,
        outputURL: URL
    ) throws -> ExtractedAudioSegment {
        guard startMs >= 0, durationMs > 0, durationMs <= maxSegmentDurationMs else {
            throw MediaError.invalidRange
        }
        let input: AVAudioFile
        do { input = try AVAudioFile(forReading: url) }
        catch { throw MediaError.invalidAudio(error.localizedDescription) }
        let format = input.processingFormat
        let sampleRate = format.sampleRate
        guard sampleRate.isFinite, sampleRate > 0 else {
            throw MediaError.invalidAudio("The source has no usable audio sample rate.")
        }
        let totalDurationMs = Int64((Double(input.length) / sampleRate * 1_000).rounded())
        let startFrame = AVAudioFramePosition((Double(startMs) * sampleRate / 1_000).rounded(.down))
        guard startFrame < input.length else { throw MediaError.invalidRange }
        let requestedFrames = AVAudioFramePosition((Double(durationMs) * sampleRate / 1_000).rounded(.down))
        let frameCount = min(requestedFrames, input.length - startFrame)
        guard frameCount > 0 else { throw MediaError.invalidRange }
        let actualDurationMs = Int64((Double(frameCount) / sampleRate * 1_000).rounded())
        try copySegment(from: input, startFrame: startFrame, frameCount: frameCount, to: outputURL)
        return ExtractedAudioSegment(processedDurationMs: actualDurationMs, mediaDurationMs: totalDurationMs)
    }

    private static func transcribe(_ url: URL, startMs: Int64, durationMs: Int64) async throws -> TranscriptionReport {
        guard startMs >= 0, durationMs > 0, durationMs <= maxSegmentDurationMs else {
            throw MediaError.invalidRange
        }
        let file: AVAudioFile
        do {
            file = try AVAudioFile(forReading: url)
        } catch {
            throw MediaError.invalidAudio(error.localizedDescription)
        }

        let inputFormat = file.processingFormat
        let sampleRate = inputFormat.sampleRate
        guard sampleRate.isFinite, sampleRate > 0 else {
            throw MediaError.invalidAudio("The source has no usable audio sample rate.")
        }
        let totalFrames = file.length
        let durationSeconds = Double(totalFrames) / sampleRate
        let totalDurationMs = Int64((durationSeconds * 1_000).rounded())
        let startFrame = AVAudioFramePosition((Double(startMs) * sampleRate / 1_000).rounded(.down))
        guard startFrame < totalFrames else { throw MediaError.invalidRange }
        let requestedFrames = AVAudioFramePosition((Double(durationMs) * sampleRate / 1_000).rounded(.down))
        let frameCount = min(requestedFrames, totalFrames - startFrame)
        guard frameCount > 0 else { throw MediaError.invalidRange }
        let actualDurationMs = Int64((Double(frameCount) / sampleRate * 1_000).rounded())

        let speechTranscriber = try await transcriber()
        let status = await assetStatus(for: speechTranscriber)
        guard status == "installed" else { throw MediaError.assetsUnavailable(status) }

        let segmentURL = FileManager.default.temporaryDirectory
            .appendingPathComponent("knowledge-garden-audio-\(UUID().uuidString).caf")
        defer { try? FileManager.default.removeItem(at: segmentURL) }
        try copySegment(from: file, startFrame: startFrame, frameCount: frameCount, to: segmentURL)
        let segmentFile = try AVAudioFile(forReading: segmentURL)
        let analyzer = SpeechAnalyzer(modules: [speechTranscriber])
        let collected = Task<[TranscriptSegment], Error> {
            var segments: [TranscriptSegment] = []
            for try await result in speechTranscriber.results {
                guard result.isFinal else { continue }
                let transcript = String(result.text.characters)
                let confidences = result.text.runs.compactMap {
                    $0[AttributeScopes.SpeechAttributes.ConfidenceAttribute.self]
                }
                let confidence = confidences.isEmpty ? nil : confidences.min()
                let start = CMTimeGetSeconds(result.range.start) + Double(startMs) / 1_000
                let end = CMTimeGetSeconds(result.range.end) + Double(startMs) / 1_000
                guard start.isFinite, end.isFinite, end >= start else { continue }
                let alternatives = result.alternatives
                    .map { String($0.characters) }
                    .filter { $0 != transcript }
                segments.append(TranscriptSegment(
                    startMs: Int64((start * 1_000).rounded()),
                    endMs: Int64((end * 1_000).rounded()),
                    text: transcript,
                    confidence: confidence,
                    alternatives: Array(alternatives.prefix(5)),
                    speaker: nil,
                    speakerState: "unidentified",
                    final: true
                ))
            }
            return segments
        }

        do {
            let lastSample = try await analyzer.analyzeSequence(from: segmentFile)
            if let lastSample {
                try await analyzer.finalizeAndFinish(through: lastSample)
            } else {
                await analyzer.cancelAndFinishNow()
            }
            let segments = try await collected.value
            return TranscriptionReport(
                schema: 1,
                requestedStartMs: startMs,
                requestedDurationMs: durationMs,
                processedDurationMs: actualDurationMs,
                mediaDurationMs: totalDurationMs,
                state: segments.isEmpty ? "no_recognized_speech" : "complete",
                coverage: "partial",
                detail: segments.isEmpty
                    ? "No recognizable speech was returned for this interval. Silence, noise, overlap, and unintelligible speech are not distinguished by this result. Speaker identity remains unknown."
                    : "On-device speech recognition returned a best transcription and alternatives. Text, timing, and confidence are machine-generated interpretations. Speakers are not identified; overlap and noise are not independently classified. Compare any uncertain wording with the retained original.",
                segments: segments
            )
        } catch {
            await analyzer.cancelAndFinishNow()
            throw error
        }
    }

    private static func copySegment(
        from input: AVAudioFile,
        startFrame: AVAudioFramePosition,
        frameCount: AVAudioFramePosition,
        to outputURL: URL
    ) throws {
        let format = input.processingFormat
        let output = try AVAudioFile(
            forWriting: outputURL,
            settings: format.settings,
            commonFormat: format.commonFormat,
            interleaved: format.isInterleaved
        )
        input.framePosition = startFrame
        var remainingFrames = frameCount
        let sampleRate = format.sampleRate
        let maximumBufferFrames = AVAudioFrameCount((readBufferDurationSeconds * sampleRate).rounded(.down))
        while remainingFrames > 0 {
            let nextFrames = AVAudioFrameCount(min(remainingFrames, AVAudioFramePosition(maximumBufferFrames)))
            guard let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: nextFrames) else {
                throw MediaError.invalidAudio("A bounded processing buffer could not be allocated.")
            }
            try input.read(into: buffer, frameCount: nextFrames)
            guard buffer.frameLength > 0 else {
                throw MediaError.invalidAudio("The selected audio segment ended before its recorded duration.")
            }
            try output.write(from: buffer)
            remainingFrames -= AVAudioFramePosition(buffer.frameLength)
        }
    }

    private static func writeJSON<T: Encodable>(_ value: T) async throws {
        let data = try JSONEncoder().encode(value)
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data([0x0a]))
    }
}
