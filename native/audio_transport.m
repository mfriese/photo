#import "audio_transport.h"
#include <math.h>
@interface PSAudioTransport ()
@property(nonatomic,strong,readwrite) AVPlayer *player;
@property(nonatomic,copy,readwrite) NSString *path;
@property(nonatomic,copy,readwrite) NSString *error;
@property(nonatomic,readwrite) double duration;
@property(nonatomic,readwrite) BOOL loading;
@property(nonatomic,readwrite) BOOL seeking;
@property(nonatomic,readwrite) BOOL hasStarted;
@property(atomic) NSUInteger generation;
@property(atomic) BOOL cancelled;
@property(nonatomic) NSUInteger seekGeneration;
@property(nonatomic) BOOL needsSeek;
@property(nonatomic) double target;
@property(nonatomic,strong) id endObserver;
@end
@implementation PSAudioTransport
- (instancetype)init {
    self=[super init];
    if(self){_player=[AVPlayer new];_player.automaticallyWaitsToMinimizeStalling=NO;_path=@"";_error=@"";_wantsPlay=YES;}
    return self;
}
- (void)openPath:(NSString *)path fraction:(double)fraction offset:(double)seconds {
    if(self.cancelled || !isfinite(fraction) || !isfinite(seconds))return;
    NSUInteger generation=++self.generation;self.seekGeneration++;
    self.path=path;self.error=@"";self.duration=0;self.loading=YES;self.seeking=NO;self.needsSeek=NO;
    [self.player pause];[self.player replaceCurrentItemWithPlayerItem:nil];
    if(self.endObserver){[[NSNotificationCenter defaultCenter] removeObserver:self.endObserver];self.endObserver=nil;}
    static dispatch_queue_t loader;static dispatch_once_t once;
    dispatch_once(&once,^{loader=dispatch_queue_create("de.photosuite.audio-asset",dispatch_queue_attr_make_with_qos_class(DISPATCH_QUEUE_SERIAL,QOS_CLASS_USER_INITIATED,0));});
    __weak PSAudioTransport *weakSelf=self;
    dispatch_async(loader,^{
        @autoreleasepool {
            if(!weakSelf || weakSelf.cancelled || weakSelf.generation!=generation)return;
            AVURLAsset *asset=[AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:path] options:@{AVURLAssetPreferPreciseDurationAndTimingKey:@NO}];
            double duration=CMTimeGetSeconds(asset.duration);
            BOOL valid=asset.playable && isfinite(duration) && duration>0 && [asset tracksWithMediaType:AVMediaTypeAudio].count>0 && [asset tracksWithMediaType:AVMediaTypeVideo].count==0;
            dispatch_async(dispatch_get_main_queue(),^{
                PSAudioTransport *current=weakSelf;
                if(!current || current.cancelled || current.generation!=generation)return;
                if(!valid){current.loading=NO;current.error=@"Aufnahme nicht von macOS abspielbar";return;}
                current.duration=duration;current.target=MIN(duration,MAX(0,duration*MIN(1,MAX(0,fraction))+seconds));
                current.needsSeek=YES;
                AVPlayerItem *item=[AVPlayerItem playerItemWithAsset:asset];
                [current.player replaceCurrentItemWithPlayerItem:item];
                current.endObserver=[[NSNotificationCenter defaultCenter] addObserverForName:AVPlayerItemDidPlayToEndTimeNotification object:item queue:NSOperationQueue.mainQueue usingBlock:^(NSNotification *note){
                    PSAudioTransport *owner=weakSelf;
                    if(owner && !owner.cancelled && owner.player.currentItem==note.object && owner.onEnd && owner.wantsPlay)owner.onEnd();
                }];
            });
        }
    });
}
- (void)seekSeconds:(double)seconds {
    if(self.cancelled || self.duration<=0 || !isfinite(seconds))return;
    NSUInteger generation=++self.seekGeneration;
    self.seeking=YES;
    __weak PSAudioTransport *weakSelf=self;
    [self.player seekToTime:CMTimeMakeWithSeconds(MIN(self.duration,MAX(0,seconds)),60000) toleranceBefore:kCMTimeZero toleranceAfter:kCMTimeZero completionHandler:^(BOOL finished){
        dispatch_async(dispatch_get_main_queue(),^{
            PSAudioTransport *current=weakSelf;
            if(!current || current.cancelled || current.seekGeneration!=generation)return;
            current.seeking=NO;current.loading=NO;
            if(finished){if(current.onSeek)current.onSeek();if(current.wantsPlay)[current.player play];}
            else current.error=@"Zeitposition konnte nicht geladen werden";
        });
    }];
}
- (void)useVerifiedDuration:(double)duration forPath:(NSString *)path {
    if(!self.cancelled && [self.path isEqualToString:path] && isfinite(duration) && duration>0)self.duration=duration;
}
- (void)tick {
    if(self.cancelled)return;
    if(self.player.currentItem.status==AVPlayerItemStatusFailed){self.error=self.player.currentItem.error.localizedDescription ?: @"Audio konnte nicht geladen werden";self.loading=NO;return;}
    if(self.needsSeek && self.player.currentItem.status==AVPlayerItemStatusReadyToPlay){self.needsSeek=NO;
        if(self.target<=0){self.loading=NO;if(self.wantsPlay)[self.player play];}
        else [self seekSeconds:self.target];
    }
    if(self.player.timeControlStatus==AVPlayerTimeControlStatusPlaying)self.hasStarted=YES;
}
- (void)shutdown {
    self.cancelled=YES;self.generation++;self.seekGeneration++;self.onEnd=nil;self.onSeek=nil;
    if(self.endObserver){[[NSNotificationCenter defaultCenter] removeObserver:self.endObserver];self.endObserver=nil;}
    [self.player pause];[self.player replaceCurrentItemWithPlayerItem:nil];
}
- (void)dealloc {if(self.endObserver)[[NSNotificationCenter defaultCenter] removeObserver:self.endObserver];}
@end
