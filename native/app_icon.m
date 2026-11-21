#import <AppKit/AppKit.h>
#include <stddef.h>
#include <stdint.h>

// eframe's window icon alone does not set the macOS application/Dock icon.
int photosuite_set_application_icon(const uint8_t *bytes, size_t length) {
    if (![NSThread isMainThread] || !bytes || !length) return 0;
    NSData *data = [NSData dataWithBytes:bytes length:length];
    NSImage *image = [[NSImage alloc] initWithData:data];
    if (!image || !image.isValid) return 0;
    [NSApplication sharedApplication].applicationIconImage = image;
    return 1;
}
