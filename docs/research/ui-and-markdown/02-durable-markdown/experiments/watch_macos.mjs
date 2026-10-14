// Disposable macOS watcher experiment. Event notifications trigger reconciliation;
// the content scan, not an inferred event log, determines the rebuilt state.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import crypto from 'node:crypto';
import {fileURLToPath} from 'node:url';
const root=path.dirname(fileURLToPath(import.meta.url));
const expected=JSON.parse(fs.readFileSync(path.join(root,'watcher-expected.json')));
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const tmp=fs.mkdtempSync(path.join(os.tmpdir(),'kg-watch-'));
const v=path.join(tmp,'vault');fs.cpSync(path.join(root,'disposable-vault'),v,{recursive:true});
const start=performance.now();let phase='initial',events=[],errors=[],operations=[],rescans=[],timer=null;
function scan(dir=v,prefix='') {let out={};for(const entry of fs.readdirSync(dir,{withFileTypes:true}).sort((a,b)=>a.name.localeCompare(b.name))){const rel=prefix+entry.name,p=path.join(dir,entry.name);if(entry.isDirectory())Object.assign(out,scan(p,rel+'/'));else out[rel]=sha(fs.readFileSync(p));}return out;}
const normalize=o=>JSON.stringify(Object.entries(o).sort(([a],[b])=>a.localeCompare(b)));
let intended=scan();
function event(kind,type,name){events.push({kind,phase,eventType:type,filename:name?.toString()??null,elapsed_ms:performance.now()-start,timestamp_utc:new Date().toISOString()});if(kind==='directory'){clearTimeout(timer);timer=setTimeout(()=>{try{rescans.push({reason:'debounced-directory-event',elapsed_ms:performance.now()-start,hashes:scan()});}catch(e){errors.push({phase,error:String(e)});}},80);}}
let directory,file;
try {
 directory=fs.watch(v,{recursive:true},(t,n)=>event('directory',t,n));directory.on('error',e=>errors.push({phase,error:String(e)}));
 const page='Pages/Flower study.md';const pp=path.join(v,page);
 file=fs.watch(pp,(t,n)=>event('single-file-inode',t,n));file.on('error',e=>errors.push({phase,error:String(e)}));
 await wait(300);
 async function op(name,fn){phase=name;const t=performance.now();fn();operations.push({operation:name,elapsed_ms:performance.now()-start,mutation_ms:performance.now()-t});await wait(600);}
 const appended=fs.readFileSync(pp,'utf8')+'\nWatcher appended user note.\n';
 await op('append',()=>{fs.appendFileSync(pp,'\nWatcher appended user note.\n');intended[page]=sha(Buffer.from(appended));});
 const replaced=appended+'\nAtomic editor save.\n';
 await op('atomic-save rename-over',()=>{const pending=pp+'.save';fs.writeFileSync(pending,replaced);fs.renameSync(pending,pp);intended[page]=sha(Buffer.from(replaced));});
 await op('post-save append',()=>{fs.appendFileSync(pp,'\nPost-replacement edit.\n');intended[page]=sha(Buffer.from(replaced+'\nPost-replacement edit.\n'));});
 await op('external rename',()=>{fs.renameSync(path.join(v,'Pages/Mira.md'),path.join(v,'Pages/Observer.md'));intended['Pages/Observer.md']=intended['Pages/Mira.md'];delete intended['Pages/Mira.md'];});
 await op('burst create 40 notes',()=>{for(let i=0;i<40;i++){let rel=`Pages/burst-${i}.md`,text=`---\nid: "burst-${i}"\nkind: "page"\n---\n\n# Burst ${i}\n`;fs.writeFileSync(path.join(v,rel),text);intended[rel]=sha(Buffer.from(text));}});
 await op('remove',()=>{fs.unlinkSync(path.join(v,'Annotations/Identification gap.md'));delete intended['Annotations/Identification gap.md'];});
 phase='rebuild scan';const t=performance.now();const final=scan();operations.push({operation:phase,elapsed_ms:performance.now()-start,scan_ms:performance.now()-t});
 // Controlled adapter-level thinning: not presented as actual OS event loss.
 const retained=events.filter((_,i)=>i%2===0);const thinnedRebuild=scan();
 const observer=fs.readFileSync(path.join(v,'Pages/Observer.md'),'utf8');
 const checks={end_state_hashes_match_intended:normalize(final)===normalize(intended),observed_event_count_positive:events.length>0,removed_file_absent_after_rescan:!('Annotations/Identification gap.md' in final),rename_id_preserved_after_rescan:observer.includes('id: "person-mira"'),atomic_save_content_authoritative:fs.readFileSync(pp,'utf8').includes('Atomic editor save.')&&fs.readFileSync(pp,'utf8').includes('Post-replacement edit.'),burst_files_all_recovered:Array.from({length:40},(_,i)=>`Pages/burst-${i}.md`).every(k=>k in final),rescan_matches_after_event_thinning:normalize(thinnedRebuild)===normalize(intended)};
 clearTimeout(timer);file.close();directory.close();
 const counts={};for(const e of events){let key=e.kind+'/'+e.phase;counts[key]=(counts[key]??0)+1;}
 const report={timestamp_utc:new Date().toISOString(),environment:{node:process.version,platform:process.platform,release:os.release(),arch:process.arch},expected_sha256:sha(fs.readFileSync(path.join(root,'watcher-expected.json'))),operations,events,counts,errors,debounced_rescans:rescans,intended_hashes:intended,final_hashes:final,checks,summary:{passed:Object.values(checks).filter(Boolean).length,total:Object.keys(checks).length,events:events.length,final_files:Object.keys(final).length,thinned_events:retained.length},scope:'Actual local macOS fs.watch event stream; one bounded run; controlled adapter-level event thinning; final reconciliation performed by independent full scan. Not Obsidian or production watcher reliability.'};
 const out=path.join(root,'runs','watch-'+new Date().toISOString().replaceAll(':','-')+'.json');fs.writeFileSync(out,JSON.stringify(report,null,2));console.log(JSON.stringify({out,summary:report.summary,counts,errors}));if(!Object.values(checks).every(Boolean))process.exitCode=1;
} catch(e){const out=path.join(root,'runs','watch-error-'+Date.now()+'.json');fs.writeFileSync(out,JSON.stringify({error:String(e),events,operations,environment:{node:process.version,platform:process.platform}},null,2));console.error(out);process.exitCode=1;}
finally{clearTimeout(timer);file?.close();directory?.close();fs.rmSync(tmp,{recursive:true,force:true});}
