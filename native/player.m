#include "photosuite.h"
#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#include <string.h>
#import <AVKit/AVKit.h>

@interface PSPlayerView : AVPlayerView
@end
@implementation PSPlayerView
- (BOOL)acceptsFirstResponder { return YES; }
- (void)keyDown:(NSEvent *)event {
    if (event.keyCode == 49) {
        if (event.isARepeat) return;
        if (self.player.rate != 0) { [self.player pause]; }
        else {
            AVPlayerItem *item = self.player.currentItem;
            if (CMTIME_IS_NUMERIC(item.duration) && CMTimeCompare(item.currentTime, item.duration) >= 0) {
                [self.player seekToTime:kCMTimeZero];
            }
            [self.player play];
        }
    } else if (event.keyCode == 53) {
        if (self.window.styleMask & NSWindowStyleMaskFullScreen) { [self.window toggleFullScreen:nil]; }
        else { [self.window performClose:nil]; }
    } else { [super keyDown:event]; }
}
@end

@interface PSPlaybackController : NSWindowController <NSWindowDelegate>
@property(nonatomic,strong) AVPlayer *player;
@property(nonatomic,strong) AVPlayerItem *item;
@property(nonatomic,strong) PSPlayerView *playerView;
@property(nonatomic,copy) NSString *fileTitle;
@property(nonatomic) BOOL observing;
- (void)shutdown;
@end

@implementation PSPlaybackController
- (void)shutdown {
    if (self.observing) {
        [self.item removeObserver:self forKeyPath:@"status"];
        self.observing = NO;
    }
    [self.player pause];
    [self.player replaceCurrentItemWithPlayerItem:nil];
    self.playerView.player = nil;
    self.item = nil;
}
- (void)windowWillClose:(NSNotification *)notification { [self shutdown]; }
- (void)dealloc { [self shutdown]; }
- (void)observeValueForKeyPath:(NSString *)keyPath ofObject:(id)object change:(NSDictionary *)change context:(void *)context {
    if (object != self.item || ![keyPath isEqualToString:@"status"]) { return; }
    __weak PSPlaybackController *weakSelf = self;
    dispatch_async(dispatch_get_main_queue(), ^{
        PSPlaybackController *controller = weakSelf;
        if (!controller || !controller.window.visible || !controller.item) return;
        if (controller.item.status == AVPlayerItemStatusReadyToPlay) {
            controller.window.title = controller.fileTitle;
        } else if (controller.item.status == AVPlayerItemStatusFailed) {
            [controller.player pause];
            controller.window.title = @"Video nicht abspielbar";
            NSAlert *alert = [[NSAlert alloc] init];
            alert.messageText = @"Dieses Video kann nicht abgespielt werden.";
            alert.informativeText = controller.item.error.localizedDescription ?: @"Datei oder Codec wird von macOS nicht unterstützt.";
            [alert addButtonWithTitle:@"OK"];
            [alert beginSheetModalForWindow:controller.window completionHandler:nil];
        }
    });
}
@end

static PSPlaybackController *PSPlayback = nil;
int photosuite_video_play(const char *path) {
    if (![NSThread isMainThread]) return 1;
    @autoreleasepool {
        NSString *name = [[NSFileManager defaultManager] stringWithFileSystemRepresentation:path length:strlen(path)];
        BOOL isDirectory = NO;
        if (![[NSFileManager defaultManager] fileExistsAtPath:name isDirectory:&isDirectory] || isDirectory) return 2;
        if (PSPlayback) { [PSPlayback shutdown]; [PSPlayback close]; }
        NSRect screen = [NSScreen mainScreen].visibleFrame;
        NSRect rect = NSMakeRect(0, 0, MIN(960, screen.size.width - 80), MIN(600, screen.size.height - 100));
        NSWindow *window = [[NSWindow alloc] initWithContentRect:rect
            styleMask:(NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable)
            backing:NSBackingStoreBuffered defer:NO];
        window.releasedWhenClosed = NO;
        window.minSize = NSMakeSize(320, 240);
        window.title = @"Video wird geladen …";
        PSPlaybackController *controller = [[PSPlaybackController alloc] initWithWindow:window];
        PSPlayback = controller;
        controller.fileTitle = name.lastPathComponent;
        controller.item = [AVPlayerItem playerItemWithURL:[NSURL fileURLWithPath:name]];
        controller.player = [AVPlayer playerWithPlayerItem:controller.item];
        controller.playerView = [[PSPlayerView alloc] initWithFrame:rect];
        controller.playerView.player = controller.player;
        controller.playerView.controlsStyle = AVPlayerViewControlsStyleInline;
        controller.playerView.showsFullScreenToggleButton = YES;
        controller.playerView.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        window.contentView = controller.playerView;
        window.delegate = controller;
        [window center];
        [controller showWindow:nil];
        [window makeKeyAndOrderFront:nil];
        [window makeFirstResponder:controller.playerView];
        controller.observing = YES;
        [controller.item addObserver:controller forKeyPath:@"status" options:NSKeyValueObservingOptionInitial|NSKeyValueObservingOptionNew context:NULL];
        [controller.player play];
        return 0;
    }
}

