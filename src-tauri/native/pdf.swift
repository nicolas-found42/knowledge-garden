import Foundation
import PDFKit
import Vision

struct TextRegion {
    let text: String
    let channel: String
    let box: CGRect
    var confidence: Float? = nil
    var revision: Int? = nil

    var json: [String: Any] {
        var value: [String: Any] = ["text": text, "channel": channel,
            "box": [box.minX, box.minY, box.width, box.height]]
        if let confidence { value["confidence"] = confidence }
        if let revision { value["revision"] = revision }
        return value
    }
}

func describe(_ box: CGRect) -> String {
    String(format: "PDF points bottom-left x=%.3f,y=%.3f,width=%.3f,height=%.3f", box.minX, box.minY, box.width, box.height)
}

// Render the unrotated media box. Both PDFKit selections and OCR boxes then
// address original PDF points, independent of the viewer's /Rotate transform.
func project(_ page: PDFPage, number: Int) -> [String: Any] {
    let bounds = page.bounds(for: .mediaBox)
    var rows: [TextRegion] = []
    var gaps: [String] = []
    var unresolved = CGRect.null
    var recoveredInk: [CGRect] = []
    let text = page.string ?? ""
    let lines = page.selection(for: NSRange(location: 0, length: (text as NSString).length))?.selectionsByLine() ?? []
    if lines.count > 512 { gaps.append("Selectable text exceeds the 512-region page limit.") }
    for line in lines.prefix(512) {
        guard let value = line.string?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty else { continue }
        rows.append(TextRegion(text: value, channel: "pdf_text", box: line.bounds(for: page)))
    }
    if page.numberOfCharacters > 16384 { gaps.append("Selectable glyph coverage exceeds the 16384-character page bound.") }
    let characters = text as NSString
    for index in 0..<min(page.numberOfCharacters, min(characters.length, 16384)) {
        let character = characters.character(at: index)
        // A selectable space can span an inline image; only glyphs mask ink.
        if character > 32 && character != 160 {
            let box = page.characterBounds(at: index)
            if !box.isNull && !box.isEmpty { recoveredInk.append(box) }
        }
    }
    if bounds.minX.isFinite, bounds.minY.isFinite, bounds.width.isFinite, bounds.height.isFinite, bounds.width > 0, bounds.height > 0,
       let original = page.pageRef {
        let scale = min(3, 2400 / max(bounds.width, bounds.height))
        if scale < 3 { gaps.append("Page exceeds the full-resolution OCR raster bound; downsampled coverage remains partial.") }
        let width = max(1, Int(ceil(bounds.width * scale)))
        let height = max(1, Int(ceil(bounds.height * scale)))
        if let context = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8,
                                   bytesPerRow: width * 4, space: CGColorSpaceCreateDeviceRGB(),
                                   bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue) {
            context.setFillColor(CGColor(gray: 1, alpha: 1))
            context.fill(CGRect(x: 0, y: 0, width: width, height: height))
            context.scaleBy(x: CGFloat(width) / bounds.width, y: CGFloat(height) / bounds.height)
            context.translateBy(x: -bounds.minX, y: -bounds.minY)
            context.drawPDFPage(original)
            if let image = context.makeImage() {
                let request = VNRecognizeTextRequest()
                request.recognitionLevel = .accurate
                request.usesLanguageCorrection = false
                do {
                    try VNImageRequestHandler(cgImage: image).perform([request])
                    for observation in request.results ?? [] {
                        let b = observation.boundingBox
                        let box = CGRect(x: bounds.minX + b.minX * bounds.width,
                                         y: bounds.minY + b.minY * bounds.height,
                                         width: b.width * bounds.width, height: b.height * bounds.height)
                        guard rows.count < 512 else {
                            gaps.append("OCR exceeds the 512-region page limit at \(describe(box)).")
                            break
                        }
                        guard let candidate = observation.topCandidates(1).first else {
                            gaps.append("Unreadable OCR region at \(describe(box)).")
                            continue
                        }
                        // A broad selectable rectangle can enclose raster words.
                        // Deduplicate only matching text at the same location.
                        let normalized = candidate.string.split(whereSeparator: { $0.isWhitespace }).joined(separator: " ")
                        if rows.contains(where: {
                            $0.channel == "pdf_text"
                                && $0.text.split(whereSeparator: { $0.isWhitespace }).joined(separator: " ") == normalized
                                && $0.box.intersection(box).width * $0.box.intersection(box).height > box.width * box.height * 0.7
                        }) { continue }
                        if candidate.confidence < 0.8 {
                            gaps.append("Uncertain OCR region at \(describe(box)); transcription withheld.")
                            continue
                        }
                        // Mask recognized words, not the whitespace inside a line
                        // box: unrecognized inline raster marks must remain gaps.
                        let words = try! NSRegularExpression(pattern: "\\S+")
                        for match in words.matches(in: candidate.string, range: NSRange(candidate.string.startIndex..<candidate.string.endIndex, in: candidate.string)) {
                            if let range = Range(match.range, in: candidate.string), let word = try? candidate.boundingBox(for: range) {
                                let b = word.boundingBox
                                recoveredInk.append(CGRect(x: bounds.minX + b.minX * bounds.width,
                                    y: bounds.minY + b.minY * bounds.height,
                                    width: b.width * bounds.width, height: b.height * bounds.height))
                            }
                        }
                        rows.append(TextRegion(text: candidate.string, channel: "ocr", box: box,
                                               confidence: candidate.confidence, revision: request.revision))
                    }
                } catch { gaps.append("Local OCR failed: \(error.localizedDescription)") }

                // A successful request does not prove coverage: find visible marks
                // outside recovered regions, including noise and unreadable lines.
                // Work in small tiles so cost is bounded independently of row count.
                if let data = context.data {
                    let masks = recoveredInk.map { $0.insetBy(dx: -1, dy: -1) }
                    let pixels = data.assumingMemoryBound(to: UInt8.self)
                    let tile = 24
                    for top in stride(from: 0, to: height, by: tile) {
                        for left in stride(from: 0, to: width, by: tile) {
                            var missing = CGRect.null
                            let tileBox = CGRect(x: bounds.minX + CGFloat(left) / CGFloat(width) * bounds.width,
                                y: bounds.minY + (1 - CGFloat(min(top + tile, height)) / CGFloat(height)) * bounds.height,
                                width: CGFloat(min(tile, width - left)) / CGFloat(width) * bounds.width,
                                height: CGFloat(min(tile, height - top)) / CGFloat(height) * bounds.height)
                            let nearby = masks.filter { $0.intersects(tileBox) }
                            for y in top..<min(top + tile, height) {
                                for x in left..<min(left + tile, width) {
                                    let offset = y * width * 4 + x * 4
                                    if min(pixels[offset], pixels[offset + 1], pixels[offset + 2]) >= 254 { continue }
                                    let point = CGPoint(x: bounds.minX + (CGFloat(x) + 0.5) / CGFloat(width) * bounds.width,
                                        y: bounds.minY + (1 - (CGFloat(y) + 0.5) / CGFloat(height)) * bounds.height)
                                    if !nearby.contains(where: { $0.contains(point) }) {
                                        missing = missing.union(CGRect(x: point.x, y: point.y, width: bounds.width / CGFloat(width), height: bounds.height / CGFloat(height)))
                                    }
                                }
                            }
                            if !missing.isNull {
                                // Coalesce to a single located unresolved area below.
                                unresolved = unresolved.union(missing)
                            }
                        }
                    }
                }
            } else { gaps.append("Page raster could not be decoded.") }
        } else { gaps.append("Page raster allocation failed.") }
    } else { gaps.append("Invalid page bounds or unsupported page raster.") }
    if !unresolved.isNull {
        gaps.append("Unreadable or non-text visible content remains unresolved at \(describe(unresolved)). No content was inferred.")
    }
    if rows.isEmpty { gaps.append("No text recovered; this page is blank or unreadable.") }
    // Preserve separate physical lines/columns; never join adjacent columns into a fact.
    let middle = bounds.midX
    let hasColumns = rows.contains { $0.box.maxX < middle - 8 }
        && rows.contains { $0.box.minX > middle + 8 }
        && rows.allSatisfy { $0.box.maxX < middle || $0.box.minX > middle }
    func column(_ row: TextRegion) -> Int { hasColumns && row.box.minX > middle ? 2 : 1 }
    rows.sort { a, b in
        if column(a) != column(b) { return column(a) < column(b) }
        let aLine = Int((a.box.midY / 4).rounded())
        let bLine = Int((b.box.midY / 4).rounded())
        if aLine != bLine { return aLine > bLine }
        return a.box.minX < b.box.minX
    }
    let locatedRows = rows.map { row -> [String: Any] in
        var value = row.json
        value["column"] = column(row)
        return value
    }
    return ["number": number, "rotation": page.rotation, "rows": locatedRows, "gaps": gaps]
}

