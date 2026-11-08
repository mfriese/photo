#include "photosuite.h"
#import <AVFoundation/AVFoundation.h>
#import <AppKit/AppKit.h>
#include <math.h>
#include <string.h>

static NSString *PSAudioPath(const char *path) {
    return [[NSFileManager defaultManager] stringWithFileSystemRepresentation:path length:strlen(path)];
}
int photosuite_audio_probe(const char *path, double *duration, int *audioOnly, char *created, size_t capacity) {
    @autoreleasepool {
        if (capacity) created[0] = 0;
        NSString *name = PSAudioPath(path);
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:name] options:@{AVURLAssetPreferPreciseDurationAndTimingKey:@YES}];
        *duration = CMTimeGetSeconds(asset.duration);
        *audioOnly = [asset tracksWithMediaType:AVMediaTypeAudio].count > 0 && [asset tracksWithMediaType:AVMediaTypeVideo].count == 0;
        if (!asset.playable || !isfinite(*duration) || *duration <= 0) return 1;
        NSDate *date = [[NSFileManager defaultManager] attributesOfItemAtPath:name error:NULL][NSFileCreationDate];
        if (date && capacity) {
            NSISO8601DateFormatter *formatter = [NSISO8601DateFormatter new];
            formatter.timeZone = [NSTimeZone localTimeZone];
            const char *text = [formatter stringFromDate:date].UTF8String;
            strlcpy(created, text, capacity);
        }
        return 0;
    }
}

// No output file is created. AVFoundation reads the original audio segments directly.
static AVMutableComposition *PSBuildAudioComposition(NSArray<NSString *> *paths, NSArray<NSNumber *> *durations, BOOL (^cancelled)(void), NSError **error) {
    AVMutableComposition *composition = [AVMutableComposition composition];
    CMTime position = kCMTimeZero;
    for (NSUInteger i = 0; i < paths.count; ++i) {
        if (cancelled()) return nil;
        AVURLAsset *asset = [AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:paths[i]] options:@{AVURLAssetPreferPreciseDurationAndTimingKey:@YES}];
        double length = CMTimeGetSeconds(asset.duration);
        if (!asset.playable || [asset tracksWithMediaType:AVMediaTypeAudio].count == 0 || [asset tracksWithMediaType:AVMediaTypeVideo].count != 0 || !isfinite(length) || fabs(length-durations[i].doubleValue) > 0.000001) {
            if (error) *error = [NSError errorWithDomain:@"PhotoSuite" code:1 userInfo:@{NSLocalizedDescriptionKey:[NSString stringWithFormat:@"Audio nicht abspielbar oder seit dem Einlesen geändert: %@", paths[i].lastPathComponent]}];
            return nil;
        }
        if (![composition insertTimeRange:CMTimeRangeMake(kCMTimeZero, asset.duration) ofAsset:asset atTime:position error:error]) return nil;
        position = CMTimeAdd(position, asset.duration);
    }
    return composition;
}

