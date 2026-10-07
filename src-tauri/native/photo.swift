import Foundation
import ImageIO
import Vision
import UniformTypeIdentifiers

// System decoders and local Vision requests only. Never infer capture identity/date.
do {
    let url = URL(fileURLWithPath: CommandLine.arguments[1])
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [String: Any],
          let width = properties[kCGImagePropertyPixelWidth as String] as? Int,
          let height = properties[kCGImagePropertyPixelHeight as String] as? Int,
          width > 0, height > 0, Double(width) * Double(height) <= 50_000_000 else {
        throw NSError(domain: "Photo", code: 1, userInfo: [NSLocalizedDescriptionKey: "Unreadable image or image exceeds the 50 megapixel decoder limit."])
    }
    let options: [CFString: Any] = [kCGImageSourceCreateThumbnailFromImageAlways: true,
        kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 2048]
    guard let image = CGImageSourceCreateThumbnailAtIndex(source, 0, options as CFDictionary) else {
        throw NSError(domain: "Photo", code: 2, userInfo: [NSLocalizedDescriptionKey: "The image could not be decoded."])
    }
    if CommandLine.arguments.count > 2 && CommandLine.arguments[2] == "preview" {
        let bytes = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(bytes, UTType.jpeg.identifier as CFString, 1, nil) else { throw NSError(domain: "Photo", code: 3) }
        CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { throw NSError(domain: "Photo", code: 4) }
        FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: ["data_url": "data:image/jpeg;base64," + (bytes as Data).base64EncodedString()]))
    } else {
        var gaps: [String] = []
        var metadata: [[String: String]] = []
        metadata.append(["key": "stored orientation", "value": String(describing: properties[kCGImagePropertyOrientation as String] ?? 1), "channel": "file_metadata"])
        for (group, label) in [(kCGImagePropertyExifDictionary, "EXIF"),
                               (kCGImagePropertyTIFFDictionary, "TIFF"),
                               (kCGImagePropertyIPTCDictionary, "IPTC"),
                               (kCGImagePropertyGPSDictionary, "GPS")] {
            if let values = properties[group as String] as? [String: Any] {
                for key in values.keys.sorted() {
                    if let value = values[key] {
                        if metadata.count >= 64 { gaps.append("Metadata exceeds the 64 field projection limit."); break }
                        let raw = String(describing: value)
                        if raw.utf8.count > 4096 { gaps.append("Metadata field \(key) exceeds the 4096 byte field limit."); continue }
                        metadata.append(["key": "\(label) \(key)", "value": raw, "channel": group == kCGImagePropertyIPTCDictionary && key == kCGImagePropertyIPTCCaptionAbstract as String ? "supplied_caption" : "file_metadata"])
                    }
                }
            }
        }
        var rows: [[String: Any]] = []
        let text = VNRecognizeTextRequest()
        text.recognitionLevel = .accurate
        text.usesLanguageCorrection = false
        do {
            try VNImageRequestHandler(cgImage: image).perform([text])
            let observations = text.results ?? []
            if observations.count > 256 { gaps.append("OCR exceeds the 256 region limit; the projection is partial.") }
            rows = observations.prefix(256).compactMap { row in
                guard let candidate = row.topCandidates(1).first else { return nil }
                let box = row.boundingBox
                return ["text": candidate.string, "confidence": candidate.confidence, "box": [box.minX, box.minY, box.width, box.height]]
            }
        } catch { gaps.append("Local OCR request failed: \(error.localizedDescription)") }
        let categories = VNClassifyImageRequest()
        var classifications: [[String: Any]] = []
        do {
            try VNImageRequestHandler(cgImage: image).perform([categories])
            classifications = (categories.results ?? []).prefix(10).map { ["label": $0.identifier, "confidence": $0.confidence] }
        } catch { gaps.append("Local classification request failed: \(error.localizedDescription)") }
        let output: [String: Any] = ["width": width, "height": height,
            "orientation": properties[kCGImagePropertyOrientation as String] ?? 1,
            "metadata": metadata, "ocr": rows, "classifications": classifications, "gaps": gaps,
            "ocr_revision": text.revision, "classification_revision": categories.revision]
        FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: output, options: [.sortedKeys]))
    }
} catch {
    FileHandle.standardError.write(Data(error.localizedDescription.utf8))
    exit(1)
}
