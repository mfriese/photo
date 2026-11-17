// Blocking worker-thread operations. fd/context/callbacks are borrowed until return.
// Progress/cancellation callbacks must not throw or call UI APIs. HTTPS only; no credentials.
#include <stdint.h>
#include <stddef.h>
int photosuite_model_download(const char *url,int fd,uint64_t expected,void *context,void (*progress)(void *,uint64_t),int (*cancel)(void *),char *error,size_t capacity);
int photosuite_model_sha256(int fd,void *context,void (*progress)(void *,uint64_t),int (*cancel)(void *),char *digest,char *error,size_t capacity);

int photosuite_asset_download(const char *url,int fd,uint64_t expected,uint64_t limit,void *context,void (*progress)(void *,uint64_t),int (*cancel)(void *),char *error,size_t capacity);
