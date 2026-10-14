"""Replay pinned acquisition; checks stored hash, retains failures, never follows history URLs."""
import urllib.request,hashlib,json
from pathlib import Path
ROOT=Path(__file__).parent;CACHE=Path('/tmp/knowledge-garden-media-lab');CACHE.mkdir(exist_ok=True)
def main():
 manifest=json.loads((ROOT/'sources.json').read_text());attempts=[]
 for r in manifest:
  if r['status']!='captured':continue
  dest=CACHE/r['id'] if r['id'] in ['book.pdf','vtest.avi'] else ROOT/r['id']
  try:
   if dest.exists():b=dest.read_bytes()
   else:b=urllib.request.urlopen(r['url'],timeout=60).read()
   sha=hashlib.sha256(b).hexdigest()
   if sha!=r['sha256']:raise ValueError('Pinned hash changed: do not silently replace source')
   if not dest.exists():dest.write_bytes(b)
   attempts.append(dict(id=r['id'],status='captured_verified',sha256=sha))
  except Exception as e:attempts.append(dict(id=r['id'],status='could_not_retrieve',error=str(e)))
 (CACHE/'replay-acquisition.json').write_text(json.dumps(attempts,indent=2));print(json.dumps(attempts,indent=2))
if __name__=='__main__':main()