@class PSAudioController;
@interface PSAudioTimeline : NSView
@property(nonatomic,weak) PSAudioController *controller;
@property(nonatomic,copy) NSArray<NSNumber *> *durations;
@property(nonatomic) double total;
@property(nonatomic) double position;
@property(nonatomic,strong) NSTrackingArea *tracking;
@end
@interface PSAudioController : NSWindowController <NSWindowDelegate>
@property(nonatomic,strong) AVPlayer *player;
@property(nonatomic,strong) NSTimer *timer;
@property(nonatomic,strong) PSAudioTimeline *timeline;
@property(nonatomic,strong) NSButton *playButton;
@property(nonatomic,strong) NSTextField *titleLabel;
@property(nonatomic,strong) NSTextField *detailLabel;
@property(nonatomic,copy) NSArray<NSNumber *> *recordedAt;
@property(nonatomic,copy) NSArray<NSString *> *paths;
@property(nonatomic,copy) NSArray<NSString *> *labels;
@property(nonatomic,copy) NSArray<NSNumber *> *durations;
@property(atomic) BOOL cancelled;
@property(nonatomic) BOOL resumeAfterSeek;
@property(nonatomic) BOOL seeking;
@property(nonatomic) BOOL needsInitialSeek;
@property(nonatomic) double initialPosition;
@property(nonatomic) float volumeLevel;
@property(nonatomic) NSUInteger seekGeneration;
- (void)seek:(double)seconds;
- (void)shutdown;
@end
static PSAudioController *PSAudio = nil;
static NSString *PSAudioDateText(double civil) {
    static NSDateFormatter *formatter; static dispatch_once_t once;
    dispatch_once(&once, ^{formatter=[NSDateFormatter new];formatter.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];formatter.timeZone=[NSTimeZone timeZoneForSecondsFromGMT:0];formatter.dateFormat=@"dd.MM.yyyy";});
    return [formatter stringFromDate:[NSDate dateWithTimeIntervalSince1970:civil]];
}
static NSString *PSAudioClockText(double civil) {
    static NSDateFormatter *formatter; static dispatch_once_t once;
    dispatch_once(&once, ^{formatter=[NSDateFormatter new];formatter.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];formatter.timeZone=[NSTimeZone timeZoneForSecondsFromGMT:0];formatter.dateFormat=@"HH:mm:ss";});
    return [formatter stringFromDate:[NSDate dateWithTimeIntervalSince1970:civil]];
}
@implementation PSAudioTimeline
- (BOOL)acceptsFirstResponder { return YES; }
- (BOOL)isFlipped { return YES; }
- (void)drawRect:(NSRect)dirty {
    CGFloat width = MAX(1,self.bounds.size.width-16);
    CGFloat left = 8;
    double offset = 0;
    for (NSUInteger i=0;i<self.durations.count;++i) {
        double duration = self.durations[i].doubleValue;
        NSRect segment = NSMakeRect(left+width*offset/self.total,15,width*duration/self.total,28);
        [[NSColor colorWithSRGBRed:(i%2 ? 0.36 : 0.23) green:(i%2 ? 0.55 : 0.40) blue:(i%2 ? 0.79 : 0.65) alpha:1] setFill];
        NSRectFill(segment);
        [[NSColor colorWithWhite:0.92 alpha:1] setFill];
        NSRectFill(NSMakeRect(segment.origin.x,15,1,28));
        offset += duration;
    }
    CGFloat x = left+width*MIN(1,MAX(0,self.position/MAX(0.001,self.total)));
    [[NSColor whiteColor] setFill];
    NSRectFill(NSMakeRect(x-1,8,2,42));
    [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(x-5,4,10,10)] fill];
}
- (void)updateTrackingAreas {
    [super updateTrackingAreas];
    if (self.tracking) [self removeTrackingArea:self.tracking];
    self.tracking=[[NSTrackingArea alloc] initWithRect:NSZeroRect options:NSTrackingMouseMoved|NSTrackingActiveInKeyWindow|NSTrackingInVisibleRect owner:self userInfo:nil];
    [self addTrackingArea:self.tracking];
}
- (void)mouseMoved:(NSEvent *)event {
    NSPoint p=[self convertPoint:event.locationInWindow fromView:nil];
    double position=self.total*MIN(1,MAX(0,(p.x-8)/MAX(1,self.bounds.size.width-16)));
    double offset=0;
    for (NSUInteger i=0;i<self.durations.count;++i) {
        if (position<offset+self.durations[i].doubleValue || i+1==self.durations.count) {
            double civil=self.controller.recordedAt[i].doubleValue;
            self.toolTip=[NSString stringWithFormat:@"%@ %@",PSAudioDateText(civil),PSAudioClockText(civil)];
            break;
        }
        offset+=self.durations[i].doubleValue;
    }
}
- (void)mouseDown:(NSEvent *)event { [self mouseDragged:event]; }
- (void)mouseDragged:(NSEvent *)event {
    if (!self.controller.player) return;
    NSPoint p = [self convertPoint:event.locationInWindow fromView:nil];
    [self.controller seek:self.total*MIN(1,MAX(0,(p.x-8)/MAX(1,self.bounds.size.width-16)))];
}
- (void)keyDown:(NSEvent *)event {
    if (event.keyCode==49 && !event.isARepeat) [self.controller performSelector:@selector(toggle:) withObject:nil];
    else if (event.keyCode==123) [self.controller seek:self.position-5];
    else if (event.keyCode==124) [self.controller seek:self.position+5];
    else if (event.keyCode==53) [self.window performClose:nil];
    else [super keyDown:event];
}
@end
@implementation PSAudioController
- (void)shutdown {
    self.cancelled=YES; self.seekGeneration++;
    [self.timer invalidate]; self.timer=nil;
    [self.player pause]; [self.player replaceCurrentItemWithPlayerItem:nil]; self.player=nil;
}
- (void)windowWillClose:(NSNotification *)notification { [self shutdown]; }
- (void)dealloc { [self shutdown]; }
- (void)tick:(NSTimer *)timer {
    if (!self.player) return;
    if (self.player.currentItem.status==AVPlayerItemStatusFailed) {
        self.detailLabel.stringValue=self.player.currentItem.error.localizedDescription ?: @"Audio konnte nicht geladen werden";
        [self.player pause]; self.playButton.enabled=NO; return;
    }
    if (self.player.currentItem.status==AVPlayerItemStatusReadyToPlay && self.needsInitialSeek) {
        self.needsInitialSeek=NO;
        [self seek:self.initialPosition];
    }
    double position=self.seeking ? self.timeline.position : CMTimeGetSeconds(self.player.currentTime);
    if (!isfinite(position)) position=0;
    self.timeline.position=position; self.timeline.needsDisplay=YES;
    double offset=0;
    for (NSUInteger i=0;i<self.durations.count;++i) {
        double end=offset+self.durations[i].doubleValue;
        if (position<end || i+1==self.durations.count) {
            double civil=self.recordedAt[i].doubleValue+MAX(0,position-offset);
            self.titleLabel.stringValue=PSAudioDateText(civil);
            self.detailLabel.stringValue=PSAudioClockText(civil);
            self.detailLabel.toolTip=self.labels[i];
            break;
        }
        offset=end;
    }
    self.playButton.title=self.player.rate==0 ? @"Wiedergabe" : @"Pause";
    self.playButton.enabled=self.player.currentItem.status==AVPlayerItemStatusReadyToPlay;
}
- (void)toggle:(id)sender {
    if (self.player.rate!=0) { self.resumeAfterSeek=NO; [self.player pause]; }
    else { self.resumeAfterSeek=YES; if (CMTimeGetSeconds(self.player.currentTime)>=self.timeline.total-0.01) [self seek:0]; else [self.player play]; }
}
- (void)back:(id)sender { if (!self.player) return; [self seek:CMTimeGetSeconds(self.player.currentTime)-15]; }
- (void)forward:(id)sender { if (!self.player) return; [self seek:CMTimeGetSeconds(self.player.currentTime)+15]; }
- (void)volume:(NSSlider *)sender { self.volumeLevel=sender.floatValue; self.player.volume=self.volumeLevel; }
- (void)seek:(double)seconds {
    if (!isfinite(seconds) || !self.player) return;
    self.resumeAfterSeek=self.player.rate!=0 || self.resumeAfterSeek;
    NSUInteger generation=++self.seekGeneration;
    self.seeking=YES;
    double target=MIN(self.timeline.total,MAX(0,seconds));
    self.timeline.position=target; self.timeline.needsDisplay=YES;
    __weak PSAudioController *weakSelf=self;
    [self.player seekToTime:CMTimeMakeWithSeconds(target,60000) toleranceBefore:kCMTimeZero toleranceAfter:kCMTimeZero completionHandler:^(BOOL finished){
        dispatch_async(dispatch_get_main_queue(), ^{
            PSAudioController *controller=weakSelf;
            if (controller && !controller.cancelled && generation==controller.seekGeneration) {
                controller.seeking=NO;
                if (finished && controller.resumeAfterSeek) [controller.player play];
            }
        });
    }];
}
@end
static NSTextField *PSAudioLabel(NSRect frame, CGFloat size) {
    NSTextField *label=[NSTextField labelWithString:@""];
    label.frame=frame; label.font=[NSFont systemFontOfSize:size]; label.textColor=NSColor.labelColor;
    label.alignment=NSTextAlignmentCenter;
    label.lineBreakMode=NSLineBreakByTruncatingMiddle;
    label.autoresizingMask=NSViewWidthSizable;
    return label;
}
int photosuite_audio_open(const char **paths, const char **labels, const double *durations, const double *recorded_at, size_t count, const char *year, double start) {
    if (![NSThread isMainThread] || count==0 || !isfinite(start)) return 1;
    @autoreleasepool {
        NSMutableArray *names=[NSMutableArray array], *descriptions=[NSMutableArray array], *lengths=[NSMutableArray array], *recorded=[NSMutableArray array];
        double total=0;
        for (size_t i=0;i<count;++i) {
            if (!isfinite(durations[i]) || durations[i]<=0 || !isfinite(recorded_at[i])) return 2;
            [names addObject:PSAudioPath(paths[i])]; [descriptions addObject:[NSString stringWithUTF8String:labels[i]]];
            [recorded addObject:@(recorded_at[i])]; [lengths addObject:@(durations[i])]; total+=durations[i];
        }
        if (!isfinite(total)) return 2;
        if (PSAudio) { [PSAudio shutdown]; [PSAudio close]; }
        NSWindow *window=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,740,210) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
        window.acceptsMouseMovedEvents=YES;
        window.appearance=[NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
        window.title=[NSString stringWithFormat:@"Diary %@ · Jahresverlauf",[NSString stringWithUTF8String:year]];
        window.minSize=NSMakeSize(440,232); window.maxSize=NSMakeSize(1600,232); window.releasedWhenClosed=NO;
        PSAudioController *controller=[[PSAudioController alloc] initWithWindow:window]; PSAudio=controller;
        controller.volumeLevel=0.8; controller.paths=names; controller.labels=descriptions; controller.durations=lengths; controller.recordedAt=recorded;
        window.delegate=controller;
        NSView *view=window.contentView;
        controller.titleLabel=PSAudioLabel(NSMakeRect(20,168,700,24),20); controller.titleLabel.stringValue=@"Jahresaufnahme wird geladen …"; [view addSubview:controller.titleLabel];
        controller.detailLabel=PSAudioLabel(NSMakeRect(20,142,700,22),16); controller.detailLabel.stringValue=@""; [view addSubview:controller.detailLabel];
        controller.timeline=[[PSAudioTimeline alloc] initWithFrame:NSMakeRect(12,75,716,58)];
        controller.timeline.autoresizingMask=NSViewWidthSizable; controller.timeline.controller=controller; controller.timeline.durations=lengths; controller.timeline.total=total; [view addSubview:controller.timeline];
        NSArray *titles=@[@"-15 s",@"Wiedergabe",@"+15 s"];
        SEL selectors[]={@selector(back:),@selector(toggle:),@selector(forward:)};
        for (NSUInteger i=0;i<3;++i) {
            NSButton *button=[NSButton buttonWithTitle:titles[i] target:controller action:selectors[i]];
            button.frame=NSMakeRect(20+i*95,24,90,30); button.enabled=NO; [view addSubview:button];
            if (i==1) controller.playButton=button;
            else button.enabled=YES;
        }
        NSSlider *volume=[NSSlider sliderWithValue:0.8 minValue:0 maxValue:1 target:controller action:@selector(volume:)];
        volume.frame=NSMakeRect(615,29,105,20); volume.autoresizingMask=NSViewMinXMargin; volume.toolTip=@"Lautstärke"; [view addSubview:volume];
        [window center]; [controller showWindow:nil]; [window makeKeyAndOrderFront:nil]; [window makeFirstResponder:controller.timeline];
        controller.timer=[NSTimer scheduledTimerWithTimeInterval:0.1 target:controller selector:@selector(tick:) userInfo:nil repeats:YES];
        __weak PSAudioController *weakSelf=controller;
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED,0), ^{
            @autoreleasepool {
                NSError *error=nil;
                AVMutableComposition *composition=PSBuildAudioComposition(names,lengths,^BOOL{ return weakSelf==nil || weakSelf.cancelled; },&error);
                dispatch_async(dispatch_get_main_queue(), ^{
                    PSAudioController *current=weakSelf;
                    if (!current || current.cancelled) return;
                    if (!composition) { current.titleLabel.stringValue=@"Jahresverlauf nicht abspielbar"; current.detailLabel.stringValue=error.localizedDescription ?: @"Audio konnte nicht geladen werden"; return; }
                    current.player=[AVPlayer playerWithPlayerItem:[AVPlayerItem playerItemWithAsset:composition]];
                    current.player.volume=current.volumeLevel; current.resumeAfterSeek=YES;
                    current.initialPosition=start; current.needsInitialSeek=YES;
                });
            }
        });
        return 0;
    }
}
