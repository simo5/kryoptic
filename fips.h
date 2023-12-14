#define OSSL_CRYPTO_ALLOC __attribute__((__malloc__))
#define FIPS_MODULE 1

#include "openssl/core_dispatch.h"
#include "openssl/core_names.h"
#include "openssl/params.h"
#include "openssl/fips_names.h"
#include "internal/provider.h"
#include "internal/property.h"

int OSSL_provider_init_int(const OSSL_CORE_HANDLE *handle,
                           const OSSL_DISPATCH *in,
                           const OSSL_DISPATCH **out,
                           void **provctx);