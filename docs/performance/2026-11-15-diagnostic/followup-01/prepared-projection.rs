// Diagnostic runtime projection. It stores no benchmark schema or constant column offsets.
pub struct DiagnosticProjection {
    reader: Arc<Reader>,
    owner: u64,
    versions: Vec<(u16, Vec<DiagnosticSource>)>,
    last: usize,
}

#[derive(Clone, Copy)]
enum DiagnosticSource { Stored(Slot), Key(usize), Default, Owned }

#[derive(Clone, Copy)]
pub struct DiagnosticColumn<T> {
    owner: u64,
    position: usize,
    marker: std::marker::PhantomData<fn() -> T>,
}

pub trait DiagnosticType {
    const TYPE: ColumnType;
    type Output<'t>;
    fn bytes<'t>(bytes: &'t [u8]) -> Self::Output<'t>;
    fn value<'t>(value: Value<'t>) -> Self::Output<'t>;
    fn key<'t>(bytes: &[u8]) -> Self::Output<'t> {
        Self::value(key_value(&Self::TYPE, bytes))
    }
}

macro_rules! diagnostic_scalar {
    ($name:ident,$ty:ident,$out:ty,$read:expr) => {
        #[derive(Clone,Copy)] pub struct $name;
        impl DiagnosticType for $name {
            const TYPE: ColumnType=ColumnType::$ty;
            type Output<'t>=$out;
            #[inline] fn bytes<'t>(bytes:&'t [u8])->$out { ($read)(bytes) }
            #[inline] fn value<'t>(value:Value<'t>)->$out {match value {Value::$ty(v)=>v,_=>panic!("a prepared column's type")}}
        }
    }
}
diagnostic_scalar!(DiagnosticI64,I64,i64,|b:&[u8]|i64::from_le_bytes(slot_array(b)));
diagnostic_scalar!(DiagnosticF64,F64,f64,|b:&[u8]|f64::from_le_bytes(slot_array(b)));
diagnostic_scalar!(DiagnosticBool,Bool,bool,|b:&[u8]|b[0]!=0);
diagnostic_scalar!(DiagnosticTimestamp,Timestamp,i128,|b:&[u8]|i128::from(i64::from_le_bytes(slot_array(b)))*1000);

#[derive(Clone,Copy)] pub struct DiagnosticUlid;
impl DiagnosticType for DiagnosticUlid {
    const TYPE:ColumnType=ColumnType::Ulid;
    type Output<'t>=Ulid;
    #[inline] fn bytes<'t>(b:&'t [u8])->Ulid {Ulid(slot_array(b))}
    #[inline] fn value<'t>(value:Value<'t>)->Ulid {match value {Value::Ulid(v)=>v,_=>panic!("a prepared ulid")}}
    #[inline] fn key<'t>(b:&[u8])->Self::Output<'t> {Ulid(slot_array(&b[1..17]))}
}

#[derive(Clone,Copy)] pub struct DiagnosticTextBytes;
impl DiagnosticType for DiagnosticTextBytes {
    const TYPE:ColumnType=ColumnType::String;
    type Output<'t>=Cow<'t,[u8]>;
    #[inline] fn bytes<'t>(b:&'t [u8])->Cow<'t,[u8]> {Cow::Borrowed(b)}
    #[inline] fn value<'t>(value:Value<'t>)->Cow<'t,[u8]> {
        match value {Value::String(Cow::Borrowed(v))=>Cow::Borrowed(v.as_bytes()),Value::String(Cow::Owned(v))=>Cow::Owned(v.into_bytes()),_=>panic!("a prepared string")}
    }
}

impl DiagnosticProjection {
    #[inline(never)]
    pub fn new(row:&TableRow<'_>)->Self {
        static NEXT:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(1);
        let reader=match &row.reader {RowReader::Kept(_,reader)=>Arc::clone(reader),RowReader::Owned(reader)=>Arc::clone(reader)};
        Self {reader,owner:NEXT.fetch_add(1,std::sync::atomic::Ordering::Relaxed),versions:Vec::new(),last:0}
    }

    #[inline(never)]
    pub fn column<T:DiagnosticType>(&self,name:&str)->Option<DiagnosticColumn<T>> {
        let position=*self.reader.names.get(name)?;
        (self.reader.columns[position].ty==T::TYPE).then_some(DiagnosticColumn {owner:self.owner,position,marker:std::marker::PhantomData})
    }

    #[inline]
    pub fn bind<'p,'r:'p,'t>(&'p mut self,row:&'r TableRow<'t>)->DiagnosticBound<'p,'r,'t> {
        assert!(std::ptr::eq(&*self.reader,&*row.reader),"projection belongs to another schema");
        let (version,layout,bytes)=match &row.stored {
            Stored::Row {old,bytes,..}=>{let layout=old.as_ref().map_or(&self.reader.layout,|old|&old.layout);(layout.version(),layout,Some(*bytes))},
            Stored::Owned(_)=>(0,&self.reader.layout,None),
        };
        if self.versions.get(self.last).is_none_or(|p|p.0!=version) {
            self.last=match self.versions.iter().position(|p|p.0==version) {
                Some(index)=>index,
                None=>{
                    let sources=self.reader.columns.iter().enumerate().map(|(i,column)|{
                        match &row.stored {
                            Stored::Owned(_)=>DiagnosticSource::Owned,
                            Stored::Row {old,..}=>{
                                if let Some(k)=column.in_key {return DiagnosticSource::Key(k);}
                                let position=old.as_ref().map_or(Some(i),|old|old.places[i]);
                                position.map_or(DiagnosticSource::Default,|p|DiagnosticSource::Stored(layout.slot(p).expect("a stored column")))
                            }
                        }
                    }).collect();
                    self.versions.push((version,sources));self.versions.len()-1
                }
            };
        }
        DiagnosticBound {owner:self.owner,sources:&self.versions[self.last].1,row,layout,bytes}
    }
}

pub struct DiagnosticBound<'p,'r,'t> {
    owner:u64,
    sources:&'p [DiagnosticSource],
    row:&'r TableRow<'t>,
    layout:&'p RowLayout,
    bytes:Option<&'t [u8]>,
}

impl<'p,'r,'t> DiagnosticBound<'p,'r,'t> {
    #[inline]
    pub fn get<T:DiagnosticType>(&self,column:DiagnosticColumn<T>)->Option<T::Output<'t>> {
        assert_eq!(self.owner,column.owner,"column belongs to another projection");
        match self.sources[column.position] {
            DiagnosticSource::Stored(slot)=>self.layout.slot_bytes(self.bytes.expect("stored row"),slot).map(T::bytes),
            DiagnosticSource::Key(k)=>{
                let Stored::Row {key,starts,..}=&self.row.stored else {unreachable!()};
                let at=starts.as_ref().map_or(0,|starts|usize::from(starts[k]));
                Some(T::key(&key[at..]))
            },
            DiagnosticSource::Default | DiagnosticSource::Owned=>match self.row.value(column.position)? {Value::Null=>None,value=>Some(T::value(value))},
        }
    }
}
