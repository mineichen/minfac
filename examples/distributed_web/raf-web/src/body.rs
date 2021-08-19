use crate::Cow;
use bytes::Bytes;
/// Most of these types are copied from Hyper::Body.
/// The reason is, that plugins don't bloat by including hyper+tokio
use core::pin::Pin;
use futures::stream::Stream;

#[must_use = "streams do nothing unless polled"]
pub struct Body {
    pub(crate) kind: Kind,
}

impl Body {
    #[inline]
    pub fn empty() -> Body {
        Body::new(Kind::Once(None))
    }

    fn new(kind: Kind) -> Body {
        Body { kind }
    }
}

pub(crate) enum Kind {
    Once(Option<Bytes>),
    Wrapped(Box<dyn Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send>),
}

impl From<Box<dyn Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send>>
    for Body
{
    #[inline]
    fn from(
        stream: Box<
            dyn Stream<Item = Result<Bytes, Box<dyn std::error::Error + Send + Sync>>> + Send,
        >,
    ) -> Body {
        Body::new(Kind::Wrapped(stream.into()))
    }
}

impl From<Bytes> for Body {
    #[inline]
    fn from(chunk: Bytes) -> Body {
        if chunk.is_empty() {
            Body::empty()
        } else {
            Body::new(Kind::Once(Some(chunk)))
        }
    }
}

impl From<Vec<u8>> for Body {
    #[inline]
    fn from(vec: Vec<u8>) -> Body {
        Body::from(Bytes::from(vec))
    }
}

impl From<&'static [u8]> for Body {
    #[inline]
    fn from(slice: &'static [u8]) -> Body {
        Body::from(Bytes::from(slice))
    }
}

impl From<Cow<'static, [u8]>> for Body {
    #[inline]
    fn from(cow: Cow<'static, [u8]>) -> Body {
        match cow {
            Cow::Borrowed(b) => Body::from(b),
            Cow::Owned(o) => Body::from(o),
        }
    }
}

impl From<String> for Body {
    #[inline]
    fn from(s: String) -> Body {
        Body::from(Bytes::from(s.into_bytes()))
    }
}

impl From<&'static str> for Body {
    #[inline]
    fn from(slice: &'static str) -> Body {
        Body::from(Bytes::from(slice.as_bytes()))
    }
}

impl From<Cow<'static, str>> for Body {
    #[inline]
    fn from(cow: Cow<'static, str>) -> Body {
        match cow {
            Cow::Borrowed(b) => Body::from(b),
            Cow::Owned(o) => Body::from(o),
        }
    }
}
