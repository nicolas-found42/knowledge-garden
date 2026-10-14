"""Disposable controlled vault. Rebuild only a new destination, never overwrite a vault."""
from pathlib import Path
import hashlib,json,shutil,sys
ROOT=Path(__file__).resolve().parent
V=Path(sys.argv[1]) if len(sys.argv)>1 else ROOT/'disposable-vault'
if V.exists(): raise SystemExit('Refusing to overwrite existing vault: '+str(V))
def write(path,text):
 p=V/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
def note(path,props,body):
 write(path,'---\n'+''.join(f'{k}: {json.dumps(v)}\n' for k,v in props.items())+'---\n\n'+body+'\n')
write('Assets/field-notes.txt','Controlled field note. On 2026-09-01, Mira observed three red flowers at Riverside.\nNo botanical identification was established.\n')
raw=(V/'Assets/field-notes.txt').read_bytes();h=hashlib.sha256(raw).hexdigest()
write('Assets/field-notes-copy.txt',raw.decode())
write('Assets/field-notes-revision.txt',raw.decode().replace('three red','four red'))
# PNG controlled single grayscale/alpha pixel; public-domain authored fixture.
import base64
(V/'Assets/flower.png').write_bytes(base64.b64decode('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+jS2sAAAAASUVORK5CYII='))
note('Sources/Field observation.md',dict(id='source-field',kind='source',revision='r1',asset='Assets/field-notes.txt',sha256=h,origin='controlled fixture',acquisition='captured'),'''# Field observation

Original: [[Assets/field-notes.txt]]. Image: ![[Assets/flower.png]]

## Evidence

Mira observed three red flowers at Riverside on 2026-09-01. No botanical identification was established. ^observation

The image is a one-pixel controlled fixture; it is not photographic evidence of these flowers.
''')
note('Sources/Second acquisition.md',dict(id='source-second',kind='source',revision='r1',asset='Assets/field-notes-copy.txt',sha256=h,origin='separate controlled acquisition'), '# Second acquisition\n\nSame bytes, separate source identity: [[Field observation]].')
note('Pages/Mira.md',dict(id='person-mira',kind='person',aliases=['Observer Mira']), '# Mira\n\nAn observer named in [[Field observation#^observation]]. See [[Mira observed flowers]].')
note('Pages/Riverside.md',dict(id='place-riverside',kind='place'), '# Riverside\n\nA controlled place label, not a resolved real-world location. [[Mira]] recorded an observation here.')
note('Pages/Flower study.md',dict(id='page-study',kind='page',version=1), '# Flower study\n\nThree red flowers were reported, with no botanical identification. [[Field observation#^observation]]\n\n## User notes\n\nI want to compare a later observation before inferring change.\n\n## Links\n\n[[Mira]] · [[Riverside]] · [[Mira observed flowers]] · [[Study trail]] · [[Red-flower search]]\n')
note('Relationships/Mira observed flowers.md',dict(id='rel-observation',kind='relationship',relation='observed-at',subject='person-mira',object='place-riverside',status='recorded',evidence_source='source-field',evidence_revision='r1',evidence_anchor='observation'), '# Mira observed flowers\n\n[[Mira]] **observed-at** [[Riverside]].\n\nEvidence: [[Field observation#^observation]]. Recorded from a controlled authored note; no identification claim.\n\nThe source ID, revision and block anchor are durable properties. Ordinary links expose the same references in Obsidian.\n')
note('Annotations/Identification gap.md',dict(id='annotation-gap',kind='annotation',target='source-field',selector='block:observation',status='question'), '# Identification gap\n\n[[Field observation#^observation]] does not identify the species. Do not promote red color into an identification.')
note('Views/Study trail.md',dict(id='trail-study',kind='trail'), '# Study trail\n\n1. Read [[Field observation#^observation]].\n2. Inspect [[Mira observed flowers]].\n3. Revisit [[Identification gap]].\n4. Compare [[Flower study]].\n')
note('Views/Red-flower search.md',dict(id='search-red',kind='saved-search',query='red flowers',filters=['kind:source','acquisition:captured']), '# Red-flower search\n\nDurable query: `red flowers`; source records with captured originals.\n\n```query\n"red flowers"\n```\n\nThe query block is an ordinary Obsidian Search embed. App filters above require app interpretation; all meaning remains in this Markdown record.\n')
note('Views/Research workspace.md',dict(id='view-workspace',kind='saved-view',selection=['page-study','source-field','rel-observation'],layout='reading-with-evidence'), '# Research workspace\n\nA saved selection and layout, not a screenshot of temporary UI state.\n\n[[Flower study]] beside [[Field observation]] and [[Mira observed flowers]].\n')
note('START HERE.md',dict(id='start',kind='guide'), '# Disposable Markdown interoperability vault\n\nControlled sources only. No plugin or app configuration is required. This is a research fixture, not a production format.\n\nStart at [[Flower study]]. Open [[Mira observed flowers]] to inspect relationship properties and evidence. Open [[Study trail]], [[Red-flower search]] and [[Research workspace]].\n\n## Standard Markdown links\n\n[Flower study](Pages/Flower%20study.md), [original text](Assets/field-notes.txt), ![controlled image](Assets/flower.png).\n\n## GUI checks for researcher\n\nRead the page; follow links and block anchor; inspect backlinks; inspect flat properties; show image embed; externally append a sentence; rename Mira inside Obsidian and record links; move Riverside externally and record broken links; confirm IDs persist. Do these only in a temporary copy.\n')

note('Sources/Field observation revision 2.md',dict(id='source-field-r2',kind='source-revision',source_identity='source-field',revision='r2',supersedes='r1',asset='Assets/field-notes-revision.txt',sha256=hashlib.sha256((V/'Assets/field-notes-revision.txt').read_bytes()).hexdigest(),origin='controlled revised acquisition'), '# Field observation revision 2\n\nThis separate controlled revision reports four red flowers. It does not replace the historical evidence used by [[Mira observed flowers]].\n\nOriginal revision: [[Field observation]]. New bytes: [[Assets/field-notes-revision.txt]].\n')
print(V)
