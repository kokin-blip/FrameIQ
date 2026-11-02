#!/usr/bin/env python3
"""Compare task outputs in separate native/reference stages, without model overlap.

Development only. Uses the pinned export dependencies. Native workers run before
loading the reference ONNX session so the app's memory reserve stays authoritative.
"""
import argparse,hashlib,json,os,subprocess,tempfile
from pathlib import Path
import numpy as np
import onnxruntime as ort
from PIL import Image
def sha(p):
 with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def iou(a,b):
 inter=max(0,min(a[2],b[2])-max(a[0],b[0]))*max(0,min(a[3],b[3])-max(a[1],b[1]));return inter/max(1e-9,(a[2]-a[0])*(a[3]-a[1])+(b[2]-b[0])*(b[3]-b[1])-inter)
def inputs_for(image,profile,pack,rectangle):
 original=Image.open(image).convert('RGB');pw,ph=profile['width'],profile['height'];rw,rh=pw,ph
 if profile.get('letterbox'):
  scale=min(pw/original.width,ph/original.height);rw=max(1,round(original.width*scale));rh=max(1,round(original.height*scale))
 ox,oy=(0,0) if pack['task']=='detection' else ((pw-rw)//2,(ph-rh)//2)
 resized=original.resize((rw,rh),Image.Resampling.BILINEAR);pixels=np.zeros((ph,pw,3),np.float32);pixels[oy:oy+rh,ox:ox+rw]=(np.asarray(resized).astype(np.float32)*np.float32(profile['scale'])-np.asarray(profile['mean'],np.float32))/np.asarray(profile['std'],np.float32)
 if profile['layout']=='nchw':pixels=pixels.transpose(2,0,1)
 inputs={profile['imageInput']:pixels[None].copy()}
 for t in pack['inputs']:
  if t['name'] in inputs:continue
  c=profile['constants'][t['name']];n=int(np.prod(t['shape']))
  values=[rectangle['x']*rw+ox,rectangle['y']*rh+oy,(rectangle['x']+rectangle['width'])*rw+ox,(rectangle['y']+rectangle['height'])*rh+oy] if c.get('from')=='box' and rectangle else [ox,oy,ox+rw,oy+rh] if c.get('from')=='box' else [int(x>=ox and x<ox+rw and y>=oy and y<oy+rh) for y in range(ph) for x in range(pw)] if c.get('from')=='pixel_mask' else [original.height,original.width] if c.get('from')=='image_size' else [c['fill']]*n if 'fill' in c else c['values'];dtype={'float32':np.float32,'int64':np.int64,'bool':np.bool_}[t['dtype']];inputs[t['name']]=np.asarray(values,dtype).reshape(t['shape'])
 return original,inputs,(pw,ph,rw,rh,ox,oy)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('manifest',type=Path);p.add_argument('images',type=Path,nargs='+');p.add_argument('--worker',type=Path,required=True);args=p.parse_args();pack=json.loads(args.manifest.read_text());root=args.manifest.parent.resolve();profile=json.loads(pack['preprocessing']);graph=next(a for a in pack['artifacts'] if a['name'].endswith('.onnx'));report_path=root/'qualification.json';report=json.loads(report_path.read_text());assert report['modelSha256']==sha(root/graph['name'])
 checks=[]
 with tempfile.TemporaryDirectory(prefix='frameiq-native-qualification-') as tmp:
  native=[]
  for index,image in enumerate(args.images):
   mask=Path(tmp)/f'{index}.png';request={'kind':'run_task_pack','pack':pack,'directory':str(root),'image':str(image.resolve()),'rectangle':next((c.get('rectangle') for c in report['cases'] if c['imageSha256']==sha(image)),None),'mask_output':str(mask) if pack['task'] in ('mask','hair') else None};run=subprocess.run([str(args.worker.resolve())],input=json.dumps(request)+'\n',text=True,capture_output=True,timeout=120,env=os.environ);rows=[json.loads(x) for x in run.stdout.splitlines() if x];response=next((x['result'] for x in rows if x.get('kind')=='task_result'),None);check={'imageSha256':sha(image),'exitCode':run.returncode,'errors':[r for r in rows if r.get('kind')=='error'],'passed':False};native.append((image,mask,response,check));print(f'Native stage {index+1}/{len(args.images)}: '+('ran' if response else 'unavailable'),flush=True)
  options=ort.SessionOptions();options.intra_op_num_threads=1;options.inter_op_num_threads=1;session=ort.InferenceSession(str(root/graph['name']),sess_options=options,providers=['CPUExecutionProvider'])
  for image,mask,response,check in native:
   if response is not None:
    original,inputs,(pw,ph,rw,rh,ox,oy)=inputs_for(image,profile,pack,next((c.get('rectangle') for c in report['cases'] if c['imageSha256']==sha(image)),None));outputs=dict(zip([o.name for o in session.get_outputs()],session.run(None,inputs)));raw=outputs[profile['output']]
    if pack['task']=='embedding':
     expected=raw.flatten();expected/=np.linalg.norm(expected);actual=np.asarray(response['embedding']);cosine=float(np.dot(expected,actual));error=float(np.max(np.abs(expected-actual)));check.update(cosine=cosine,maximumAbsoluteError=error,passed=cosine>=.999 and error<=.003)
    elif pack['task'] in ('mask','hair'):
     expected=(raw.reshape(-1,*raw.shape[-2:])[0]>(0 if pack['task']=='mask' else profile.get('threshold',.35))).astype(np.uint8)*255;expected=Image.fromarray(expected).resize((pw,ph),Image.Resampling.NEAREST).crop((ox,oy,ox+rw,oy+rh)).resize(original.size,Image.Resampling.NEAREST);expected=np.asarray(expected)>127;actual=np.asarray(Image.open(mask))>127;union=np.logical_or(expected,actual).sum();score=float(np.logical_and(expected,actual).sum()/union) if union else 1.;check.update(maskIoU=score,passed=score>=.99)
    else:
     boxes=outputs[profile['boxes']][0];logits=raw[0];scores=1/(1+np.exp(-np.clip(logits,-80,80)));detections=[]
     for box,s in zip(boxes,scores):
      class_scores=[float(s[a:b].max()) for a,b in profile['tokenRanges']];label=int(np.argmax(class_scores));score=class_scores[label]
      if score<profile.get('threshold',.35):continue
      cx,cy,w,h=box;rect=[max(0,(float(cx-w/2)*pw-ox)/rw),max(0,(float(cy-h/2)*ph-oy)/rh),min(1,(float(cx+w/2)*pw-ox)/rw),min(1,(float(cy+h/2)*ph-oy)/rh)]
      if rect[2]>rect[0] and rect[3]>rect[1]:detections.append((score,profile['labels'][label],rect))
     kept=[]
     for d in sorted(detections,reverse=True):
      if not any(d[1]==old[1] and iou(d[2],old[2])>.5 for old in kept):kept.append(d)
      if len(kept)>=64:break
     actual=response['detections'];errors=[]
     for score,label,b in kept:
      diffs=[]
      for d in actual:
       if d['category']!=label:continue
       r=d['rectangle'];v=[r['x'],r['y'],r['x']+r['width'],r['y']+r['height']];diffs.append(max(abs(x-y)*size for x,y,size in zip(b,v,[original.width,original.height]*2)))
      errors.append(min(diffs) if diffs else float('inf'))
     check.update(referenceDetections=len(kept),nativeDetections=len(actual),categories=sorted({d["category"] for d in actual}),detections=actual,maximumSourcePixelError=max(errors,default=0.) if all(np.isfinite(e) for e in errors) else None,passed=len(kept)==len(actual) and max(errors,default=0.)<=1.)
   checks.append(check)
 report['workerSha256']=sha(args.worker);report['preprocessingSha256']=hashlib.sha256(pack['preprocessing'].encode()).hexdigest();report['nativeCases']=checks;report['passed']=all(c.get('passed') for c in report['cases']+checks) and report.get('referencePassed',True);report['native']=None;report_path.write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
 for a in pack['artifacts']:
  if a['name']=='qualification.json':a.update(sha256=sha(report_path),byteSize=str(report_path.stat().st_size))
 pack['qualification']='reference_checked' if report['passed'] else 'failed';args.manifest.write_text(json.dumps(pack,indent=2)+'\n');print(json.dumps({'passed':report['passed'],'nativeCases':checks},indent=2,allow_nan=False))
 if not report['passed']:raise SystemExit(1)
if __name__=='__main__':main()
