import Foundation
import PDFKit

// Read-only local projection. Page locators describe the retained PDF; text
// offsets belong to the projection and are never original PDF byte offsets.
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
        guard let page = document.page(at: index) else {
            pages.append(["number": index + 1, "rows": [], "gaps": ["Page could not be decoded."]])
            continue
        }
        let bounds = page.bounds(for: .mediaBox)
        let text = page.string ?? ""
        let selection = page.selection(for: NSRange(location: 0, length: (text as NSString).length))
        let selections = selection?.selectionsByLine() ?? []
        var rows: [[String: Any]] = []
        var gaps: [String] = []
        if selections.count > 512 { gaps.append("Selectable text exceeds the 512-region page limit.") }
        for line in selections.prefix(512) {
            guard let value = line.string?.trimmingCharacters(in: .whitespacesAndNewlines), !value.isEmpty else { continue }
            let region = line.bounds(for: page)
            rows.append(["text": value, "channel": "pdf_text", "box": [region.minX, region.minY, region.width, region.height]])
        }
        if rows.isEmpty { gaps.append("No selectable text was recovered; scanned or unreadable page content remains missing.") }
        pages.append(["number": index + 1, "rotation": page.rotation,
                      "bounds": [bounds.minX, bounds.minY, bounds.width, bounds.height],
                      "rows": rows, "gaps": gaps])
    }
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: ["pages": pages], options: [.sortedKeys]))
} catch {
    FileHandle.standardError.write(Data(error.localizedDescription.utf8))
    exit(1)
}
