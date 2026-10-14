"""Local-only research preview and bounded measurement capture. No model endpoint.
python3 docs/research/ui-and-markdown/serve.py [--port 8768]
"""
from pathlib import Path
from http.server import ThreadingHTTPServer,SimpleHTTPRequestHandler
import argparse,json,datetime,hashlib
HERE=Path(__file__).resolve().parent
ROOT=next(p for p in HERE.parents if (p/'AGENTS.md').exists())
class Handler(SimpleHTTPRequestHandler):
    def __init__(self,*a,**kw):super().__init__(*a,directory=str(ROOT),**kw)
    def do_POST(self):
        if self.path!='/_research/measurements':self.send_error(404);return
        length=int(self.headers.get('Content-Length',0))
        if not 0<length<1_000_000:self.send_error(413);return
        try:
            data=json.loads(self.rfile.read(length));assert isinstance(data,dict) and isinstance(data['results'],list) and 'runtime' in data
        except Exception:self.send_error(400);return
        content=json.dumps(data,indent=2)+'\n';digest=hashlib.sha256(content.encode()).hexdigest()[:10]
        out=HERE/'03-interface-concepts/browser-records';out.mkdir(exist_ok=True)
        name='measurements-'+datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')+'-'+digest+'.json'
        (out/name).write_text(content)
        body=json.dumps({'saved':name}).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(body)));self.end_headers();self.wfile.write(body)
if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--port',type=int,default=8768);args=p.parse_args()
    print(f'Preview: http://127.0.0.1:{args.port}/docs/research/ui-and-markdown/03-interface-concepts/index.html',flush=True)
    ThreadingHTTPServer(('127.0.0.1',args.port),Handler).serve_forever()
