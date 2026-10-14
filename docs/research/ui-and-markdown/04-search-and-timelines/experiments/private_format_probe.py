"""Read-only, bounded personal-file structural probe. Outputs only aggregates to repo.
Exact mappings, hashes and retained originals go to a mode-0700 directory OUTSIDE repo.
No source content, filenames, URLs or model requests enter repository output.
Selection is deterministic by hashed pathname, NOT representative random sampling.
"""
from pathlib import Path
import os,json,hashlib,shutil,time,collections,zipfile,xml.etree.ElementTree as ET,datetime,csv,io
BASE=Path.home()/'Documents'
OUT=Path.home()/'Library/Application Support/KnowledgeGardenResearch'/('ui-markdown-'+datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ'))
R=Path(__file__).resolve().parent
EXT={'.pdf','.pptx','.docx','.xlsx','.jpg','.jpeg','.png','.heic','.csv','.mp4','.mov','.wav','.mp3','.txt'}
SKIP={'github','.git','node_modules','.venv','venv','Library','.cache','target','build','dist','.obsidian'}
def main():
    start=time.perf_counter();pool=collections.defaultdict(list);scanned=0
    for root,dirs,files in os.walk(BASE):
        relative=Path(root).relative_to(BASE);dirs[:]=sorted(d for d in dirs if d not in SKIP and not d.startswith('.') and len(relative.parts)<5)
        for name in sorted(files):
            p=Path(root)/name;scanned+=1
            if p.suffix.lower() not in EXT or p.is_symlink():continue
            try:
                if 0<p.stat().st_size<=8_000_000:pool[p.suffix.lower()].append(p)
            except OSError:pass
        if scanned>=10000:break
    OUT.mkdir(parents=True,mode=0o700);os.chmod(OUT,0o700)
    selected=[]
    for ext,paths in sorted(pool.items()):
        selected.extend(sorted(paths,key=lambda p:hashlib.sha256(str(p).encode()).hexdigest())[:1])
    private=[];aggregates=[]
    for i,p in enumerate(selected[:14]):
        sid=f'private-format-{i+1:02d}';dest=OUT/(sid+p.suffix.lower());data=p.read_bytes();before=hashlib.sha256(data).hexdigest();dest.write_bytes(data);os.chmod(dest,0o600)
        result={'format':p.suffix.lower(),'selected':1,'bytesBucket':'<1MB' if len(data)<1_000_000 else '1–8MB','copyHashVerified':hashlib.sha256(dest.read_bytes()).hexdigest()==before,'status':'retained-only'}
        t=time.perf_counter()
        try:
            if p.suffix.lower()=='.pdf':
                from pypdf import PdfReader
                reader=PdfReader(io.BytesIO(data));length=sum(len(pg.extract_text() or '') for pg in reader.pages);result.update(status='text-layer-checked',units=len(reader.pages),hasExtractableText=length>0)
            elif p.suffix.lower() in {'.pptx','.docx','.xlsx'}:
                with zipfile.ZipFile(io.BytesIO(data)) as z:
                    prefix={'.pptx':'ppt/slides/slide','.docx':'word/document.xml','.xlsx':'xl/worksheets/sheet'}[p.suffix.lower()]
                    entries=[n for n in z.namelist() if n.startswith(prefix) and n.endswith('.xml') and '/_rels/' not in n]
                    count=sum(sum(1 for e in ET.fromstring(z.read(n)).iter() if e.text and e.text.strip()) for n in entries)
                result.update(status='xml-structure-checked',units=len(entries),hasTextNodes=count>0)
            elif p.suffix.lower() in {'.jpg','.jpeg','.png','.heic'}:
                from PIL import Image
                with Image.open(io.BytesIO(data)) as im:im.verify()
                result.update(status='image-container-decoded')
            elif p.suffix.lower()=='.csv':
                rows=list(csv.reader(io.StringIO(data.decode('utf-8-sig'))));result.update(status='csv-decoded',units=len(rows))
            elif p.suffix.lower()=='.txt':result.update(status='utf8-decoded',hasText=bool(data.decode('utf8').strip()))
        except Exception as e:result.update(status='probe-failed',errorType=type(e).__name__)
        result.update(seconds=time.perf_counter()-t,originalUnchanged=hashlib.sha256(p.read_bytes()).hexdigest()==before)
        private.append(dict(id=sid,original=str(p),copy=str(dest),sha256=before,result=result));aggregates.append(result)
    (OUT/'manifest.json').write_text(json.dumps(private,indent=2)+'\n');os.chmod(OUT/'manifest.json',0o600)
    report=dict(researchDate='2026-09-29',inventoryFilesConsidered=scanned,eligibleByFormat={k:len(v) for k,v in sorted(pool.items())},selectedCount=len(aggregates),results=aggregates,seconds=time.perf_counter()-start,boundary='Documents only; github and hidden/cache/build folders excluded; max depth 5, 10000 files, 8 MB/file, 1/hash-selected format, 14 total. Local copies and exact mappings restricted outside repository. No text exposed or sent to API. Structural extraction is NOT semantic fidelity or general support.',conversationURLDestinationsFetched=0)
    (R/'private-format-aggregates.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
if __name__=='__main__':main()
