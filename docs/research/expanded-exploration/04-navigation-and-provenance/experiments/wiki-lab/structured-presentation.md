# Deterministic structured source presentation

This baseline copies original record IDs, origins, selectors and literal previews. It generates no narrative. Previews are explicitly clipped; full exact text and artifact hashes remain in [portable-fixtures.json](portable-fixtures.json). Generated prose is shown separately in [capsules.md](capsules.md). No readability or task-time study was performed.

## motion

**motion-c1** · constructed conversation · acquisition: captured

Selector: turn c1. Artifact: 01-source-interpretation/experiments/media-lab/controlled-coding-thread.json.

```text
Controlled research demonstration: use the public OpenCV surveillance sample to find frame changes. Mark changed pixels, without interpreting people or their intent.
```

SHA-256: 90fa651ada20eb1ed97afa82e20c7b5f6953e5a87aef5aa8645b55c2e9cd0d3e

**motion-c2** · constructed assistant assertion, no measured before comparison · acquisition: captured

Selector: turn c2. Artifact: 01-source-interpretation/experiments/media-lab/controlled-coding-thread.json.

```text
First attempt compared RGB values directly with threshold 10, producing noise. I propose blur plus grayscale and threshold 35.
```

SHA-256: 90fa651ada20eb1ed97afa82e20c7b5f6953e5a87aef5aa8645b55c2e9cd0d3e

**motion-c3** · constructed user adoption · acquisition: captured

Selector: turn c3. Artifact: 01-source-interpretation/experiments/media-lab/controlled-coding-thread.json.

```text
Use grayscale, a small blur, and threshold 35. Keep frame time anchors. This is an experiment; do not claim the detector identifies suspicious behavior.
```

SHA-256: 90fa651ada20eb1ed97afa82e20c7b5f6953e5a87aef5aa8645b55c2e9cd0d3e

**motion-code** · actually executed constructed code · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 01-source-interpretation/experiments/media-lab/controlled_demo.py.

```text
"""Actual controlled change and runnable public-video difference demonstration.
No object identity or intent inference. Output overlays pixel changes.
"""
from pathlib import Path
import subprocess,json
from PIL import Image,ImageChops,ImageFilter,ImageDraw
ROOT=Path(__file__).parent
CACHE=Path('/tmp/knowledge-garden-media-lab')
THRESHOLD=35
def mask(previous,current):
 a=previous.convert('L').filter(ImageFilter.GaussianBlur(1))
 b=current.conver
```

Preview omits 1310 characters; this is not a complete evidence window. Full record is preserved in portable-fixtures.json.

SHA-256: 0252143111c601230860c15973fbaa1f75aaf07d838ffdb09dcdb24a4c523e14

**motion-t2** · actual measured output · acquisition: captured

Selector: seconds=2.0. Artifact: 01-source-interpretation/experiments/media-lab/controlled-demo-metrics.json.

```text
{"seconds": 2.0, "changed_pixels": 11566, "total_pixels": 442368}
```

SHA-256: f22e4dc18a264c0597cd413add1f5c8d2f22dd65999e1434ab6606fab5bc24fe

**motion-c4** · constructed assistant assertion · acquisition: captured

Selector: turn c4. Artifact: 01-source-interpretation/experiments/media-lab/controlled-coding-thread.json.

```text
Changed controlled_demo.py accordingly and rendered a demonstration. Difference masks are evidence of pixel change, not a reliable object tracker.
```

SHA-256: 90fa651ada20eb1ed97afa82e20c7b5f6953e5a87aef5aa8645b55c2e9cd0d3e

## book

**book-ocr** · derived OCR from real 1860 book scan; not exact scan text · acquisition: captured

Selector: PDF page84,printed page78. Artifact: 01-source-interpretation/experiments/media-lab/book-pdf-page-84-ocr.json.

```text
78
NATURAL SELECTION.
[CHAP. IV
On the other hand, we may feel sure that any
variation
in the lenst degree injurious would be rigidly destroved.
This preservation of favourable variations and the rejec-
tion of injurious
variations, I call Natural Selection.
Variations neither useful nor injurious would not be aftect-
ed by natural selection, and would be left a fluctuating
clement, as perhaps we see in the species called polymor-
phic.
We shall 
```

Preview omits 1726 characters; this is not a complete evidence window. Full record is preserved in portable-fixtures.json.

SHA-256: aa1886cd8584e1bfda580e7bfa1c0e598733d0ec56c93a1d1ba844ad7f22cd79

**deap-article** · captured public DEAP tutorial, not user history · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 01-source-interpretation/experiments/media-lab/article-selection.rst.

```text

Selection
---------

Selection is made among a population by the selection operators that are
available in the :mod:`deap.tools` module. The selection operator usually
takes as first argument an iterable container of individuals and the number of
individuals to select. It returns a list containing the references to the
selected individuals. The selection is made as follow.

.. literalinclude:: /code/tutorials/part_3/3_next_step.py
   :lines: 49-
```

Preview omits 2750 characters; this is not a complete evidence window. Full record is preserved in portable-fixtures.json.

