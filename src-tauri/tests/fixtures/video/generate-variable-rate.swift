// Rebuild only the synthetic silent variable-frame-rate fixture; no acquired media.
// swift generate-variable-rate.swift /absolute/path/variable-frame-rate.mov
import AVFoundation
import Foundation
import CoreVideo
import CoreGraphics

let destination = URL(fileURLWithPath: CommandLine.arguments[1])
let writer = try AVAssetWriter(outputURL: destination, fileType: .mov)
let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
    AVVideoCodecKey: AVVideoCodecType.h264,
    AVVideoWidthKey: 320, AVVideoHeightKey: 240
])
let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: [
    kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32ARGB,
    kCVPixelBufferWidthKey as String: 320, kCVPixelBufferHeightKey as String: 240
])
writer.add(input)
guard writer.startWriting() else { fatalError(String(describing: writer.error)) }
writer.startSession(atSourceTime: .zero)
for (index, ms) in [0, 100, 300, 400, 800, 1000, 1600].enumerated() {
    let deadline = Date().addingTimeInterval(10)
    while !input.isReadyForMoreMediaData && Date() < deadline { Thread.sleep(forTimeInterval: 0.01) }
    guard input.isReadyForMoreMediaData else { fatalError("Encoder did not become ready") }
    var buffer: CVPixelBuffer?
    guard CVPixelBufferPoolCreatePixelBuffer(nil, adaptor.pixelBufferPool!, &buffer) == kCVReturnSuccess else { fatalError("Pixel buffer") }
    let pixels = buffer!
    CVPixelBufferLockBaseAddress(pixels, [])
    let context = CGContext(data: CVPixelBufferGetBaseAddress(pixels), width: 320, height: 240, bitsPerComponent: 8, bytesPerRow: CVPixelBufferGetBytesPerRow(pixels), space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipFirst.rawValue)!
    context.setFillColor(CGColor(red: CGFloat(index) / 8, green: 0.3, blue: 0.7, alpha: 1))
    context.fill(CGRect(x: 0, y: 0, width: 320, height: 240))
    CVPixelBufferUnlockBaseAddress(pixels, [])
    guard adaptor.append(pixels, withPresentationTime: CMTime(value: Int64(ms), timescale: 1000)) else { fatalError(String(describing: writer.error)) }
}
input.markAsFinished()
let finished = DispatchSemaphore(value: 0)
writer.finishWriting { finished.signal() }
guard finished.wait(timeout: .now() + 30) == .success, writer.status == .completed else { fatalError(String(describing: writer.error)) }
print(destination.path)
