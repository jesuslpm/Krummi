from pathlib import Path
import json,os,shutil,subprocess,zipfile,time
root=Path(__file__).resolve().parent;diag=root.parent;repo=diag.parent.parent;src=root/'src'
os.environ['PYTHONUTF8']='1'
while 'PROJECTION FOLLOWUP FINISHED' not in (root/'projection-final.log').read_text(encoding='utf-8-sig'):time.sleep(3)
def run(cmd,log,env=None):
    print('Running',cmd,flush=True)
    with (root/log).open('x',encoding='utf-8') as f:subprocess.run(cmd,stdout=f,stderr=subprocess.STDOUT,env=env,check=True)
paths=['crates/krummi-pager/src/journal.rs','crates/krummi/src/lib.rs','crates/krummi/src/table/mod.rs','crates/krummi/src/table/rows.rs','crates/krummi-bench/src/worker/tables.rs']
with zipfile.ZipFile(root/'projection-source.zip','x',zipfile.ZIP_DEFLATED) as z:
    for p in paths:z.write(src/p,p)
    z.write(src/'crates/krummi/tests/diagnostic_projection.rs','crates/krummi/tests/diagnostic_projection.rs')
for p in paths:shutil.copy2(repo/p,src/p)
env=dict(os.environ,CARGO_TARGET_DIR=str(diag/'target'),CARGO_PROFILE_RELEASE_DEBUG='line-tables-only')
build=['cargo','build','--manifest-path',str(src/'Cargo.toml'),'--locked','--offline','--release','-p','krummi-bench','--bin','krummi-bench']
p=src/'crates/krummi-pager/src/journal.rs'
s=(root/'journal-sampling-final.rs').read_text(encoding='utf-8')
s=s.replace('#[cfg(test)] eprintln!("sample raw={} encoded={n}",sample.len());','eprintln!("sample total={} raw={} encoded={n}",raw.len(),sample.len());')
p.write_text(s,encoding='utf-8')
run(build,'build-compression-observe.log',env)
shutil.copy2(diag/'target/release/krummi-bench.exe',diag/'bin/r2-compression-observe.exe')
run(['python','-u',str(diag/'run.py'),'r2-compression-observe','r2-compression-observe','W17','1'],'compression-observe.log')
s=(repo/'crates/krummi-pager/src/journal.rs').read_text(encoding='utf-8')
old='''    let compressed = match lz4_flex::block::compress_into(raw, &mut payload[..bound]) {
        Ok(n) if n < raw_len => Some(n),
        _ => None,
    };'''
new='''    // Diagnostic ceiling only: violates compression requirements for compressible large records.
    let compressed = if raw_len >= 1024*1024 { None } else {
        match lz4_flex::block::compress_into(raw, &mut payload[..bound]) {
            Ok(n) if n < raw_len => Some(n),
            _ => None,
        }
    };'''
assert old in s;s=s.replace(old,new,1);p.write_text(s,encoding='utf-8');(root/'journal-ceiling.rs').write_text(s,encoding='utf-8')
run(build,'build-compression-ceiling.log',env)
shutil.copy2(diag/'target/release/krummi-bench.exe',diag/'bin/r2-compression-ceiling.exe')
run(['python','-u',str(diag/'run.py'),'r2-compression-ceiling-ab','baseline,r2-compression-ceiling','W17','4'],'compression-ceiling-ab.log')
cmd=[str(diag/'bin/baseline.exe'),'run','krummi','W18','full',str(root/'verify-ceiling-W18'),str(diag/'r2-compression-ceiling-ab/3-r2-compression-ceiling-W17/rep-3')]
print('Running',cmd,flush=True)
result=subprocess.run(cmd,env=dict(os.environ,KRUMMI_BENCH_ROLE='worker'),capture_output=True,text=True)
(root/'verify-ceiling-W18.json').write_text(json.dumps({'command':cmd,'exit':result.returncode,'stdout':result.stdout,'stderr':result.stderr},indent=2))
assert result.returncode==0,result.stderr
shutil.copy2(repo/'crates/krummi-pager/src/journal.rs',p)
with zipfile.ZipFile(diag/'before-projection.zip') as z:
    for name in ['crates/krummi-pager/src/lib.rs','crates/krummi-pager/src/scratch.rs','crates/krummi-tx/src/transaction.rs']:
        (src/name).write_bytes(z.read(name))
