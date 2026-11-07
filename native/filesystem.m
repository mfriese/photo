#include "photosuite.h"
#import <Foundation/Foundation.h>
#include <string.h>
// Filesystem adapter: reversible removal only after explicit duplicate review.
int photosuite_trash_file(const char *path) {
    @autoreleasepool {
        NSString *name = [[NSFileManager defaultManager] stringWithFileSystemRepresentation:path length:strlen(path)];
        NSError *error = nil;
        return [[NSFileManager defaultManager] trashItemAtURL:[NSURL fileURLWithPath:name] resultingItemURL:NULL error:&error] ? 0 : 1;
    }
}
