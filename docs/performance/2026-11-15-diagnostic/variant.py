from pathlib import Path
import shutil,subprocess,sys,zipfile,difflib
root=Path(__file__).resolve().parent
repo=root.parent.parent
src=root/'src'
mode=sys.argv[1]
paths=['crates/krummi-pager/src/lib.rs','crates/krummi-pager/src/journal.rs','crates/krummi-pager/src/temp.rs','crates/krummi-pager/src/scratch.rs','crates/krummi-tx/src/lib.rs','crates/krummi-tx/src/transaction.rs','crates/krummi-tree/src/lib.rs','crates/krummi-tree/src/multi.rs','crates/krummi/src/lib.rs','crates/krummi/src/table/rows.rs','crates/krummi-bench/src/worker.rs','crates/krummi-bench/src/worker/tables.rs','crates/krummi-bench/src/data.rs','crates/krummi/src/table/row.rs']
archive=root/f'before-{mode}.zip'
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED) as z:
    for p in paths:z.write(src/p,p)
    z.write(src/'crates/krummi-pager/src/diag.rs','crates/krummi-pager/src/diag.rs')
for p in paths:shutil.copy2(repo/p,src/p)
if mode=='diff':
    p=src/'crates/krummi-pager/src/journal.rs'
    s=p.read_text(); original=s
    s=s.replace('''    while word < WORDS {
        if !differs(word) {''','''    while word < WORDS {
        // Diagnostic variant: skip equal blocks without changing word-level runs.
        while word + 8 <= WORDS && match base {
            Some(base) => page[word*8..word*8+64] == base[word*8..word*8+64],
            None => page[word*8..word*8+64] == [0u8;64],
        } { word += 8; }
        if word == WORDS { break; }
        if !differs(word) {''',1)
    s=s.replace('''        while word < WORDS && differs(word) {
            word += 1;
        }''','''        while word < WORDS && differs(word) {
            word += 1;
            while word + 4 <= WORDS && differs(word) && differs(word+1) && differs(word+2) && differs(word+3) {
                word += 4;
            }
        }''',1)
    assert s!=original
    p.write_text(s)
    (root/'diff-variant.patch').write_text(''.join(difflib.unified_diff(original.splitlines(True),s.splitlines(True),fromfile='baseline/journal.rs',tofile='variant/journal.rs')))
elif mode=='byte-count':
    p=src/'crates/krummi-bench/src/data.rs';s=p.read_text();start=s.index('pub fn byte_sum(');op=s.index('{',start);n=1;end=op+1
    while n:n+=(s[end]=='{')-(s[end]=='}');end+=1
    s=s[:start]+'#[inline(never)]\n'+s[start:op+1]+'\n    std::hint::black_box(bytes).len() as u64\n}'+s[end:]
    p.write_text(s)
elif mode=='prefix':
    p=src/'crates/krummi/src/table/rows.rs';s=p.read_text()
    s=s.replace('let in_order = row.len() == self.columns.len()','let in_order = row.len() <= self.columns.len()',1)
    s=s.replace('''        if in_order {
            let values = row''','''        if in_order {
            if let Some(&i) = self.key_columns.iter().find(|&&i| i >= row.len()) {
                return Err(self.columns[i].fault(ValueRule::MissingKeyColumn));
            }
            let mut values = row''',1)
    s=s.replace('''            let key = self.encode_key_of(&values)?;
            return Ok(Checked { values, key });''','''            for column in &self.columns[row.len()..] {
                values.push(if fill { Some(column.missing()?) } else { None });
            }
            let key = self.encode_key_of(&values)?;
            return Ok(Checked { values, key });''',1)
    old='''            let mut values = row
                .iter()
                .zip(&self.columns)
                .map(|((_, value), column)| {
                    normalize(&column.ty, column.nullable, value.reborrow())
                        .map(Some)
                        .map_err(|rule| column.fault(rule))
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;'''
    new='''            let mut values = Vec::with_capacity(self.columns.len());
            for ((_, value), column) in row.iter().zip(&self.columns) {
                values.push(Some(normalize(&column.ty, column.nullable, value.reborrow())
                    .map_err(|rule| column.fault(rule))?));
            }'''
    assert old in s;s=s.replace(old,new,1)
    p.write_text(s)
