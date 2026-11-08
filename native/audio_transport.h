#pragma once
#import <AVFoundation/AVFoundation.h>

// Single-asset transport. No year ordering, date interpretation or timeline estimates.
// All public calls are main-thread only; one serial loader performs media I/O.
@interface PSAudioTransport : NSObject
@property(nonatomic,strong,readonly) AVPlayer *player;
@property(nonatomic,copy,readonly) NSString *path;
@property(nonatomic,copy,readonly) NSString *error;
@property(nonatomic,readonly) double duration;
@property(nonatomic,readonly) BOOL loading;
@property(nonatomic,readonly) BOOL seeking;
@property(nonatomic,readonly) BOOL hasStarted;
@property(nonatomic) BOOL wantsPlay;
@property(nonatomic,copy) void (^onEnd)(void);
@property(nonatomic,copy) void (^onSeek)(void);
- (void)openPath:(NSString *)path fraction:(double)fraction offset:(double)seconds;
- (void)seekSeconds:(double)seconds;
- (void)tick;
- (void)shutdown;
@end