// Read-only local projection. Offsets address extracted text, never PDF bytes.
do {
    let url = URL(fileURLWithPath: CommandLine.arguments[1])
    guard let document = PDFDocument(url: url) else {
        throw NSError(domain: "PDF", code: 1, userInfo: [NSLocalizedDescriptionKey: "PDFKit cannot decode this damaged or unsupported PDF."])
    }
    guard !document.isLocked else {
        throw NSError(domain: "PDF", code: 2, userInfo: [NSLocalizedDescriptionKey: "The PDF is encrypted and locked; no password was supplied and no content was inferred."])
    }
    guard document.pageCount > 0, document.pageCount <= 256 else {
        throw NSError(domain: "PDF", code: 3, userInfo: [NSLocalizedDescriptionKey: "The PDF has no readable pages or exceeds the 256-page extraction bound."])
    }
    var pages: [[String: Any]] = []
    for index in 0..<document.pageCount {
        let result: [String: Any] = autoreleasepool {
            guard let page = document.page(at: index) else {
                return ["number": index + 1, "rows": [], "gaps": ["Page could not be decoded."]]
            }
            return project(page, number: index + 1)
        }
        pages.append(result)
    }
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: ["pages": pages], options: [.sortedKeys]))
} catch {
    FileHandle.standardError.write(Data(error.localizedDescription.utf8))
    exit(1)
}
