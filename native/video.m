#include "photosuite.h"
#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
// Owned malloc buffer; Rust always releases it with photosuite_free.
int photosuite_video_frame(const char *path, double seconds, double *actualSeconds, unsigned char **bytes, size_t *length, char *errorMessage, size_t errorCapacity) {
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
        if (!image) {
            snprintf(errorMessage,errorCapacity,"%s",error.localizedDescription.UTF8String ?: "AVFoundation konnte keinen Frame decodieren");
            return 1;
        }
        NSBitmapImageRep *bitmap = [[NSBitmapImageRep alloc] initWithCGImage:image];
        CGImageRelease(image);
        NSData *data = [bitmap representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        if (!data || data.length == 0) {
            snprintf(errorMessage,errorCapacity,"Videoframe konnte nicht in PNG umgewandelt werden");
            return 2;
        }
        *bytes = malloc(data.length);
        if (!*bytes) {
            snprintf(errorMessage,errorCapacity,"Nicht genug Speicher für Videoframe");
            return 3;
        }
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

