"""Audit actual generated candidates against independently retained exact source records."""
import json,re,concurrent.futures
from pathlib import Path
from run_judgments import call,extract,ocr
ROOT=Path(__file__).parent
sources={'book_scan_pdf_page84':extract(84),'book_scan_pdf_page92':extract(92),'public_deap_article':(ROOT/'article-selection.rst').read_text()[4500:6900],'public_deap_code':(ROOT/'old-project-selection.py').read_text()[:6500],'controlled_personal_note':(ROOT/'controlled-personal-note.md').read_text()}
r=json.loads((ROOT/'perception-records/book-project-connection-bounded-repair.json').read_text());s=r['response']['choices'][0]['message']['content'];s=re.sub(r'^```(?:json)?\s*|\s*```$','',s.strip());data=json.loads(s);cases=[]
for i,candidate in enumerate(data['observations']):
 # Known initial/manual audit expectations, frozen before these calls.
 expected='contradicted' if i==5 else ('unknown' if i==2 else 'supported')
 c=dict(id=f'generated-source-audit-{i}',family='generated-source-audit',source_truth=expected,state=dict(candidate=candidate,claimed_source=candidate['anchor'],exact_sources=sources,source_warning='These sources are independent retained extracts, not the same supporting-model observation repeated. Test claim vs named source; preserve negation and attribution.'),questions={'support':dict(type='choice',instructions='Does the candidate CLAIM, not merely its why quotation, accurately represent the claimed_source from exact_sources? Preserve negations and source attribution. Related material in another source does not prove the named source supports the claim.',criteria={'supported':'Full claim matches named source.','contradicted':'Named source directly contradicts claim.','unknown':'Source missing or factual attribution not established.'})});cases.append(c)
# Specific initial attribution failure: same correct Darwin claim assigned to DEAP article.
c=dict(id='generated-initial-wrong-source',family='generated-source-audit',source_truth='unknown',state=dict(candidate={'claim':'Natural selection preserves favourable variations and rejects injurious variations.','anchor':'public_deap_article'},claimed_source='public_deap_article',exact_sources=sources),questions={'support':dict(type='choice',instructions='Does public_deap_article itself establish the complete candidate claim? A correct quote in the book cannot repair attribution to the article.',criteria={'supported':'Named article supports.','contradicted':'Named article contradicts.','unknown':'Claim absent from named article.'})});cases.append(c)
(ROOT/'generated-audit-labels.json').write_text(json.dumps(cases,indent=2))
if __name__=='__main__':
 with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
  for out in pool.map(call,cases):print(out,flush=True)
