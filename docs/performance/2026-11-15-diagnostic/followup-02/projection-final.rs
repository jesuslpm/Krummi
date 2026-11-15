// Disposable diagnostic prototype: immutable per-schema projections, no format changes.
use std::marker::PhantomData as ProjectionPhantom;
use std::sync::Mutex as ProjectionMutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError { UnknownColumn(String), UnsupportedType(String), WrongType, Index, WrongSchema, WrongProjection, InvalidRow }
impl std::fmt::Display for ProjectionError {
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {write!(f,"{self:?}")}
}
impl std::error::Error for ProjectionError {}
type ProjectionResult<T> = std::result::Result<T,ProjectionError>;

// A store owns this cache. The persisted table/schema identities are never names or page numbers.
// Every retained Reader has its own Arc token, which cannot be reused while any projection keeps it.
#[derive(Default)]
pub(crate) struct ProjectionReaders {
    readers: ProjectionMutex<HashMap<(u64,u64,u16),Arc<Reader>>>,
}
impl ProjectionReaders {
    pub(crate) fn knows_schema(&self,table:u64,state:u64)->bool {
        self.readers.lock().unwrap_or_else(|p|p.into_inner()).keys().any(|key|key.0==table && key.1==state)
    }
    pub(crate) fn knows_table(&self,id:u64)->bool {
        self.readers.lock().unwrap_or_else(|p|p.into_inner()).keys().any(|key|key.0==id)
    }
    pub(crate) fn get<S:PageSource+?Sized>(&self,schema:&TableSchema,tx:&S,history:u64)->Result<Arc<Reader>> {
        let key=(schema.table_id(),schema.schema_state(),schema.version());
        let mut cache=self.readers.lock().unwrap_or_else(|p|p.into_inner());
        if let Some(reader)=cache.get(&key) {return Ok(Arc::clone(reader));}
        let reader=Reader::new(schema);
        let _=(tx,history);
        let reader=Arc::new(reader);
        cache.insert(key,Arc::clone(&reader));
        Ok(reader)
    }
}

#[derive(Clone,Copy,PartialEq,Eq)]
pub enum ProjectionTag { I64,F64,Bool,Timestamp,Ulid,Text,Bytes }
#[derive(Clone,Copy)]
struct ProjectionIndex {tag:ProjectionTag,slot:usize}
pub struct Projection {
    schema_owner:Arc<()>,
    owner:Arc<()>,
    indices:Vec<ProjectionIndex>,
    current:[ProjectionVersion;3],
    older:Vec<OnceLock<Arc<[ProjectionVersion;3]>>>,
    positions:Vec<usize>,
    owned:ProjectionVersion,
}
pub struct Col<T> {owner:Arc<()>,slot:usize,marker:ProjectionPhantom<fn()->T>}
pub struct Timestamp;
pub struct TextBytes;
pub struct Bytes;

mod projection_sealed {pub trait Sealed {}}
pub trait ProjectionType:projection_sealed::Sealed+Sized {
    type Output<'a>;
    const TAG:ProjectionTag;
    #[doc(hidden)] fn plans(version:&ProjectionVersion)->&[ProjectionAccess<Self>];
    #[doc(hidden)] fn bytes<'a>(bytes:&'a [u8])->Self::Output<'a>;
    #[doc(hidden)] fn key<'a>(bytes:&[u8])->Self::Output<'a>;
    #[doc(hidden)] fn value<'a>(value:&'a Value<'_>)->Option<Self::Output<'a>>;
    #[doc(hidden)] fn constant<'a>(bytes:&[u8])->Option<Self::Output<'a>>;
}

