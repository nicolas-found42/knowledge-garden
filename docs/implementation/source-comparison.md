# Current source comparison (#23)

The reader can retain two temporary source selections and show two independently
scrollable, bounded passages. Select a source from its reading page or the
supporting source from a knowledge page. A third selection explicitly replaces a
side. The first-line controls select surrounding passages; the original and
source-context actions preserve access to the material's qualifications. Back
restores comparison position/focus; Close restores the prior reader context.

`comparison_side` reads current retained text or a labeled extracted projection,
never generates missing passages, and reports missing originals/extraction and
coverage separately. Excerpts are at most 40 lines / 8 KiB, preserving UTF-8
boundaries. Projection offsets are explicitly distinguished from original bytes.

A signature includes current source version, source text/title/coverage, and
current linked knowledge bytes. Changed material clears the old passage until
explicit refresh; missing material does not leave cached text visible. Original
handoff rechecks the same selection under the desktop engine lock. Navigation
restoration revalidates before rendering passages. This view writes no durable
comparison document, history snapshot, or alternate current knowledge.

Verification covers qualified differing counts, replacement and owner-edit
invalidation, missing/unsupported originals, bounded UTF-8 passages, independent
side scrolling, out-of-order responses, and reader/context/Back/Close focus.
A Vite-hosted real reader connected to the Rust application driver exercises the
two-source flow with actual temporary retained originals. Packaging and broad
assistive-access qualification remain the dedicated downstream release gates.
