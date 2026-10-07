# Photo evidence on the target Mac

The macOS build embeds a Swift helper linked to ImageIO and Vision. Import retains the exact original and writes a readable Markdown projection. No runtime Swift compiler or general-purpose cloud image model is required. The source reader generates a JPEG preview only when requested; the exact original remains available through its normal link.

PNG, JPEG and HEIC are independently labeled synthetic fixture formats actually decoded on macOS 26.4.1, Apple Silicon with 16 GiB RAM. ImageIO also supports the application's TIFF, HEIF and WebP dispatch, but those additional formats are not yet qualified by this fixture set. Do not present the dispatch list as a tested corpus.

The decoder limit is 50 megapixels. Vision receives an upright preview with a maximum edge of 2048 pixels. OCR is bounded to 256 regions, file metadata to 64 fields and 4096 bytes per value, classifier outputs to ten predictions, the child operation to 60 seconds and its result to 8 MiB. Limits or failed native requests leave explicit extraction gaps, retained originals and qualified evidence. Semantic candidate coverage has its existing independent 64-span budget and cannot silently truncate an oversized photo projection.

Evidence channels remain separate in durable fact records:

- `observed_pixels`: successfully decoded pixel dimensions, without named identities or capture authentication.
- `file_metadata`: stored orientation and bounded EXIF, TIFF, IPTC and GPS fields; values are unauthenticated file assertions.
- `supplied_caption`: textual IPTC caption content, even though its storage envelope is metadata.
- `ocr`: recognized text, confidence and a normalized region locator. Printed “12 visits” does not independently establish that an observation counted twelve visits.
- `generated_interpretation`: uncertain native classifier predictions, confidence and request revision; a class label does not establish a person or authenticated scene object.

Metadata or caption dates do not establish an authenticated capture date or reliable source-version ordering. Named photo identities remain unknown. The application validates channel origin, locator and offset basis independently of provider acceptance, and rejects promotion to date/count/identity facts beyond the qualified channel statement. General-purpose selected-provider image analysis has not been validated and remains a disclosed capability gap; no unselected generative service is substituted.

Photo evidence uses `extracted_image_projection` offsets, never original JPEG/HEIC byte offsets. OCR boxes use upright-image normalized coordinates with a bottom-left origin. The readable locator accompanies search results and the image preview; evidence without a region uses a disclosed whole-image fallback.

The fixed-label corpus is one generated photo encoded in three formats. It has conflicting EXIF/caption dates, stripped date metadata and unknown identities. A separate injected false-OCR fixture tests qualification preservation and rejection of count promotion; it is not a measured native OCR error rate. Real native requests and a separate live Jev semantic evaluation must be reported with their frozen build, inputs and limits. Full photo quality and scale qualification belong to the later corpus/release gates.