// Each getter is selected once at preparation: there is no SlotPlace/DiagnosticSource match per call.
pub struct ProjectionAccess<T:ProjectionType> {
    get:for<'a> fn(&ProjectionContext<'a>,&ProjectionAccess<T>)->Option<T::Output<'a>>,
    a:usize,b:usize,c:usize,width:usize,null_byte:usize,null_mask:u8,fast:u8,
    constant:[u8;16],is_null:bool,marker:ProjectionPhantom<fn()->T>,
}
pub struct ProjectionContext<'a> {
    bytes:&'a [u8],key:&'a [u8],starts:Option<&'a [u16]>,
    defaults:Option<&'a [Value<'a>]>,owned:&'a [Value<'static>],
}
#[derive(Default)]
pub struct ProjectionVersion {
    i64s:Vec<ProjectionAccess<i64>>, f64s:Vec<ProjectionAccess<f64>>, bools:Vec<ProjectionAccess<bool>>,
    timestamps:Vec<ProjectionAccess<Timestamp>>,ulids:Vec<ProjectionAccess<Ulid>>,
    texts:Vec<ProjectionAccess<TextBytes>>,bytes:Vec<ProjectionAccess<Bytes>>,
}
pub struct ProjectedRow<'p,'r> {owner:&'p Arc<()>,indices:&'p [ProjectionIndex],version:&'p ProjectionVersion,context:ProjectionContext<'r>}

macro_rules! projection_scalar {
    ($type:ty,$tag:ident,$field:ident,$variant:ident,$out:ty,$bytes:expr,$key:expr) => {
        impl projection_sealed::Sealed for $type {}
        impl ProjectionType for $type {
            type Output<'a>=$out;
            const TAG:ProjectionTag=ProjectionTag::$tag;
            #[inline] fn plans(v:&ProjectionVersion)->&[ProjectionAccess<Self>] {&v.$field}
            #[inline] fn bytes<'a>(b:&'a [u8])->$out {($bytes)(b)}
            #[inline] fn key<'a>(b:&[u8])->Self::Output<'a> {($key)(b)}
            #[inline] fn value<'a>(v:&'a Value<'_>)->Option<$out> {match v {Value::$variant(v)=>Some(*v),_=>None}}
            #[inline] fn constant<'a>(b:&[u8])->Option<Self::Output<'a>> {Some(($bytes)(b))}
        }
    }
}
projection_scalar!(i64,I64,i64s,I64,i64,|b:&[u8]|i64::from_le_bytes(slot_array(b)),|b:&[u8]|match key_value(&ColumnType::I64,b) {Value::I64(v)=>v,_=>unreachable!()});
projection_scalar!(f64,F64,f64s,F64,f64,|b:&[u8]|f64::from_le_bytes(slot_array(b)),|b:&[u8]|match key_value(&ColumnType::F64,b) {Value::F64(v)=>v,_=>unreachable!()});
projection_scalar!(bool,Bool,bools,Bool,bool,|b:&[u8]|b[0]!=0,|b:&[u8]|match key_value(&ColumnType::Bool,b) {Value::Bool(v)=>v,_=>unreachable!()});
projection_scalar!(Timestamp,Timestamp,timestamps,Timestamp,i128,|b:&[u8]|i128::from(i64::from_le_bytes(slot_array(b)))*1000,|b:&[u8]|match key_value(&ColumnType::Timestamp,b) {Value::Timestamp(v)=>v,_=>unreachable!()});
projection_scalar!(Ulid,Ulid,ulids,Ulid,Ulid,|b:&[u8]|Ulid(slot_array(b)),|b:&[u8]|Ulid(slot_array(&b[1..17])));
macro_rules! projection_bytes_type {
    ($type:ty,$tag:ident,$field:ident,$variant:ident,$convert:ident)=>{
        impl projection_sealed::Sealed for $type {}
        impl ProjectionType for $type {
            type Output<'a>=&'a [u8];
            const TAG:ProjectionTag=ProjectionTag::$tag;
            #[inline] fn plans(v:&ProjectionVersion)->&[ProjectionAccess<Self>] {&v.$field}
            #[inline] fn bytes<'a>(b:&'a [u8])->&'a [u8] {b}
            fn key<'a>(_:&[u8])->&'a [u8] {unreachable!("variable keys live in the row")}
            #[inline] fn value<'a>(v:&'a Value<'_>)->Option<&'a [u8]> {match v {Value::$variant(v)=>Some(v.$convert()),_=>None}}
            fn constant<'a>(_:&[u8])->Option<&'a [u8]> {None}
        }
    }
}
projection_bytes_type!(TextBytes,Text,texts,String,as_bytes);
projection_bytes_type!(Bytes,Bytes,bytes,Bytes,as_ref);

