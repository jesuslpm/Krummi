"""Serial, pinned single-load runs. Outputs and stores stay beneath this file."""
from pathlib import Path
import ctypes, hashlib, json, os, statistics, subprocess, sys, time, winreg
from ctypes import wintypes
root=Path(__file__).resolve().parent
repo=root.parent.parent
tag=sys.argv[1]
labels=sys.argv[2].split(',')
loads=sys.argv[3].split(',')
rounds=int(sys.argv[4]) if len(sys.argv)>4 else 2
dest=root/tag
dest.mkdir(exist_ok=False)
k32=ctypes.windll.kernel32
k32.GetCurrentProcess.restype=wintypes.HANDLE
k32.SetProcessAffinityMask.argtypes=[wintypes.HANDLE,ctypes.c_size_t]
k32.GetProcessAffinityMask.argtypes=[wintypes.HANDLE,ctypes.POINTER(ctypes.c_size_t),ctypes.POINTER(ctypes.c_size_t)]
assert k32.SetProcessAffinityMask(k32.GetCurrentProcess(),0xffff)
mask,total=ctypes.c_size_t(),ctypes.c_size_t()
assert k32.GetProcessAffinityMask(k32.GetCurrentProcess(),ctypes.byref(mask),ctypes.byref(total))
assert mask.value==0xffff
env=dict(os.environ,KRUMMI_BENCH_ROLE='worker')
vbase=(root/'v-baseline/VoronBench.dll') if (root/'v-baseline/VoronBench.dll').exists() else (root/'dotnet/bin/VoronBench/release/VoronBench.dll')
def executable(label):
    if label=='voron': return ['dotnet',str(vbase)],'voron'
    if label.startswith('v-'):return ['dotnet',str(root/label/'VoronBench.dll')],'voron'
    return [str(root/'bin'/f'{label}.exe')],'krummi'
results=[]
def worker(label,load,path,source=None,prepare=False):
    exe,engine=executable(label)
    cmd=exe+(['prepare',engine,'full',str(path)] if prepare else ['run',engine,load,'full',str(path)])
    if source:cmd.append(str(source))
    t=time.time()
    p=subprocess.run(cmd,env=env,capture_output=True,text=True)
    log=path.with_suffix('.log')
    log.write_text(p.stdout+p.stderr,encoding='utf-8')
    item={'label':label,'load':load,'command':cmd,'exit':p.returncode,'wall_s':time.time()-t,'output':p.stdout,'stderr':p.stderr}
    results.append(item)
    (dest/'results.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
    if p.returncode:raise RuntimeError((cmd,p.stderr))
    rows=[]
    for line in p.stdout.splitlines():
        f=line.split()
        if f and f[0]=='rep':rows.append({'index':int(f[1]),'ops':int(f[3]),'ns':int(f[4]),'ns_op':int(f[4])/int(f[3])})
    item['reps']=rows
    if rows:
        vals=[r['ns_op'] for r in rows if r['index']>0]
        item['median_ns_op']=statistics.median(vals)
        print(label,load,'ns/op',round(item['median_ns_op'],3),'reps',','.join(str(round(v,3)) for v in vals),flush=True)
    (dest/'results.json').write_text(json.dumps(results,indent=2),encoding='utf-8')
    return item
with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE,r'HARDWARE\DESCRIPTION\System\CentralProcessor\0') as key:
    cpu=winreg.QueryValueEx(key,'ProcessorNameString')[0]
meta={'machine':os.environ.get('COMPUTERNAME'),'cpu':cpu,'os':sys.getwindowsversion()[:], 'affinity':hex(mask.value), 'started':time.strftime('%Y-%m-%dT%H:%M:%S%z'), 'labels':labels,'loads':loads}
for label in labels:
    exe,engine=executable(label)
    path=Path(exe[-1])
    meta[label]={'binary':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest()}
meta['power']=subprocess.check_output(['powercfg','/getactivescheme'],text=True)
meta['cores']=subprocess.check_output(['dotnet',str(vbase),'cores'],text=True)
(dest/'machine.json').write_text(json.dumps(meta,indent=2))
for engine in dict.fromkeys(executable(l)[1] for l in labels):
    label='baseline' if engine=='krummi' else 'voron'
    if 'W11' in loads and not (root/(engine+'-base')).exists():
        worker(label,'prepare',root/(engine+'-base'),prepare=True)
    if any(l in ['W26','W27','W28'] for l in loads) and not (root/(engine+'-table')).exists():
        worker(label,'W23',root/(engine+'-table'))
for r in range(rounds):
    for load in loads:
        for label in (labels if r%2==0 else labels[::-1]):
            engine=executable(label)[1]
            source=root/(engine+'-table')/'rep-3' if load in ['W26','W27','W28'] else root/(engine+'-base') if load=='W11' else None
            worker(label,load,dest/f'{r}-{label}-{load}',source)
print('FINISHED',tag,flush=True)
