// Automated integration: a real AVFoundation composition, seek across files and cleanup.
#import "../native/audio.m"
#include <assert.h>
static void pump(double seconds) {
    NSDate *end=[NSDate dateWithTimeIntervalSinceNow:seconds];
    while ([end timeIntervalSinceNow]>0) [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
}
int main(int argc, char **argv) {
    assert(argc==3);
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSArray *paths=@[PSAudioPath(argv[1]),PSAudioPath(argv[2])];
        NSError *error=nil;
        AVMutableComposition *composition=PSBuildAudioComposition(paths,@[@1,@3],^BOOL{return NO;},&error);
        assert(composition && !error);
        assert(fabs(CMTimeGetSeconds(composition.duration)-4)<0.01);
        AVPlayer *player=[AVPlayer playerWithPlayerItem:[AVPlayerItem playerItemWithAsset:composition]];
        player.muted=YES;
        for (int i=0;i<500 && player.currentItem.status==AVPlayerItemStatusUnknown;++i) pump(0.01);
        assert(player.currentItem.status==AVPlayerItemStatusReadyToPlay);
        PSAudioController *controller=[PSAudioController new];
        controller.player=player;
        controller.timeline=[[PSAudioTimeline alloc] initWithFrame:NSMakeRect(0,0,600,60)];
        controller.timeline.total=4;
        controller.durations=@[@1,@3];controller.paths=paths;controller.labels=@[@"source",@"source"];
        controller.titleLabel=[NSTextField labelWithString:@""];controller.detailLabel=[NSTextField labelWithString:@""];
        NSDateFormatter *date=[NSDateFormatter new];date.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];date.timeZone=[NSTimeZone timeZoneForSecondsFromGMT:0];date.dateFormat=@"yyyyMMdd_HHmmss";
        controller.recordedAt=@[@([[date dateFromString:@"20231227_225132"] timeIntervalSince1970]),@([[date dateFromString:@"20231227_235959"] timeIntervalSince1970])];
        controller.initialPosition=2.25; controller.needsInitialSeek=YES;
        [controller tick:nil];
        assert(!controller.needsInitialSeek);
        for (int i=0;i<500 && controller.seeking;++i) pump(0.01);
        assert(!controller.seeking);
        assert(fabs(CMTimeGetSeconds(player.currentTime)-2.25)<0.03);
        [controller tick:nil];
        assert([controller.titleLabel.stringValue isEqualToString:@"28.12.2023"]);
        assert([controller.detailLabel.stringValue isEqualToString:@"00:00:00"]);
        [player play]; pump(0.25);
        assert(CMTimeGetSeconds(player.currentTime)>2.30);
        [player pause]; double paused=CMTimeGetSeconds(player.currentTime); pump(0.1);
        assert(fabs(CMTimeGetSeconds(player.currentTime)-paused)<0.03);
        [controller shutdown];
        assert(!controller.player && !player.currentItem && controller.cancelled);
        assert(PSBuildAudioComposition(paths,@[@1,@3],^BOOL{return YES;},&error)==nil);
        puts("Native audio: 1:3 composition, cross-file seek, playback, pause, cancellation and cleanup OK");
    }
    return 0;
}