elif mode=='run-pool':
    p=src/'crates/krummi-pager/src/scratch.rs';s=p.read_text()
    old='''    let bytes = usize::try_from(requested).map_err(|_| out_of_memory())?;
    sys::alloc_zeroed(bytes).map_err(|_| out_of_memory())'''
    new='''    let bytes = usize::try_from(requested).map_err(|_| out_of_memory())?;
    let spare = RUN_POOL.with(|pool| {
        let mut pool = pool.borrow_mut();
        let at = pool.1.iter().position(|r| r.len() == bytes)?;
        pool.0 -= bytes;
        Some(pool.1.swap_remove(at))
    });
    if let Some(mut run) = spare {
        sys::alloc_point().map_err(|_| out_of_memory())?;
        run.fill(0);
        return Ok(run);
    }
    sys::alloc_zeroed(bytes).map_err(|_| out_of_memory())'''
    assert old in s;s=s.replace(old,new,1)
    s+='''
thread_local! { static RUN_POOL: std::cell::RefCell<(usize, Vec<RunBuf>)> = const { std::cell::RefCell::new((0,Vec::new())) }; }
#[doc(hidden)]
pub fn keep_runs(runs: Vec<(u64,RunBuf)>) {
    RUN_POOL.with(|pool| {let mut pool=pool.borrow_mut();for (_,run) in runs {if pool.0+run.len() <= 32*1024*1024 {pool.0+=run.len();pool.1.push(run);}}});
}
'''
    p.write_text(s)
    p=src/'crates/krummi-pager/src/lib.rs';s=p.read_text();p.write_text(s+'\n#[doc(hidden)]\npub use scratch::keep_runs;\n')
    p=src/'crates/krummi-tx/src/transaction.rs';s=p.read_text();s=s.replace('let committed = self.writer.commit(&pages, &created, &runs);','let committed = self.writer.commit(&pages, &created, &runs);\n        krummi_pager::keep_runs(runs);',1);p.write_text(s)
elif mode=='short-sum':
    p=src/'crates/krummi-bench/src/data.rs';s=p.read_text()
    s=s.replace('pub fn byte_sum(bytes: &[u8]) -> u64 {','pub fn byte_sum(bytes: &[u8]) -> u64 {\n    if bytes.len() <= 16 { return u64::from(bytes.iter().map(|&v|u32::from(v)).sum::<u32>()); }',1)
    p.write_text(s)
elif mode=='projection':
    p=src/'crates/krummi/src/table/rows.rs';s=p.read_text()
    s+='''
// Diagnostic upper bound for the six known benchmark columns in their current schema version.
impl<'t> TableRow<'t> {
 #[doc(hidden)] #[inline]
 pub fn diagnostic_benchmark_columns(&self) -> Option<([u8;16],i64,f64,i64,bool,&'t [u8])> {
  let Stored::Row{key,bytes,old:None,..}=&self.stored else{return None};
  let reader=&*self.reader;
  let at=|i:usize|reader.layout.slot_bytes(bytes,reader.fast[i].slot);
  Some((key[1..17].try_into().ok()?,i64::from_le_bytes(at(1)?.try_into().ok()?),f64::from_le_bytes(at(2)?.try_into().ok()?),i64::from_le_bytes(at(3)?.try_into().ok()?),at(4)?==[1],at(5)?))
 }
}
''';p.write_text(s)
    p=src/'crates/krummi-bench/src/worker/tables.rs';s=p.read_text();s=s.replace('fn checksum(row: &TableRow<\'_>) -> Result<u64> {','''fn checksum(row: &TableRow<'_>) -> Result<u64> {
    if let Some((id,n,x,ts,flag,name))=row.diagnostic_benchmark_columns() {
        return Ok(table_data::checksum(&id,n,x,ts,flag,name));
    }''',1);p.write_text(s)
else:raise ValueError(mode)
print('Prepared',mode)
