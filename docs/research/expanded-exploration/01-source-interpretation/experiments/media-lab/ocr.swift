import Foundation
import Vision
import AppKit
let url = URL(fileURLWithPath: CommandLine.arguments[1])
let request = VNRecognizeTextRequest()
request.recognitionLevel = .accurate
request.usesLanguageCorrection = false
let handler = VNImageRequestHandler(url: url)
do {
 try handler.perform([request])
 let rows = (request.results ?? []).map { observation -> [String: Any] in
  let r = observation.boundingBox
  return ["text": observation.topCandidates(1).first?.string ?? "", "confidence": observation.topCandidates(1).first?.confidence ?? 0, "rect_bottom_left_normalized": [r.origin.x,r.origin.y,r.size.width,r.size.height]]
 }
 let data = try JSONSerialization.data(withJSONObject: rows, options: [.prettyPrinted,.sortedKeys])
 print(String(data:data,encoding:.utf8)!)
} catch { fputs("OCR failed: \(error)\n", stderr); exit(1) }