impl ProjectionTag {
    fn of(ty:ColumnType)->Option<Self> {Some(match ty {ColumnType::I64=>Self::I64,ColumnType::F64=>Self::F64,ColumnType::Bool=>Self::Bool,ColumnType::Timestamp=>Self::Timestamp,ColumnType::Ulid=>Self::Ulid,ColumnType::String=>Self::Text,ColumnType::Bytes=>Self::Bytes,_=>return None})}
    fn width(self)->usize {match self {Self::I64|Self::F64|Self::Timestamp=>8,Self::Bool=>1,Self::Ulid=>16,_=>0}}
}

impl ReadTable<'_> {
    #[inline(never)]
    pub fn projection(&self,names:&[&str])->ProjectionResult<Arc<Projection>> {
        let mut cache=self.reader.projections.lock().unwrap_or_else(|p|p.into_inner());
        if let Some((_,p))=cache.iter().find(|(key,_)|key.iter().map(String::as_str).eq(names.iter().copied())) {return Ok(Arc::clone(p));}
        let p=self.prepare_projection_uncached(names)?;
        cache.insert(names.iter().map(|s|(*s).to_owned()).collect(),Arc::clone(&p));
        Ok(p)
    }
    // For timing first construction without re-opening/re-building a store, never used by the load hot path.
    #[inline(never)]
    pub fn prepare_projection_uncached(&self,names:&[&str])->ProjectionResult<Arc<Projection>> {
        let reader=&*self.reader;
        let mut positions=Vec::new();let mut indices=Vec::new();let mut counts=[0usize;7];
        for name in names {
            let &position=reader.names.get(*name).ok_or_else(||ProjectionError::UnknownColumn((*name).into()))?;
            let tag=ProjectionTag::of(reader.columns[position].ty).ok_or_else(||ProjectionError::UnsupportedType((*name).into()))?;
            let slot=counts[tag as usize];counts[tag as usize]+=1;
            positions.push(position);indices.push(ProjectionIndex{tag,slot});
        }
        let current=std::array::from_fn(|flag|ProjectionVersion::prepare(reader,&positions,None,1<<flag,false));
        let older=(1..reader.layout.version()).map(|_|OnceLock::new()).collect();
        let owned=ProjectionVersion::prepare(reader,&positions,None,1,true);
        Ok(Arc::new(Projection{schema_owner:Arc::clone(&reader.projection_owner),owner:Arc::new(()),indices,current,older,positions,owned}))
    }
}
impl ProjectionVersion {
    fn prepare(reader:&Reader,positions:&[usize],old:Option<&OldVersion>,width:usize,owned:bool)->Self {
        let mut out=Self::default();
        for &position in positions {
            match ProjectionTag::of(reader.columns[position].ty).expect("validated at preparation") {
                ProjectionTag::I64=>out.i64s.push(ProjectionAccess::prepare(reader,position,old,width,owned)),
                ProjectionTag::F64=>out.f64s.push(ProjectionAccess::prepare(reader,position,old,width,owned)),
                ProjectionTag::Bool=>out.bools.push(ProjectionAccess::prepare(reader,position,old,width,owned)),
                ProjectionTag::Timestamp=>out.timestamps.push(ProjectionAccess::prepare(reader,position,old,width,owned)),
                ProjectionTag::Ulid=>out.ulids.push(ProjectionAccess::prepare(reader,position,old,width,owned)),
                ProjectionTag::Text=>out.texts.push(ProjectionAccess::prepare(reader,position,old,width,owned)),
                ProjectionTag::Bytes=>out.bytes.push(ProjectionAccess::prepare(reader,position,old,width,owned)),
            }
        } out
    }
}

