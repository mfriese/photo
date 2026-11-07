#include "photosuite.h"
#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#include <stdlib.h>
#include <string.h>
// Owned malloc buffer; Rust always releases it with photosuite_free.
int photosuite_video_frame(const char *path, double seconds, double *actualSeconds, unsigned char **bytes, size_t *length) {
    @autoreleasepool {
        *bytes = NULL; *length = 0;
        NSString *name = [[NSFileManager defaultManager] stringWithFileSystemRepresentation:path length:strlen(path)];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:name] options:nil];
        AVAssetImageGenerator *generator = [[AVAssetImageGenerator alloc] initWithAsset:asset];
        generator.appliesPreferredTrackTransform = YES;
        generator.maximumSize = CGSizeMake(640, 640);
        NSError *error = nil;
        generator.requestedTimeToleranceBefore = kCMTimeZero;
        generator.requestedTimeToleranceAfter = kCMTimeZero;
        CMTime actualTime = kCMTimeInvalid;
        CGImageRef image = [generator copyCGImageAtTime:CMTimeMakeWithSeconds(seconds, 60000) actualTime:&actualTime error:&error];
        *actualSeconds = CMTimeGetSeconds(actualTime);
        if (!image) return 1;
        NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithCGImage:image];
        CGImageRelease(image);
        NSData *data = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        if (!data || data.length == 0) return 2;
        *bytes = malloc(data.length);
        if (!*bytes) return 3;
        memcpy(*bytes, data.bytes, data.length); *length = data.length;
        return 0;
    }
}
void photosuite_free(void *bytes) { free(bytes); }

double photosuite_video_duration(const char *path) {
    @autoreleasepool {
        NSString *name = [[NSFileManager defaultManager] stringWithFileSystemRepresentation:path length:strlen(path)];
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:name] options:nil];
        return CMTimeGetSeconds(asset.duration);
    }
}

