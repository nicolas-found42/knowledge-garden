#!/usr/bin/env python3
"""Capture public-source status/hash and temporary text for source inspection."""
import concurrent.futures, datetime, hashlib, html, json, pathlib, re, urllib.request
ROOT=pathlib.Path(__file__).resolve().parent
SOURCES={
'obsidian-graph':'https://help.obsidian.md/plugins/graph',
'obsidian-canvas':'https://help.obsidian.md/plugins/canvas',
'obsidian-search':'https://help.obsidian.md/plugins/search',
'devonthink':'https://www.devontechnologies.com/apps/devonthink/ai',
'heptabase':'https://support.heptabase.com/en/articles/11176386-use-deep-links',
'tinderbox-sorting':'https://www.eastgate.com/Tinderbox/sorting.html',
'devonthink-ai-blog':'https://shop.devontechnologies.com/blog/20250717-devonthink-ai-app',
'tinderbox':'https://www.eastgate.com/Tinderbox/attributes.html',
'zotero-related':'https://www.zotero.org/support/related',
'zotero-reader':'https://www.zotero.org/support/pdf_reader',
'liquidtext':'https://www.liquidtext.net/',
'scrintal':'https://scrintal.com/',
'curio':'https://www.zengobi.com/curio/',
'eagle':'https://en.eagle.cool/',
'kagi':'https://help.kagi.com/kagi/search/quick-answer.html',
'kagi-current':'https://help.kagi.com/kagi/ai/quick-answer.html',
'obsidian-graph-source':'https://raw.githubusercontent.com/obsidianmd/obsidian-help/master/en/Plugins/Graph%20view.md',
'obsidian-canvas-source':'https://raw.githubusercontent.com/obsidianmd/obsidian-help/master/en/Plugins/Canvas.md',
'obsidian-search-source':'https://raw.githubusercontent.com/obsidianmd/obsidian-help/master/en/Plugins/Search.md',
'reader':'https://readwise.io/reader',
'brain':'https://www.thebrain.com/products/thebrain',
'apple-photos':'https://support.apple.com/guide/photos/find-photos-by-text-content-or-date-pht5c9e7c0a0/mac',
'apple-sandbox-md':'https://developer.apple.com/tutorials/data/documentation/security/app-sandbox.md',
'apple-file-access':'https://developer.apple.com/tutorials/data/documentation/security/accessing-files-from-the-macos-app-sandbox.md',
'apple-notary-md':'https://developer.apple.com/tutorials/data/documentation/security/notarizing-macos-software-before-distribution.md',
'fsevents-using':'https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/UsingtheFSEventsFramework/UsingtheFSEventsFramework.html',
'archive-fsevents':'https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/Introduction/Introduction.html',
'apple-webkit':'https://developer.apple.com/tutorials/data/documentation/webkit/wkwebview.json',
'apple-workspace':'https://developer.apple.com/tutorials/data/documentation/appkit/nsworkspace.json',
'apple-bookmarks':'https://developer.apple.com/library/archive/documentation/Security/Conceptual/AppSandboxDesignGuide/AppSandboxInDepth/AppSandboxInDepth.html',
'apple-distribute':'https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution',
'tauri-architecture':'https://v2.tauri.app/concept/architecture/',
'tauri-fs':'https://v2.tauri.app/plugin/file-system/',
'tauri-opener':'https://v2.tauri.app/plugin/opener/',
'tauri-sign':'https://v2.tauri.app/distribute/sign/macos/',
'electron-process':'https://www.electronjs.org/docs/latest/tutorial/process-model',
'electron-security':'https://www.electronjs.org/docs/latest/tutorial/security',
'electron-shell':'https://www.electronjs.org/docs/latest/api/shell',
'electron-sign':'https://www.electronjs.org/docs/latest/tutorial/code-signing',
'node-fs':'https://nodejs.org/api/fs.html',
'sigma':'https://www.sigmajs.org/docs/',
'cytoscape':'https://js.cytoscape.org/',
'reactflow':'https://reactflow.dev/learn',
'fts5':'https://www.sqlite.org/fts5.html',
'vis-timeline':'https://visjs.github.io/vis-timeline/docs/timeline/',
'pdfjs':'https://mozilla.github.io/pdf.js/',
}
def get(pair):
 id,url=pair
 row={'id':id,'url':url,'inspected_at_utc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'selection':'independently selected public primary source','application_tested':False}
 try:
  with urllib.request.urlopen(urllib.request.Request(url,headers={'User-Agent':'Mozilla/5.0 research'}),timeout=25) as r:
   data=r.read(); row.update(status=r.status,resolved_url=r.url,bytes=len(data),sha256=hashlib.sha256(data).hexdigest())
  s=data.decode('utf-8','replace')
  s=re.sub(r'<(script|style)\b[^>]*>.*?</\1>',' ',s,flags=re.S|re.I)
  s=html.unescape(re.sub('<[^>]+>',' ',s)); s=re.sub(r'\s+',' ',s)
  pathlib.Path('/tmp/kg-precedents').mkdir(exist_ok=True)
  pathlib.Path('/tmp/kg-precedents',id+'.txt').write_text(s)
  row['status_label']='captured'; row['text_characters']=len(s)
 except Exception as e: row.update(status_label='could_not_retrieve',error=str(e))
 return row
if __name__=='__main__':
 with concurrent.futures.ThreadPoolExecutor(max_workers=8) as ex: rows=list(ex.map(get,SOURCES.items()))
 (ROOT/'source-manifest.json').write_text(json.dumps({'purpose':'public documentation inspection, not app execution or full-source audit','sources':rows},indent=2)+'\n')
 for r in rows: print(r['id'],r['status_label'],r.get('bytes',r.get('error')))
