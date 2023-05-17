use std::borrow::Cow;

macro_rules! name {
    ($($tok:ident $str:literal)*) => {
        #[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
        pub enum KnownName {
            $($tok,)*
        }
        impl KnownName {
            pub fn from_str(name: &str) -> Option<KnownName> {
                match name {
                    $($str => Some(KnownName::$tok),)*
                    _ => None,
                }
            }
            pub fn as_str(&self) -> &'static str {
                match *self {
                    $(KnownName::$tok => $str,)*
                }
            }
        }
    };
}

name! {
    // Core object types
    CoreObjectBlobPlain "..:obj.BlobPlain"
    CoreObjectBlobZstd "..:obj.BlobZstd"
    CoreObjectZstdDictionary "..:obj.ZstdDictionary"
    CoreObjectZstdPatchDictionary "..:obj.ZstdPatchDictionary"

    // Directory-related
    CoreObjectMetadata "..:obj.Metadata"
    CoreObjectDirectory "..:obj.Directory"
}

/// A name which may or may not be registered in the string table.
#[derive(Clone, Debug, Hash)]
pub struct Name<'a>(NameData<'a>);
#[derive(Clone, Debug, Hash)]
enum NameData<'a> {
    Known(KnownName),
    Owned(Cow<'a, str>),
}
impl<'a> Name<'a> {
    /// Returns the string underlying this name.
    pub fn as_str(&self) -> &str {
        match &self.0 {
            NameData::Known(name) => name.as_str(),
            NameData::Owned(str) => str.as_ref(),
        }
    }

    pub fn is_known(&self) -> bool {
        match &self.0 {
            NameData::Known(_) => true,
            NameData::Owned(_) => false,
        }
    }

    /// Whether this name is low level. (i.e. starts with `..`)
    pub fn is_low_level(&self) -> bool {
        self.as_str().starts_with("..")
    }
}

impl<'a, 'b> PartialEq<Name<'a>> for Name<'b> {
    fn eq(&self, other: &Name<'a>) -> bool {
        match (self, other) {
            (Name(NameData::Known(a)), Name(NameData::Known(b))) => a == b,
            (a, b) => a.as_str() == b.as_str(),
        }
    }
}
impl<'a> Eq for Name<'a> {}

impl<'a> From<KnownName> for Name<'a> {
    fn from(name: KnownName) -> Self {
        Name(NameData::Known(name))
    }
}
impl<'a> From<&'a str> for Name<'a> {
    fn from(name: &'a str) -> Self {
        match KnownName::from_str(name) {
            Some(v) => v.into(),
            None => Name(NameData::Owned(name.into())),
        }
    }
}
impl<'a> From<String> for Name<'a> {
    fn from(name: String) -> Self {
        match KnownName::from_str(&name) {
            Some(v) => v.into(),
            None => Name(NameData::Owned(name.into())),
        }
    }
}
impl<'a> From<Cow<'a, str>> for Name<'a> {
    fn from(name: Cow<'a, str>) -> Self {
        match KnownName::from_str(&name) {
            Some(v) => v.into(),
            None => Name(NameData::Owned(name)),
        }
    }
}

pub type StaticName = Name<'static>;
