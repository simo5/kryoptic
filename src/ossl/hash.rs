#[cfg(feature = "fips")]
use {super::fips, fips::*};

#[cfg(not(feature = "fips"))]
use {super::ossl, ossl::*};

use std::os::raw::*;

#[derive(Debug)]
pub struct HashState {
    md: EvpMd,
    ctx: EvpMdCtx,
}

impl HashState {
    pub fn new(alg: &[u8]) -> KResult<HashState> {
        unsafe {
            let libctx = get_libctx();
            Ok(HashState {
                md: EvpMd::from_ptr(EVP_MD_fetch(
                    libctx,
                    alg.as_ptr() as *const c_char,
                    std::ptr::null_mut(),
                ))?,
                ctx: EvpMdCtx::from_ptr(EVP_MD_CTX_new())?,
            })
        }
    }
}

unsafe impl Send for HashState {}
unsafe impl Sync for HashState {}

impl HashOperation {
    pub fn new(mech: CK_MECHANISM_TYPE) -> KResult<HashOperation> {
        let alg: &[u8] = match mech {
            CKM_SHA_1 => OSSL_DIGEST_NAME_SHA1,
            CKM_SHA224 => OSSL_DIGEST_NAME_SHA2_224,
            CKM_SHA256 => OSSL_DIGEST_NAME_SHA2_256,
            CKM_SHA384 => OSSL_DIGEST_NAME_SHA2_384,
            CKM_SHA512 => OSSL_DIGEST_NAME_SHA2_512,
            CKM_SHA3_224 => OSSL_DIGEST_NAME_SHA3_224,
            CKM_SHA3_256 => OSSL_DIGEST_NAME_SHA3_256,
            CKM_SHA3_384 => OSSL_DIGEST_NAME_SHA3_384,
            CKM_SHA3_512 => OSSL_DIGEST_NAME_SHA3_512,
            _ => return err_rv!(CKR_MECHANISM_INVALID),
        };
        Ok(HashOperation {
            mech: mech,
            state: HashState::new(alg)?,
            finalized: false,
            in_use: false,
        })
    }
    pub fn hashlen(&self) -> usize {
        unsafe { EVP_MD_get_size(self.state.md.as_ptr()) as usize }
    }
    pub fn blocklen(&self) -> usize {
        unsafe { EVP_MD_get_block_size(self.state.md.as_ptr()) as usize }
    }

    fn digest_init(&mut self) -> KResult<()> {
        unsafe {
            match EVP_DigestInit(
                self.state.ctx.as_mut_ptr(),
                self.state.md.as_ptr(),
            ) {
                1 => Ok(()),
                _ => err_rv!(CKR_DEVICE_ERROR),
            }
        }
    }
}

impl MechOperation for HashOperation {
    fn mechanism(&self) -> CK_MECHANISM_TYPE {
        self.mech
    }
    fn in_use(&self) -> bool {
        self.in_use
    }
    fn finalized(&self) -> bool {
        self.finalized
    }
    fn reset(&mut self) -> KResult<()> {
        self.finalized = false;
        self.in_use = false;
        Ok(())
    }
}

impl Digest for HashOperation {
    fn digest(&mut self, data: &[u8], digest: &mut [u8]) -> KResult<()> {
        if self.in_use || self.finalized {
            return err_rv!(CKR_OPERATION_NOT_INITIALIZED);
        }
        if digest.len() != self.digest_len()? {
            return err_rv!(CKR_GENERAL_ERROR);
        }
        self.finalized = true;
        /* NOTE: It is ok if data and digest point to the same buffer*/
        let mut digest_len = self.digest_len()? as c_uint;
        let r = unsafe {
            EVP_Digest(
                data.as_ptr() as *const c_void,
                data.len(),
                digest.as_mut_ptr(),
                &mut digest_len,
                self.state.md.as_ptr(),
                std::ptr::null_mut(),
            )
        };
        if r != 1 || digest_len as usize != digest.len() {
            return err_rv!(CKR_GENERAL_ERROR);
        }
        Ok(())
    }

    fn digest_update(&mut self, data: &[u8]) -> KResult<()> {
        if self.finalized {
            return err_rv!(CKR_OPERATION_NOT_INITIALIZED);
        }
        if !self.in_use {
            self.digest_init()?;
            self.in_use = true;
        }
        let r = unsafe {
            EVP_DigestUpdate(
                self.state.ctx.as_mut_ptr(),
                data.as_ptr() as *const c_void,
                data.len(),
            )
        };
        match r {
            1 => Ok(()),
            _ => {
                self.finalized = true;
                err_rv!(CKR_DEVICE_ERROR)
            }
        }
    }

    fn digest_final(&mut self, digest: &mut [u8]) -> KResult<()> {
        if !self.in_use {
            return err_rv!(CKR_OPERATION_NOT_INITIALIZED);
        }
        if self.finalized {
            return err_rv!(CKR_OPERATION_NOT_INITIALIZED);
        }
        if digest.len() != self.digest_len()? {
            return err_rv!(CKR_GENERAL_ERROR);
        }
        self.finalized = true;
        let mut digest_len = self.digest_len()? as c_uint;
        let r = unsafe {
            EVP_DigestFinal_ex(
                self.state.ctx.as_mut_ptr(),
                digest.as_mut_ptr(),
                &mut digest_len,
            )
        };
        if r != 1 || digest_len as usize != digest.len() {
            return err_rv!(CKR_GENERAL_ERROR);
        }
        Ok(())
    }

    fn digest_len(&self) -> KResult<usize> {
        Ok(self.hashlen())
    }
}
