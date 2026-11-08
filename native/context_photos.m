#include "photosuite.h"
#import <Foundation/Foundation.h>
#import <ImageIO/ImageIO.h>
#include <stdlib.h>
#include <string.h>
int photosuite_context_thumbnail(const char *path, unsigned int edge, unsigned char **bytes, size_t *length) {
    @autoreleasepool {
        *bytes=NULL;*length=0;
        if (edge<16 || edge>512) return 1;
        NSString *name=[[NSFileManager defaultManager] stringWithFileSystemRepresentation:path length:strlen(path)];
        CGImageSourceRef source=CGImageSourceCreateWithURL((__bridge CFURLRef)[NSURL fileURLWithPath:name],(__bridge CFDictionaryRef)@{(__bridge NSString *)kCGImageSourceShouldCache:@NO});
        if (!source) return 2;
        NSDictionary *options=@{(__bridge NSString *)kCGImageSourceCreateThumbnailFromImageAlways:@YES,(__bridge NSString *)kCGImageSourceCreateThumbnailWithTransform:@YES,(__bridge NSString *)kCGImageSourceThumbnailMaxPixelSize:@(edge)};
        CGImageRef thumbnail=CGImageSourceCreateThumbnailAtIndex(source,0,(__bridge CFDictionaryRef)options);
        CFRelease(source);
        if (!thumbnail) return 3;
        NSMutableData *data=[NSMutableData data];
        CGImageDestinationRef destination=CGImageDestinationCreateWithData((__bridge CFMutableDataRef)data,CFSTR("public.png"),1,NULL);
        if (!destination){CGImageRelease(thumbnail);return 4;}
        CGImageDestinationAddImage(destination,thumbnail,NULL);
        BOOL ready=CGImageDestinationFinalize(destination);
        CFRelease(destination);CGImageRelease(thumbnail);
        if (!ready || !data.length || data.length>8*1024*1024) return 5;
        *bytes=malloc(data.length);if (!*bytes) return 6;
        memcpy(*bytes,data.bytes,data.length);*length=data.length;return 0;
    }
}