impl<T:ProjectionType> ProjectionAccess<T> {
    fn prepare(reader:&Reader,position:usize,old:Option<&OldVersion>,offset_width:usize,owned:bool)->Self {
        let mut out=Self{get:Self::owned,a:position,b:0,c:0,width:T::TAG.width(),null_byte:0,null_mask:0,fast:0,constant:[0;16],is_null:false,marker:ProjectionPhantom};
        if owned {return out;}
        let column=&reader.columns[position];
        if let Some(k)=column.in_key {out.a=k;out.get=Self::key;out.fast=2;return out;}
        let (layout,place)=old.map_or((&reader.layout,Some(position)),|old|(&old.layout,old.places[position]));
        let Some(place)=place else {
            if needs_stored(&column.default) {out.get=Self::default_borrowed;} else {
                out.is_null=matches!(column.default,Value::Null);
                match column.default {Value::I64(v)=>out.constant[..8].copy_from_slice(&v.to_le_bytes()),Value::F64(v)=>out.constant[..8].copy_from_slice(&v.to_le_bytes()),Value::Bool(v)=>out.constant[0]=u8::from(v),Value::Timestamp(v)=>out.constant[..8].copy_from_slice(&((v/1000) as i64).to_le_bytes()),Value::Ulid(v)=>out.constant=v.0,Value::Null=>{},_=>unreachable!("supported default")}
                out.get=Self::constant;
            }return out;
        };
        let slot=layout.slot(place).expect("stored column");
        out.null_byte=slot.null_byte as usize;out.null_mask=slot.null_mask;
        match slot.place {
            super::super::row::SlotPlace::Fixed{offset,width}=>{out.a=offset as usize;out.b=out.a+width as usize;out.get=Self::fixed;out.fast=1;}
            super::super::row::SlotPlace::Variable(j)=>{
                let j=j as usize;
                out.c=layout.offsets_start+(layout.variables-1)*offset_width;
                if layout.variables==1 {out.fast=3;}
                out.a=if j==0 {usize::MAX} else {layout.offsets_start+(j-1)*offset_width};
                out.b=if j+1==layout.variables {usize::MAX} else {layout.offsets_start+j*offset_width};
                out.get=match offset_width {1=>Self::variable::<1>,2=>Self::variable::<2>,4=>Self::variable::<4>,_=>unreachable!()};
            }
            super::super::row::SlotPlace::Key=>unreachable!("handled before layout"),
        } out
    }
    // Only these three classes use the inline path. All offsets and null bits remain runtime data.
    #[inline] fn call<'a>(&self,cx:&ProjectionContext<'a>)->Option<T::Output<'a>> {
        match self.fast {
            1=>Self::fixed(cx,self),
            2=>Self::key(cx,self),
            3=>if self.null(cx.bytes) {None} else {Some(T::bytes(&cx.bytes[self.c..]))},
            _=>(self.get)(cx,self),
        }
    }
    #[inline] fn null(&self,bytes:&[u8])->bool {bytes[self.null_byte]&self.null_mask!=0}
    #[inline] fn fixed<'a>(cx:&ProjectionContext<'a>,p:&Self)->Option<T::Output<'a>> {
        if p.null(cx.bytes) {None} else {Some(T::bytes(&cx.bytes[p.a..p.b]))}
    }
    #[inline] fn variable<'a,const W:usize>(cx:&ProjectionContext<'a>,p:&Self)->Option<T::Output<'a>> {
        if p.null(cx.bytes) {return None;}
        let end=|at:usize| {let b=&cx.bytes[at..at+W];p.c+match W {1=>usize::from(b[0]),2=>u16::from_le_bytes(slot_array(b)) as usize,4=>u32::from_le_bytes(slot_array(b)) as usize,_=>unreachable!()}};
        let start=if p.a==usize::MAX {p.c} else {end(p.a)};
        let stop=if p.b==usize::MAX {cx.bytes.len()} else {end(p.b)};
        Some(T::bytes(&cx.bytes[start..stop]))
    }
    #[inline] fn key<'a>(cx:&ProjectionContext<'a>,p:&Self)->Option<T::Output<'a>> {let start=cx.starts.map_or(0,|s|s[p.a] as usize);Some(T::key(&cx.key[start..]))}
    #[inline] fn owned<'a>(cx:&ProjectionContext<'a>,p:&Self)->Option<T::Output<'a>> {T::value(&cx.owned[p.a])}
    #[inline] fn default_borrowed<'a>(cx:&ProjectionContext<'a>,p:&Self)->Option<T::Output<'a>> {T::value(&cx.defaults.expect("row decoder kept catalog defaults")[p.a])}
    #[inline] fn constant<'a>(_:&ProjectionContext<'a>,p:&Self)->Option<T::Output<'a>> {
        if p.is_null {return None;}
        // Only scalar types select this getter: key decoding produces values without borrowing p.
        // Separate scalar conversion below avoids extending any borrow of cached plan memory.
        T::constant(&p.constant[..p.width])
    }
}

