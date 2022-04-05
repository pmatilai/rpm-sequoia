// This is a reimplementation of rpm/rpmio/digest_openssl.c /
// rpm/rpmio/digest_libgcrypt.c using Sequoia.

use libc::{
    c_int,
    size_t,
};

use sequoia_openpgp as openpgp;
use openpgp::types::HashAlgorithm;
use openpgp::crypto::hash::Digest;

use crate::Error;
use crate::Result;

#[derive(Clone)]
pub struct DigestContext {
    pub(crate) ctx: Box<dyn Digest>,
}

impl DigestContext {
    pub fn digest_size(&self) -> usize {
        self.ctx.digest_size()
    }

    pub fn update<T: AsRef<[u8]>>(&mut self, data: T) {
        self.ctx.update(data.as_ref());
    }

    pub fn digest(&mut self, digest: &mut [u8]) -> Result<()> {
        Ok(self.ctx.digest(digest)?)
    }

    pub fn into_digest(self) -> Result<Vec<u8>> {
        Ok(self.ctx.into_digest()?)
    }
}

// int rpmInitCrypto(void)
ffi!(fn rpmInitCrypto() -> Binary {
    Ok(())
});

// int rpmFreeCrypto(void)
ffi!(fn rpmFreeCrypto() -> Binary {
    Ok(())
});

// DIGEST_CTX rpmDigestInit(int hashalgo, rpmDigestFlags flags)
//
// rpmDigestFlags currently does not define any flags.
ffi!(fn rpmDigestInit(hashalgo: c_int, flags: c_int) -> *mut DigestContext {
    if hashalgo < 0 || hashalgo > u8::MAX as c_int {
        return Err(Error::Fail("Out of range".into()));
    }
    let hashalgo = HashAlgorithm::from(hashalgo as u8);

    if flags != 0 {
        return Err(Error::Fail(format!("Unsupported flags: {}", flags)));
    }

    let ctx = DigestContext {
        ctx: hashalgo.context()?,
    };

    Ok(move_to_c!(ctx))
});

// DIGEST_CTX rpmDigestDup(DIGEST_CTX octx)
ffi!(fn rpmDigestDup(ctx: *const DigestContext) -> *mut DigestContext {
    let ctx = check_ptr!(ctx);
    Ok(Box::into_raw(Box::new(ctx.clone())))
});

// size_t rpmDigestLength(int hashalgo)
ffi!(fn rpmDigestLength(hashalgo: c_int) -> size_t[0] {
    if hashalgo < 0 || hashalgo > u8::MAX as c_int {
        return Ok(0);
    }
    let hashalgo = HashAlgorithm::from(hashalgo as u8);

    use HashAlgorithm::*;
    let len = match hashalgo {
        MD5 => 16,
        SHA1 => 20,
        RipeMD => 20,
        SHA256 => 32,
        SHA384 => 48,
        SHA512 => 64,
        SHA224 => 28,
        _ => 0,
    };

    Ok(len)
});

// int rpmDigestUpdate(DIGEST_CTX ctx, const void * data, size_t len)
ffi!(fn rpmDigestUpdate(ctx: *mut DigestContext,
                        data: *const u8, len: size_t) -> ErrorCode {
    let ctx = check_mut!(ctx);
    let data = check_slice!(data, len);

    ctx.update(data);

    Ok(())
});

// int rpmDigestFinal(DIGEST_CTX ctx, void ** datap, size_t *lenp, int asAscii)
ffi!(fn rpmDigestFinal(ctx: *mut DigestContext,
                       datap: *mut *mut u8, lenp: *mut size_t,
                       as_ascii: c_int) -> Binary {
    if ctx.is_null() {
        return Ok(());
    }

    let ctx = claim_from_c!(ctx);
    let datap = check_optional_mut!(datap);
    let lenp = check_optional_mut!(lenp);

    let mut digest = ctx.into_digest()?;

    if as_ascii != 0 {
        digest = digest
            .iter()
            .map(|x| {
                let x = format!("{:02x}", x);
                let x = x.as_bytes();
                std::iter::once(x[0]).chain(std::iter::once(x[1]))
            })
            .flatten()
            // Add a NUL.
            .chain(std::iter::once(0))
            .collect();
    }

    digest.shrink_to_fit();
    if let Some(lenp) = lenp {
        *lenp = digest.len() as size_t;
    }
    if let Some(datap) = datap {
        *datap = digest.as_mut_ptr();
        // Pass ownership to the caller.
        std::mem::forget(digest);
    }

    Ok(())
});