p=src/'crates/krummi-pager/src/scratch.rs';s=p.read_text(encoding='utf-8')
s+='''
#[cfg(test)] mod diagnostic_run_pool_checks {
    use super::*;
    #[test] fn diagnostic_run_pool_cap_zeroing_and_allocation_failure() {
        RUN_POOL.with(|p|*p.borrow_mut()=(0,Vec::new()));
        let large=alloc_run(4097).unwrap();keep_runs(vec![(0,large)]);
        RUN_POOL.with(|p|assert_eq!(p.borrow().0,0));
        let first=alloc_run(4096).unwrap();let overflow=alloc_run(9).unwrap();
        keep_runs(vec![(0,first),(4096,overflow)]);
        RUN_POOL.with(|p|{let p=p.borrow();assert_eq!(p.0,32*1024*1024);assert_eq!(p.1.len(),1);});
        RUN_POOL.with(|p|*p.borrow_mut()=(0,Vec::new()));
        let mut run=alloc_run(9).unwrap();run.fill(0xAB);let pointer=run.as_ptr();keep_runs(vec![(0,run)]);
        let run=alloc_run(9).unwrap();assert_eq!(pointer,run.as_ptr());assert!(run.iter().all(|&b|b==0));keep_runs(vec![(0,run)]);
        crate::sys::faults::reset();crate::sys::faults::inject(crate::sys::Op::Alloc,0,crate::sys::faults::Action::Fail(std::io::ErrorKind::OutOfMemory));
        assert!(matches!(alloc_run(9),Err(Error::OutOfMemory{..})));crate::sys::faults::reset();
        RUN_POOL.with(|p|assert_eq!(p.borrow().0,0));
    }
}
''';p.write_text(s,encoding='utf-8')
test='''
use krummi::{Store,StoreOptions,OpenMode};
#[test] fn diagnostic_pool_keeps_old_snapshot_and_accepts_large_transaction() {
 let store=Store::open_temporary(&StoreOptions::default()).unwrap();
 let mut w=store.write().unwrap();let tree=w.open_tree_with("t",OpenMode::CreateNew).unwrap();w.set(&tree,b"k",&vec![1u8;65536]).unwrap();w.commit().unwrap();
 let old=store.read().unwrap();let tree=old.open_tree("t").unwrap();let borrowed=old.get(&tree,b"k").unwrap().unwrap();let pointer=borrowed.as_ptr();
 for byte in [2u8,3,4] {let mut w=store.write().unwrap();let tree=w.open_tree("t").unwrap();w.set(&tree,b"k",&vec![byte;65536]).unwrap();w.commit().unwrap();assert_eq!(borrowed.as_ptr(),pointer);assert!(borrowed.iter().all(|&b|b==1));}
 let mut w=store.write().unwrap();let tree=w.open_tree("t").unwrap();w.set(&tree,b"large",&vec![9u8;40*1024*1024]).unwrap();w.commit().unwrap();
 let current=store.read().unwrap();let tree=current.open_tree("t").unwrap();let large=current.get(&tree,b"large").unwrap().unwrap();assert_eq!(large.len(),40*1024*1024);assert!(large.iter().all(|&b|b==9));
 assert!(borrowed.iter().all(|&b|b==1));
 let mut w=store.write().unwrap();let tree=w.open_tree("t").unwrap();w.set(&tree,b"k",&vec![7u8;65536]).unwrap();w.abort();
 let current=store.read().unwrap();let tree=current.open_tree("t").unwrap();assert!(current.get(&tree,b"k").unwrap().unwrap().iter().all(|&b|b==4));
}
'''
(src/'crates/krummi/tests/diagnostic_run_pool.rs').write_text(test,encoding='utf-8')
run(['cargo','test','--manifest-path',str(src/'Cargo.toml'),'--locked','--offline','--release','-p','krummi-pager','--lib','diagnostic_run_pool_cap_zeroing_and_allocation_failure','--','--nocapture'],'pool-cap-test.log',env)
run(['cargo','test','--manifest-path',str(src/'Cargo.toml'),'--locked','--offline','--release','-p','krummi','--test','diagnostic_run_pool','--','--nocapture'],'pool-snapshot-test.log',env)
print('FOLLOWUP CHECKS FINISHED',flush=True)