impl Projection {
    pub fn column<T:ProjectionType>(&self,index:usize)->ProjectionResult<Col<T>> {
        let i=self.indices.get(index).ok_or(ProjectionError::Index)?;
        if i.tag!=T::TAG {return Err(ProjectionError::WrongType);}
        Ok(Col{owner:Arc::clone(&self.owner),slot:i.slot,marker:ProjectionPhantom})
    }
    #[inline]
    pub fn read<'p,'r>(&'p self,row:&'r TableRow<'_>)->ProjectionResult<ProjectedRow<'p,'r>> {
        if !Arc::ptr_eq(&self.schema_owner,&row.reader.projection_owner) {return Err(ProjectionError::WrongSchema);}
        let (version,context)=match &row.stored {
            Stored::Owned(values)=>(&self.owned,ProjectionContext{bytes:&[],key:&[],starts:None,defaults:None,owned:values}),
            Stored::Row{key,starts,bytes,defaults,old}=>{
                let plans=match old {
                    None=>&self.current,
                    Some(old)=>self.older.get(usize::from(old.layout.version()).wrapping_sub(1)).ok_or(ProjectionError::InvalidRow)?
                        .get_or_init(||Arc::new(std::array::from_fn(|flag|ProjectionVersion::prepare(&row.reader,&self.positions,Some(old),1<<flag,false)))).as_ref(),
                };
                let version=plans.get(usize::from(bytes[2])).ok_or(ProjectionError::InvalidRow)?;
                (version,ProjectionContext{bytes,key,starts:starts.as_deref(),defaults:defaults.as_deref().map(Vec::as_slice),owned:&[]})
            }
        };
        Ok(ProjectedRow{owner:&self.owner,indices:&self.indices,version,context})
    }
}
impl<'p,'r> ProjectedRow<'p,'r> {
    #[inline] pub fn get<T:ProjectionType>(&self,column:&Col<T>)->ProjectionResult<Option<T::Output<'r>>> {
        if !Arc::ptr_eq(self.owner,&column.owner) {return Err(ProjectionError::WrongProjection);}
        let p=T::plans(self.version).get(column.slot).ok_or(ProjectionError::Index)?;
        Ok(p.call(&self.context))
    }
    #[inline] fn indexed<T:ProjectionType>(&self,index:usize)->ProjectionResult<Option<T::Output<'r>>> {
        let i=self.indices.get(index).ok_or(ProjectionError::Index)?;
        if i.tag!=T::TAG {return Err(ProjectionError::WrongType);}
        let p=T::plans(self.version).get(i.slot).ok_or(ProjectionError::Index)?;
        Ok(p.call(&self.context))
    }
    #[inline] pub fn i64(&self,i:usize)->ProjectionResult<Option<i64>> {self.indexed::<i64>(i)}
    #[inline] pub fn f64(&self,i:usize)->ProjectionResult<Option<f64>> {self.indexed::<f64>(i)}
    #[inline] pub fn bool(&self,i:usize)->ProjectionResult<Option<bool>> {self.indexed::<bool>(i)}
    #[inline] pub fn timestamp(&self,i:usize)->ProjectionResult<Option<i128>> {self.indexed::<Timestamp>(i)}
    #[inline] pub fn ulid(&self,i:usize)->ProjectionResult<Option<Ulid>> {self.indexed::<Ulid>(i)}
    #[inline] pub fn text_bytes(&self,i:usize)->ProjectionResult<Option<&'r [u8]>> {self.indexed::<TextBytes>(i)}
    #[inline] pub fn bytes(&self,i:usize)->ProjectionResult<Option<&'r [u8]>> {self.indexed::<Bytes>(i)}
}
