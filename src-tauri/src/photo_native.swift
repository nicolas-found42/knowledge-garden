import Foundation
import ImageIO
import Vision
import UniformTypeIdentifiers

struct OCRItem: Codable {
    let text: String
    let confidence: Float
    let x: Double
    let y: Double
    let width: Double
    let height: Double
}
struct Inspection: Codable {
    let width: Int
    let height: Int
    let captureDate: String?
    let caption: String?
    let ocr: [OCRItem]
}

func fail(_ message: String) -> Never {
    fputs(message + "\n", stderr)
    exit(2)
}
guard CommandLine.arguments.count == 3 else { fail("Expected operation and input path.") }
let operation = CommandLine.arguments[1]
let input = URL(fileURLWithPath: CommandLine.arguments[2])
guard let source = CGImageSourceCreateWithURL(input as CFURL, nil),
      CGImageSourceGetCount(source) > 0,
      let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
    fail("ImageIO could not decode an image from this file.")
}
let width = image.width
let height = image.height
guard width > 0, height > 0, width <= 50_000, height <= 50_000,
      width.multipliedReportingOverflow(by: height).overflow == false,
      width * height <= 100_000_000 else {
    fail("Image dimensions exceed the 100-megapixel inspection limit.")
}
let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any] ?? [:]
let exif = props[kCGImagePropertyExifDictionary] as? [CFString: Any] ?? [:]
let tiff = props[kCGImagePropertyTIFFDictionary] as? [CFString: Any] ?? [:]
let iptc = props[kCGImagePropertyIPTCDictionary] as? [CFString: Any] ?? [:]
let capture = (exif[kCGImagePropertyExifDateTimeOriginal] as? String)
    ?? (exif[kCGImagePropertyExifDateTimeDigitized] as? String)
    ?? (tiff[kCGImagePropertyTIFFDateTime] as? String)
let caption = (iptc[kCGImagePropertyIPTCCaptionAbstract] as? String)
    ?? (props[kCGImagePropertyPNGDescription] as? String)
    ?? (tiff[kCGImagePropertyTIFFImageDescription] as? String)
if operation == "inspect" {
    var request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    let handler = VNImageRequestHandler(cgImage: image)
    do { try handler.perform([request]) } catch { fail("Vision OCR failed: \(error)") }
    let observations = request.results ?? []
    let lines: [OCRItem] = observations.compactMap { observation in
        guard let candidate = observation.topCandidates(1).first,
              !candidate.string.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else { return nil }
        let box = observation.boundingBox
        return OCRItem(text: candidate.string, confidence: candidate.confidence,
                       x: box.origin.x, y: box.origin.y, width: box.width, height: box.height)
    }
    let result = Inspection(width: width, height: height, captureDate: capture,
                            caption: caption, ocr: lines)
    do { FileHandle.standardOutput.write(try JSONEncoder().encode(result)); print("") }
    catch { fail("Could not encode image inspection: \(error)") }
} else if operation == "preview" {
    let options: [CFString: Any] = [
        kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceThumbnailMaxPixelSize: 1800,
        kCGImageSourceCreateThumbnailWithTransform: true
    ]
    let outputData = NSMutableData()
    guard let preview = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary),
          let destination = CGImageDestinationCreateWithData(outputData, UTType.jpeg.identifier as CFString, 1, nil) else {
        fail("ImageIO could not create a preview.")
    }
    CGImageDestinationAddImage(destination, preview, [kCGImageDestinationLossyCompressionQuality: 0.86] as CFDictionary)
    guard CGImageDestinationFinalize(destination) else { fail("Preview encoding failed.") }
    let output = outputData as Data
    print(output.base64EncodedString())
} else {
    fail("Unsupported image operation.")
}
