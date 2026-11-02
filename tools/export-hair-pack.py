#!/usr/bin/env python3
"""Development-only MediaPipe multiclass hair export, with TFLite/ONNX references.

The original HairSegmenter has unsupported custom pool/unpool operators. This
Apache-2.0 official alternative supports full-body and multi-person inputs.
Install tools/hair-export-requirements.txt in an isolated Python 3.12 environment.
"""
import argparse,hashlib,json,subprocess,sys
from pathlib import Path
import numpy as np
import tensorflow as tf
import onnx,onnxruntime as ort
from onnx import helper,TensorProto,numpy_helper
from PIL import Image
from flatbuffers.table import Table
from flatbuffers import encode,packer,number_types as N
from tensorflow.lite.python import schema_py_generated as tfl
URL='https://storage.googleapis.com/mediapipe-models/image_segmenter/selfie_multiclass_256x256/float32/latest/selfie_multiclass_256x256.tflite'
LICENSE='https://storage.googleapis.com/mediapipe-assets/Model%20Card%20Multiclass%20Segmentation.pdf'
def sha(p):
 with Path(p).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def normalization(path):
 model=tfl.Model.GetRootAsModel(path.read_bytes(),0)
 b=None
 for i in range(model.MetadataLength()):
  a=model.Metadata(i)
  if a.Name()==b'TFLITE_METADATA':b=bytearray(model.Buffers(a.Buffer()).DataAsNumpy().tobytes())
 if b is None:raise ValueError('Missing upstream preprocessing metadata')
 def first(t,slot):o=t.Offset(4+slot*2);return Table(b,t.Indirect(t.Vector(o)))
 root=Table(b,encode.Get(packer.uoffset,b,0));unit=first(first(first(root,3),2),4);o=unit.Offset(6);norm=Table(b,unit.Indirect(unit.Pos+o))
 return [norm.GetVectorAsNumpy(N.Float32Flags,norm.Offset(4+slot*2)).tolist() for slot in [0,1]]
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('tflite',type=Path);p.add_argument('output',type=Path);p.add_argument('images',type=Path,nargs='+');args=p.parse_args();args.output.mkdir(parents=True,exist_ok=False)
 if normalization(args.tflite)!=[[127.5],[127.5]]:raise ValueError('Upstream normalization changed; review the adapter')
 raw=args.output/'reference.onnx';subprocess.run([sys.executable,'-m','tf2onnx.convert','--tflite',str(args.tflite),'--output',str(raw),'--opset','17'],check=True)
 graph=onnx.load(raw);input_name=graph.graph.input[0].name;original=graph.graph.output[0].name
 # Category argmax uses the complete six-class logits, exactly as the reference.
 graph.graph.node.extend([helper.make_node('ArgMax',[original],['category'],axis=3,keepdims=1),helper.make_node('Equal',['category','hair_class'],['is_hair']),helper.make_node('Cast',['is_hair'],['hair_nhwc'],to=TensorProto.FLOAT),helper.make_node('Transpose',['hair_nhwc'],['mask'],perm=[0,3,1,2])]);graph.graph.initializer.append(numpy_helper.from_array(np.asarray(1,np.int64),'hair_class'));del graph.graph.output[:];graph.graph.output.append(helper.make_tensor_value_info('mask',TensorProto.FLOAT,[1,1,256,256]));onnx.checker.check_model(graph);output=args.output/'model.onnx';onnx.save(graph,output)
 opts=ort.SessionOptions();opts.intra_op_num_threads=1;opts.inter_op_num_threads=1
 reference=ort.InferenceSession(str(raw),opts,providers=['CPUExecutionProvider']);session=ort.InferenceSession(str(output),opts,providers=['CPUExecutionProvider']);m=tf.lite.Interpreter(model_path=str(args.tflite),num_threads=1);m.allocate_tensors();cases=[]
 for image in args.images:
  x=np.asarray(Image.open(image).convert('RGB').resize((256,256),Image.Resampling.BILINEAR),np.float32)[None]/np.float32(127.5)-np.float32(1)
  m.set_tensor(m.get_input_details()[0]['index'],x);m.invoke();y=m.get_tensor(m.get_output_details()[0]['index']);actual=reference.run(None,{input_name:x})[0];expected=(y.argmax(-1)==1);mask=session.run(None,{input_name:x})[0][:,0]>.5;union=np.logical_or(expected,mask).sum();iou=float(np.logical_and(expected,mask).sum()/union) if union else 1.;error=float(np.max(np.abs(y-actual)));passed=np.allclose(y,actual,atol=.003,rtol=.003) and iou>=.99
  cases.append({'imageSha256':sha(image),'maximumAbsoluteError':error,'maskIoU':iou,'passed':bool(passed),'scope':'Original TFLite/ONNX tensor and hair-category parity; not annotated accuracy'})
 report={'schemaVersion':1,'adapter':'hair_segmenter_v1','upstream':URL,'upstreamSha256':sha(args.tflite),'modelSha256':sha(output),'normalization':{'mean':[127.5],'std':[127.5]},'tensorflow':tf.__version__,'onnxruntime':ort.__version__,'cases':cases,'passed':False,'note':'Native qualification must pass separately before activation.'};report_path=args.output/'qualification.json';report_path.write_text(json.dumps(report,indent=2)+'\n');raw.unlink()
 artifact=lambda path:{'name':path.name,'sha256':sha(path),'byteSize':str(path.stat().st_size),'sourceUrl':URL}
 pack={'schemaVersion':2,'id':'mediapipe-selfie-multiclass-hair-'+sha(args.tflite)[:12],'task':'hair','adapter':'hair_segmenter_v1','inputs':[{'name':input_name,'shape':[1,256,256,3],'dtype':'float32'}],'outputs':[{'name':'mask','shape':[1,1,256,256],'dtype':'float32'}],'artifacts':[artifact(output),artifact(report_path)],'license':'Apache-2.0','licenseUrl':LICENSE,'preprocessing':json.dumps({'resampling':'pillow_bilinear_v1','imageInput':input_name,'width':256,'height':256,'layout':'nhwc','scale':1/127.5,'mean':[1,1,1],'std':[1,1,1],'output':'mask','threshold':.5,'constants':{}},separators=(',',':')),'qualification':'pending_native'};(args.output/'manifest.json').write_text(json.dumps(pack,indent=2)+'\n');print(json.dumps(report,indent=2));sys.exit(0 if all(c['passed'] for c in cases) else 1)
if __name__=='__main__':main()