SHA-256: 94e2c8d2df9bf220fdae088d7d9e092e7e19e2877f905b171135a0e2cda1dde7

**deap-code** · captured public implementation, not a recovered user project · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 01-source-interpretation/experiments/media-lab/old-project-selection.py.

```text
import random
import numpy as np

from functools import partial
from operator import attrgetter

######################################
# Selections                         #
######################################


def selRandom(individuals, k):
    """Select *k* individuals at random from the input *individuals* with
    replacement. The list returned contains references to the input
    *individuals*.

    :param individuals: A list of individ
```

Preview omits 6050 characters; this is not a complete evidence window. Full record is preserved in portable-fixtures.json.

SHA-256: 18fbb468f438dfb9c8eb2a161a48a869b80bdde52c173628d3330bc0e11e05a4

**controlled-note** · authored controlled personal note, not recovered user history · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 01-source-interpretation/experiments/media-lab/controlled-personal-note.md.

```text
CONTROLLED PERSONAL NOTE — authored for this experiment, not recovered user history.

I once tried retaining only the fastest benchmark configuration in a toy search project. That lost alternatives useful under changed workloads. I want to revisit whether keeping diverse candidates improves adaptation. No relationship to Darwin or DEAP has been recorded in this note.

```

SHA-256: b4211909cc77f267323616af1fbd3f673f098b5358e494ce24163cc6fe987fcc

## methods

**thread-csv_ids** · constructed_coding_history_with_actual_execution · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 03-connections-and-methods/experiments/method-demo.json.

```text
{"id": "thread-csv_ids", "status": "constructed_coding_history_with_actual_execution", "project": "csv_ids", "messages": [{"id": "csv_ids-m1", "role": "user", "text": "Preserve distinct source identities in the derived lookup key."}, {"id": "csv_ids-m2", "role": "assistant", "text": "A lossy normalization seemed convenient. The reproducer failed, so preserve the scope/identity fields before any optional semantic lookup."}, {"id": "csv_ids-m3", "r
```

Preview omits 1055 characters; this is not a complete evidence window. Full record is preserved in portable-fixtures.json.

SHA-256: ff4edd40c0cc4a66dc9df36975907b10bf9c9c7032fbf8aa2610d5efe836deb1

**thread-url_keys** · constructed_coding_history_with_actual_execution · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 03-connections-and-methods/experiments/method-demo.json.

```text
{"id": "thread-url_keys", "status": "constructed_coding_history_with_actual_execution", "project": "url_keys", "messages": [{"id": "url_keys-m1", "role": "user", "text": "Preserve distinct source identities in the derived lookup key."}, {"id": "url_keys-m2", "role": "assistant", "text": "A lossy normalization seemed convenient. The reproducer failed, so preserve the scope/identity fields before any optional semantic lookup."}, {"id": "url_keys-m3
```

Preview omits 1436 characters; this is not a complete evidence window. Full record is preserved in portable-fixtures.json.

SHA-256: ff4edd40c0cc4a66dc9df36975907b10bf9c9c7032fbf8aa2610d5efe836deb1

**thread-cache_scope** · constructed_coding_history_with_actual_execution · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 03-connections-and-methods/experiments/method-demo.json.

```text
{"id": "thread-cache_scope", "status": "constructed_coding_history_with_actual_execution", "project": "cache_scope", "messages": [{"id": "cache_scope-m1", "role": "user", "text": "Preserve distinct source identities in the derived lookup key."}, {"id": "cache_scope-m2", "role": "assistant", "text": "A lossy normalization seemed convenient. The reproducer failed, so preserve the scope/identity fields before any optional semantic lookup."}, {"id": 
```

Preview omits 1337 characters; this is not a complete evidence window. Full record is preserved in portable-fixtures.json.

SHA-256: ff4edd40c0cc4a66dc9df36975907b10bf9c9c7032fbf8aa2610d5efe836deb1

**thread-unresolved** · constructed_counterexample · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 03-connections-and-methods/experiments/method-demo.json.

```text
{"id": "thread-unresolved", "status": "constructed_counterexample", "project": "signed_urls", "messages": [{"id": "signed-m1", "role": "user", "text": "Can query parameters be sorted for signed URLs? The signature might cover exact bytes. No provider signing specification or verification run is available."}]}
```

SHA-256: ff4edd40c0cc4a66dc9df36975907b10bf9c9c7032fbf8aa2610d5efe836deb1

**method-candidate** · authored method hypothesis; not an installed skill · acquisition: captured

Selector: whole captured source/excerpt. Artifact: 03-connections-and-methods/experiments/method-demo.json.

```text
{"candidate": "Preserve exact identity/scope fields, reproduce collision, make bounded correction, run invariant regression, then use semantic judgments only where compatible.", "exceptions": ["Source fields declared numeric may legitimately normalize padding.", "Signed URL byte equivalence remains unresolved: preserve original bytes until provider semantics verified.", "Tenant identity not applicable to genuinely public immutable resources."]}
```

SHA-256: ff4edd40c0cc4a66dc9df36975907b10bf9c9c7032fbf8aa2610d5efe836deb1

