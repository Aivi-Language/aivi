use std::{fmt, hash::Hasher};

struct DebugHashWriter<'hasher, H> {
    hasher: &'hasher mut H,
}

impl<H: Hasher> fmt::Write for DebugHashWriter<'_, H> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.hasher.write(value.as_bytes());
        Ok(())
    }
}

pub(crate) fn hash_debug<H: Hasher>(value: &(impl fmt::Debug + ?Sized), hasher: &mut H) {
    // Debug-rendered fields share an explicit domain and stream directly into
    // the fingerprint, avoiding a full intermediate `String` for large IRs.
    hasher.write_u8(0xd0);
    fmt::write(&mut DebugHashWriter { hasher }, format_args!("{value:?}"))
        .expect("fingerprint debug writer is infallible");
    hasher.write_u8(0xff);
}
