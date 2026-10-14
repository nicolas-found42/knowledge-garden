"""Separate bounded repairs; never replace initial truncation/attribution failures."""
from run_perception import TASKS,call
from pathlib import Path
import concurrent.futures
selected=['video-contact-sheet-observation','controlled-demo-screenshot-observation','book-project-connection','book-pdf-page-92-interpretation']
if __name__=='__main__':
 tasks=[]
 for t in TASKS:
  if t['id'] in selected:
   r={**t,'id':t['id']+'-bounded-repair'}
   r['extra']=t.get('extra','')+'\nREPAIR OUTPUT CONTRACT: at most 6 observations and 3 candidates; output under 700 tokens; omit full transcript/exact_visible_text. Use explicit source_id from [book_scan_pdf_page84, book_scan_pdf_page92, public_deap_article, public_deap_code, controlled_personal_note, source_video_contact_sheet, controlled_demo_screenshot] for every claim. Book text must never be attributed to DEAP. Do not describe notes as real recovered personal history. All anchors are proposals; keep source identity correct. Include central qualification: favorable/injurious/neutral variation or wolves retain strength, if visible.'
   tasks.append(r)
 with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
  for out in pool.map(call,tasks):print(out,flush=True)
