#!/usr/bin/env python3
"""Export pinned upstream vision models and retain numerical qualification evidence.

Development only. Runtime uses the app's supervised ONNX worker, never Python.
Install the pinned requirements in tools/fashion-export-requirements.txt first.
"""
import argparse, hashlib, json, os, subprocess, sys
import onnx
from pathlib import Path
import numpy as np
import torch
import onnxruntime as ort
from PIL import Image
from huggingface_hub import model_info

MODELS = {
    "embedding": ("google/siglip2-base-patch16-224", "siglip2_image_v1", 224),
    "detection": ("IDEA-Research/grounding-dino-tiny", "grounding_dino_v1", 800),
    "mask": ("facebook/sam2.1-hiera-tiny", "sam2_image_v1", 1024),
}
LABELS = ["dress", "top", "shirt", "outerwear", "trousers", "skirt", "footwear", "bag", "jewelry", "sunglasses", "hat", "hair_accessory", "belt", "hosiery", "accessory", "hair"]
PROMPTS = ["dress", "top", "shirt", "jacket", "pants", "skirt", "shoes", "bag", "jewelry", "sunglasses", "hat", "hair accessory", "belt", "stockings", "accessory", "hair"]
def digest(path):
    with Path(path).open("rb") as f:
        return hashlib.file_digest(f, "sha256").hexdigest()
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("task", choices=MODELS)
    parser.add_argument("output", type=Path)
    parser.add_argument("images", type=Path, nargs="+")
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--mask-family",choices=["sam2","sam_vit_b"],default="sam2")
    parser.add_argument("--regions",type=Path,help="Reviewed source-normalized mask prompt rectangles keyed by image SHA-256")
    parser.add_argument("--simplify",action="store_true",help="Fold fixed-shape control flow; reference comparisons must still pass")
    parser.add_argument("--reuse-export",action="store_true",help="Recheck an existing generated graph against its upstream reference")
    parser.add_argument("--revision", help="Upstream commit; resolved once and pinned when omitted")
    args=parser.parse_args()
    if args.output.exists() and any(args.output.iterdir()) and not args.reuse_export:
        parser.error("Choose an empty output directory; existing artifacts are preserved.")
    args.output.mkdir(parents=True, exist_ok=True)
    regions=json.loads(args.regions.read_text()) if args.regions else {}
    torch.set_num_threads(1)
    repo,adapter,size=MODELS[args.task]
    if args.task=="mask" and args.mask_family=="sam_vit_b":repo,adapter="facebook/sam-vit-base","sam_image_v1"
    prior_report=args.output/"qualification.json"
    revision=args.revision or (json.loads(prior_report.read_text()).get("upstreamRevision") if args.reuse_export and prior_report.exists() else None) or model_info(repo).sha
    means=[0.5]*3 if args.task=="embedding" else [0.485,0.456,0.406]
    stds=[0.5]*3 if args.task=="embedding" else [0.229,0.224,0.225]
    profile={"resampling":"pillow_bilinear_v1","imageInput":"pixel_values","width":size,"height":size,"layout":"nchw","scale":1/255,"mean":means,"std":stds,"letterbox":args.task=="detection","output":"embedding" if args.task=="embedding" else "logits" if args.task=="detection" else "mask","constants":{}}
    from transformers import AutoConfig, AutoModel, AutoTokenizer, SiglipVisionModel, Sam2Model, SamModel, GroundingDinoForObjectDetection
    if args.task=="embedding":
        model=SiglipVisionModel.from_pretrained(repo,revision=revision).eval()
        class Wrapper(torch.nn.Module):
            def __init__(self,m):super().__init__();self.m=m
            def forward(self,pixel_values):return self.m(pixel_values=pixel_values).pooler_output
        wrapper=Wrapper(model); names=["pixel_values"];output_names=["embedding"];extra=[]
    elif args.task=="detection":
        config=AutoConfig.from_pretrained(repo,revision=revision)
        config._attn_implementation="eager";config.text_config._attn_implementation="eager";config.disable_custom_kernels=True
        model=GroundingDinoForObjectDetection.from_pretrained(repo,revision=revision,config=config).eval()
        tokenizer=AutoTokenizer.from_pretrained(repo,revision=revision)
        prompt=" . ".join(PROMPTS)+" ."
        encoded=tokenizer(prompt,truncation=True,max_length=256,return_tensors="pt",return_offsets_mapping=True)
        ranges=[];start=0
        for phrase in PROMPTS:
            begin=prompt.index(phrase,start);end=begin+len(phrase);start=end
            indices=[i for i,(a,b) in enumerate(encoded["offset_mapping"][0].tolist()) if a<end and b>begin]
            ranges.append([min(indices),max(indices)+1])
        input_ids=encoded["input_ids"];attention_mask=encoded["attention_mask"];token_type_ids=encoded.get("token_type_ids",torch.zeros_like(input_ids))
        class Wrapper(torch.nn.Module):
            def __init__(self,m):super().__init__();self.m=m
            def forward(self,pixel_values,input_ids,attention_mask,token_type_ids,pixel_mask):
                r=self.m(pixel_values=pixel_values,input_ids=input_ids,attention_mask=attention_mask,token_type_ids=token_type_ids,pixel_mask=pixel_mask)
                return r.logits,r.pred_boxes
        wrapper=Wrapper(model);names=["pixel_values","input_ids","attention_mask","token_type_ids","pixel_mask"];output_names=["logits","boxes"];extra=[input_ids,attention_mask,token_type_ids,torch.ones((1,size,size),dtype=torch.int64)]
        for name,t in zip(names[1:],extra):profile["constants"][name]={"values":t.flatten().tolist()}
        profile["constants"]["pixel_mask"]={"from":"pixel_mask"}
        profile.update(boxes="boxes",labels=LABELS,tokenRanges=ranges,threshold=0.35)
    else:
        model=(SamModel if args.mask_family=="sam_vit_b" else Sam2Model).from_pretrained(repo,revision=revision).eval()
        class Wrapper(torch.nn.Module):
            def __init__(self,m):super().__init__();self.m=m
            def forward(self,pixel_values,input_boxes):
                return self.m(pixel_values=pixel_values,input_boxes=input_boxes,multimask_output=False).pred_masks
        wrapper=Wrapper(model);names=["pixel_values","input_boxes"];output_names=["mask"];extra=[torch.tensor([[[0.,0.,float(size),float(size)]]])]
        profile["constants"]["input_boxes"]={"from":"box"}
    wrapper.eval()
    def pixels(path):
        if args.task=="mask" and regions:
            box=regions[digest(path)];x,y,w,h=[box[k] for k in ["x","y","width","height"]];extra[0]=torch.tensor([[[x*size,y*size,(x+w)*size,(y+h)*size]]],dtype=torch.float32)
        image=Image.open(path).convert("RGB")
        rw,rh=size,size
        if profile["letterbox"]:
            scale=min(size/image.width,size/image.height);rw=max(1,round(image.width*scale));rh=max(1,round(image.height*scale))
        ox,oy=(0,0) if args.task=="detection" else ((size-rw)//2,(size-rh)//2)
        resized=image.resize((rw,rh),Image.Resampling.BILINEAR)
        a=np.zeros((size,size,3),np.float32)
        a[oy:oy+rh,ox:ox+rw]=(np.asarray(resized).astype(np.float32)*np.float32(1/255)-np.asarray(means,dtype=np.float32))/np.asarray(stds,dtype=np.float32)
        if args.task=="detection":
            extra[-1].zero_();extra[-1][:,oy:oy+rh,ox:ox+rw]=1
        return torch.from_numpy(a.transpose(2,0,1)[None].copy())
    sample=(pixels(args.images[0]),*extra)
    model_path=args.output/"model.onnx"
    with torch.no_grad():
        reference=wrapper(*sample)
        if not args.reuse_export:torch.onnx.export(wrapper,sample,str(model_path),input_names=names,output_names=output_names,opset_version=17,dynamo=False,do_constant_folding=True)
    # Some exporters omit output dimension metadata even though every input and
    # multimask flag is fixed. Record the actual reference dimensions, then verify
    # the complete graph and every runtime output against those dimensions.
    proto=onnx.load(str(model_path),load_external_data=False)
    refs=list(reference) if isinstance(reference,(tuple,list)) else [reference]
    for output,tensor in zip(proto.graph.output,refs):
        output.type.tensor_type.shape.ClearField("dim")
        for n in tensor.shape:output.type.tensor_type.shape.dim.add().dim_value=int(n)
    onnx.checker.check_model(proto)
    if args.simplify:
        from onnxscript import optimizer
        # Only constants and fixed-shape control flow are folded. Every output is
        # compared with the upstream model below before the pack can qualify.
        proto=optimizer.optimize(proto,num_iterations=4)
        onnx.checker.check_model(proto)
    onnx.save(proto,str(model_path))
    session=ort.InferenceSession(str(model_path),providers=["CPUExecutionProvider"],sess_options=ort.SessionOptions())
    inputs=[{"name":i.name,"shape":i.shape,"dtype":"float32" if i.type=="tensor(float)" else "int64" if i.type=="tensor(int64)" else "bool"} for i in session.get_inputs()]
    outputs=[{"name":o.name,"shape":o.shape,"dtype":"float32"} for o in session.get_outputs()]
    if any(not isinstance(n,int) or n<=0 for t in inputs+outputs for n in t["shape"]):raise RuntimeError("Export still has unbounded tensors; do not activate it")
    # Export pruning can remove constant inputs; prune their metadata as well.
    exported_names={i["name"] for i in inputs}
    profile["constants"]={k:v for k,v in profile["constants"].items() if k in exported_names}
    cases=[]
    for image in args.images:
        tensors=(pixels(image),*extra)
        with torch.no_grad():ref=wrapper(*tensors)
        refs=list(ref) if isinstance(ref,(tuple,list)) else [ref]
        arrays={n:t.detach().cpu().numpy() for n,t in zip(names,tensors) if n in exported_names}
        actual=session.run(None,arrays)
        errors=[float(np.max(np.abs(a[np.isfinite(a)&np.isfinite(r.detach().numpy())]-r.detach().numpy()[np.isfinite(a)&np.isfinite(r.detach().numpy())]),initial=0.)) for a,r in zip(actual,refs)]
        passed=all(np.allclose(a,r.detach().numpy(),atol=0.003,rtol=0.003) for a,r in zip(actual,refs))
        decision={}
        if args.task=="mask":
            a=actual[0]>0;r=refs[0].detach().numpy()>0;union=np.logical_or(a,r).sum();iou=float(np.logical_and(a,r).sum()/union) if union else 1.
            decision={"maskIoU":iou,"maximumLogitErrorLimit":0.05,"minimumMaskIoU":0.99};passed=iou>=0.99 and max(errors)<=.05
        if args.task=="detection":
            # Decoder query order can differ when proposal scores tie. Compare the
            # actual thresholded, class-aware detections after NMS by assignment.
            def detections(logits,boxes):
                found=[]
                for b,row in zip(boxes[0],logits[0]):
                    scores=[float((1/(1+np.exp(-np.clip(row[a:z],-80,80)))).max()) for a,z in ranges];label=int(np.argmax(scores));score=scores[label]
                    if score>=.35:found.append((score,label,[float(b[0]-b[2]/2),float(b[1]-b[3]/2),float(b[0]+b[2]/2),float(b[1]+b[3]/2)]))
                def iou(a,b):
                    inter=max(0,min(a[2],b[2])-max(a[0],b[0]))*max(0,min(a[3],b[3])-max(a[1],b[1]));return inter/max(1e-9,(a[2]-a[0])*(a[3]-a[1])+(b[2]-b[0])*(b[3]-b[1])-inter)
                kept=[]
                for d in sorted(found,reverse=True):
                    if not any(d[1]==o[1] and iou(d[2],o[2])>.5 for o in kept):kept.append(d)
                return kept[:64]
            a=detections(*actual);r=detections(*[t.detach().numpy() for t in refs]);remaining=list(a);matches=[]
            for score,label,box in r:
                eligible=[(max(abs(x-y)*size for x,y in zip(box,b)),j,abs(score-s)) for j,(s,l,b) in enumerate(remaining) if l==label]
                if not eligible:matches.append((1e9,1.));continue
                distance,j,delta=min(eligible);remaining.pop(j);matches.append((distance,delta))
            decision={"referenceDetections":len(r),"exportDetections":len(a),"maximumModelPixelError":max([m[0] for m in matches],default=0.),"maximumScoreError":max([m[1] for m in matches],default=0.),"tensorParityPassed":bool(passed)}
            passed=len(a)==len(r) and decision["maximumModelPixelError"]<=1. and decision["maximumScoreError"]<=.01
        finite_outputs=all(np.all(np.isfinite(a)) for a in actual[1:]) and all(np.all(np.isfinite(t.detach().numpy())) for t in refs[1:]) and not np.isnan(actual[0]).any() and not np.isnan(refs[0].detach().numpy()).any()
        passed=passed and finite_outputs
        cases.append({**decision,"rectangle":regions.get(digest(image)),"finiteOutputs":bool(finite_outputs),"imageSha256":digest(image),"maximumAbsoluteErrors":errors,"passed":bool(passed),"scope":"Task-output PyTorch/ONNX Runtime reference comparison; tensor differences retained; not accuracy evaluation"})
    report={"schemaVersion":1,"adapter":adapter,"modelSha256":digest(model_path),"upstream":repo,"upstreamRevision":revision,"runtime":ort.__version__,"torch":torch.__version__,"passed":False,"referencePassed":all(c["passed"] for c in cases) and (args.task!="detection" or any(c.get("referenceDetections",0)>0 for c in cases)),"cases":cases,"native":None}
    report_path=args.output/"qualification.json";report_path.write_text(json.dumps(report,indent=2)+"\n")
    artifact=lambda p:{"name":p.name,"sha256":digest(p),"byteSize":str(p.stat().st_size),"sourceUrl":f"https://huggingface.co/{repo}/tree/{revision}"}
    pack={"schemaVersion":2,"id":repo.replace("/","-")+"-"+revision[:12],"task":args.task,"adapter":adapter,"inputs":inputs,"outputs":outputs,"artifacts":[artifact(model_path),artifact(report_path)],"license":"Apache-2.0","licenseUrl":f"https://huggingface.co/{repo}/tree/{revision}","preprocessing":json.dumps(profile,separators=(",",":")),"qualification":"reference_checked" if report["passed"] else "failed"}
    # Native inference runs in the second qualification command, after this
    # process releases its PyTorch/reference sessions and their memory.
    pack["qualification"]="pending_native" if report["referencePassed"] else "failed"
    (args.output/"manifest.json").write_text(json.dumps(pack,indent=2,allow_nan=False)+"\n")
    print(json.dumps({"manifest":str(args.output/"manifest.json"),"referencePassed":report["referencePassed"],"cases":cases},indent=2,allow_nan=False))
    if not report["referencePassed"]:sys.exit(1)

if __name__=="__main__":main()
