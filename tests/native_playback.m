#include "../native/player.m"
#include <stdio.h>
static void pump(double seconds) {
    NSDate *end = [NSDate dateWithTimeIntervalSinceNow:seconds];
    while ([end timeIntervalSinceNow] > 0) {
        [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.02]];
    }
}
static NSEvent *key(unsigned short code, NSString *text) {
    return [NSEvent keyEventWithType:NSEventTypeKeyDown location:NSZeroPoint modifierFlags:0 timestamp:0 windowNumber:PSPlayback.window.windowNumber context:nil characters:text charactersIgnoringModifiers:text isARepeat:NO keyCode:code];
}
int main(int argc,const char **argv) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        if (argc != 2 || photosuite_video_play(argv[1]) != 0) return 1;
        NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:12];
        while (CMTimeGetSeconds(PSPlayback.item.currentTime) < 0.1 && [deadline timeIntervalSinceNow] > 0) { pump(0.1); }
        if (PSPlayback.item.status != AVPlayerItemStatusReadyToPlay || CMTimeGetSeconds(PSPlayback.item.currentTime) < 0.1) return 2;
        [PSPlayback.playerView keyDown:key(49,@" ")];
        if (PSPlayback.player.rate != 0) return 3;
        [PSPlayback.playerView keyDown:key(49,@" ")];
        pump(0.1);
        if (PSPlayback.player.rate == 0) return 4;
        [PSPlayback.playerView keyDown:key(53,@"\033")];
        pump(0.1);
        if (PSPlayback.item != nil || PSPlayback.player.rate != 0 || PSPlayback.window.visible) return 5;
        printf("PASS: native playback advanced; Space paused/resumed; Escape closed and released item.\n");
        return 0;
    }
}
