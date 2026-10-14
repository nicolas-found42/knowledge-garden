"""Diagnostic alternative supporting model plus explicitly timed native video repair."""
import run_perception as runner
from concurrent.futures import ThreadPoolExecutor
if __name__=='__main__':
 runner.MODEL='openai/gpt-4o-mini'
 selected=['phototest-observation','book-pdf-page-92-observation','video-contact-sheet-observation','controlled-demo-screenshot-observation']
 tasks=[{**t,'id':t['id']+'-gpt4omini','extra':'Return at most 5 factual observations under 600 tokens. Preserve visible labels. For contact sheet distinguish panel timestamp from bounding box; for screenshot distinguish changed pixel overlays from detected human objects. For book preserve provided-always strength qualification.'} for t in runner.TASKS if t['id'] in selected]
 with ThreadPoolExecutor(max_workers=2) as pool:
  for out in pool.map(runner.call,tasks):print(out,flush=True)
 runner.MODEL='google/gemini-2.5-flash-lite'
 t=next(t for t in runner.TASKS if t['id']=='native-video-observation')
 print(runner.call({**t,'id':t['id']+'-timed-repair','extra':'DIAGNOSTIC: output exactly 3 event observations and no repeated objects. JSON objects MUST contain time_seconds (a number 0 to10), claim, uncertainty. Do not use image bounding boxes as time anchors. Limit output to400 tokens; no full exact text or repeated person lists.'}),flush=True)
