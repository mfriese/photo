// Explicit network smoke test; only a pinned 1477-byte metadata file, never model weights.
#import <Foundation/Foundation.h>
#include <assert.h>
#include <fcntl.h>
#include <unistd.h>
#include "whisper_models.h"
static void progress(void *context,uint64_t bytes){*(uint64_t *)context=bytes;}
static int cancel(void *context){return 0;}
int main(int argc,char **argv){@autoreleasepool {
    assert(argc==3);int fd=open(argv[2],O_WRONLY|O_CREAT|O_TRUNC,0600);assert(fd>=0);uint64_t received=0;char error[1024]={0};
    int status=photosuite_model_download(argv[1],fd,1477,&received,progress,cancel,error,sizeof(error));close(fd);
    if(status){fprintf(stderr,"%s\n",error);return 1;}assert(received==1477);
    fd=open(argv[2],O_RDONLY);assert(fd>=0);char digest[65]={0};
    assert(photosuite_model_sha256(fd,&received,progress,cancel,digest,error,sizeof(error))==0);close(fd);
    assert(strcmp(digest,"db7c0371f46f0840b8f25794c1e3321c9b5820a8cb6ba9694a46fc64b8fae5a6")==0);
    // Wrong expected response size must fail, not silently accept the response.
    fd=open(argv[2],O_WRONLY|O_TRUNC);assert(fd>=0);
    assert(photosuite_model_download(argv[1],fd,1478,&received,progress,cancel,error,sizeof(error))!=0);close(fd);
    puts("Native model HTTPS: pinned metadata transfer, progress, SHA-256 and wrong-size rejection OK; no model weights downloaded.");return 0;
}}
