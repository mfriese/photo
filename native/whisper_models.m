#import <Foundation/Foundation.h>
#import <CommonCrypto/CommonDigest.h>
#include <unistd.h>
#include <errno.h>
#include "whisper_models.h"
@interface PSModelDownload:NSObject<NSURLSessionDataDelegate>
@property int fd;
@property uint64_t expected;
@property uint64_t limit;
@property uint64_t received;
@property void *context;
@property void (*progress)(void *,uint64_t);
@property dispatch_semaphore_t done;
@property NSString *failure;
@end
@implementation PSModelDownload
- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task didReceiveResponse:(NSURLResponse *)response completionHandler:(void (^)(NSURLSessionResponseDisposition))completion {
    NSInteger status=[response isKindOfClass:NSHTTPURLResponse.class]?((NSHTTPURLResponse *)response).statusCode:0;
    if(status!=200){self.failure=[NSString stringWithFormat:@"HTTP %ld beim Download",(long)status];completion(NSURLSessionResponseCancel);return;}
    if(response.expectedContentLength>0 && ((self.expected && (uint64_t)response.expectedContentLength!=self.expected) || (uint64_t)response.expectedContentLength>self.limit)){self.failure=@"Server meldet eine unerwartete Dateigröße";completion(NSURLSessionResponseCancel);return;}
    completion(NSURLSessionResponseAllow);
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task willPerformHTTPRedirection:(NSHTTPURLResponse *)response newRequest:(NSURLRequest *)request completionHandler:(void (^)(NSURLRequest *))completion {
    if(![request.URL.scheme.lowercaseString isEqual:@"https"]){self.failure=@"Unsichere Downloadweiterleitung abgelehnt";completion(nil);[task cancel];return;}completion(request);
}
- (void)URLSession:(NSURLSession *)session dataTask:(NSURLSessionDataTask *)task didReceiveData:(NSData *)data {
    if(self.failure)return;
    if((uint64_t)data.length>self.limit-self.received){self.failure=@"Download überschreitet das Größenlimit";[task cancel];return;}
    const uint8_t *bytes=data.bytes;size_t remaining=data.length;
    while(remaining){ssize_t n=write(self.fd,bytes,remaining);if(n<0&&errno==EINTR)continue;if(n<=0){self.failure=[NSString stringWithFormat:@"Download konnte nicht geschrieben werden: %s",strerror(errno)];[task cancel];return;}bytes+=n;remaining-=n;}
    self.received+=data.length;self.progress(self.context,self.received);
}
- (void)URLSession:(NSURLSession *)session task:(NSURLSessionTask *)task didCompleteWithError:(NSError *)error {
    if(error&&!self.failure)self.failure=error.localizedDescription;
    if(!self.failure&&self.expected&&self.received!=self.expected)self.failure=@"Unvollständiger Download";
    dispatch_semaphore_signal(self.done);
}
@end
static int report(NSString *text,char *error,size_t capacity){if(error&&capacity)snprintf(error,capacity,"%s",text.UTF8String?:"Modellfehler");return 1;}
int photosuite_asset_download(const char *url,int fd,uint64_t expected,uint64_t limit,void *context,void (*progress)(void *,uint64_t),int (*cancel)(void *),char *error,size_t capacity){@autoreleasepool {
    if(cancel(context))return report(@"Abgebrochen",error,capacity);
    NSURL *address=[NSURL URLWithString:[NSString stringWithUTF8String:url]];
    if(!address||![address.scheme.lowercaseString isEqual:@"https"])return report(@"Nur HTTPS-Quellen sind erlaubt",error,capacity);
    PSModelDownload *state=[PSModelDownload new];state.fd=fd;state.expected=expected;state.limit=limit;state.context=context;state.progress=progress;state.done=dispatch_semaphore_create(0);
    NSURLSessionConfiguration *config=NSURLSessionConfiguration.ephemeralSessionConfiguration;config.URLCache=nil;config.requestCachePolicy=NSURLRequestReloadIgnoringLocalCacheData;config.timeoutIntervalForRequest=30;config.timeoutIntervalForResource=7200;config.waitsForConnectivity=NO;
    NSOperationQueue *queue=[NSOperationQueue new];queue.maxConcurrentOperationCount=1;
    NSURLSession *session=[NSURLSession sessionWithConfiguration:config delegate:state delegateQueue:queue];
    NSMutableURLRequest *request=[NSMutableURLRequest requestWithURL:address];[request setValue:@"identity" forHTTPHeaderField:@"Accept-Encoding"];
    NSURLSessionDataTask *task=[session dataTaskWithRequest:request];[task resume];
    BOOL cancelled=NO;
    while(dispatch_semaphore_wait(state.done,dispatch_time(DISPATCH_TIME_NOW,100*NSEC_PER_MSEC))!=0){if(cancel(context)&&!cancelled){cancelled=YES;[task cancel];}}
    [session finishTasksAndInvalidate];
    if(cancelled||cancel(context))return report(@"Abgebrochen",error,capacity);
    if(state.failure)return report(state.failure,error,capacity);
    return 0;
}}
int photosuite_model_download(const char *url,int fd,uint64_t expected,void *context,void (*progress)(void *,uint64_t),int (*cancel)(void *),char *error,size_t capacity){
    return photosuite_asset_download(url,fd,expected,expected,context,progress,cancel,error,capacity);
}
int photosuite_model_sha256(int fd,void *context,void (*progress)(void *,uint64_t),int (*cancel)(void *),char *digest,char *error,size_t capacity){@autoreleasepool {
    CC_SHA256_CTX hash;CC_SHA256_Init(&hash);uint8_t buffer[65536];uint64_t total=0;
    while(1){if(cancel(context))return report(@"Abgebrochen",error,capacity);ssize_t n=read(fd,buffer,sizeof(buffer));if(n<0&&errno==EINTR)continue;if(n<0)return report(@"Modell konnte nicht gelesen werden",error,capacity);if(!n)break;CC_SHA256_Update(&hash,buffer,(CC_LONG)n);total+=n;progress(context,total);}
    uint8_t bytes[CC_SHA256_DIGEST_LENGTH];CC_SHA256_Final(bytes,&hash);for(size_t i=0;i<sizeof(bytes);i++)snprintf(digest+i*2,3,"%02x",bytes[i]);return 0;
}}
