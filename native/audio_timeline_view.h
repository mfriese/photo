#pragma once
#import <AppKit/AppKit.h>
// Pure view projections supplied in domain order. Weight is not a number of seconds.
@interface PSAudioClipViewModel : NSObject
@property(nonatomic,copy) NSString *path;
@property(nonatomic,copy) NSString *label;
@property(nonatomic) double weight;
@property(nonatomic) double duration; // measured seconds or zero when unverified
@property(nonatomic) double recordedAt; // logical reference time
@property(nonatomic) double actualRecordedAt;
@property(nonatomic) unsigned int visibilityMask; // supplied by Rust; bits All/Diary/Comments
@property(nonatomic) unsigned int verification; // 0 pending, 1 verified, 2 failed
@end
@interface PSAudioTimeline : NSView
@property(nonatomic,copy) NSArray<PSAudioClipViewModel *> *clips;
@property(nonatomic,readonly) double total;
@property(nonatomic) double position;
@property(nonatomic,copy) void (^onSeek)(double position);
@property(nonatomic,copy) void (^onRelativeSeek)(double seconds);
@property(nonatomic,copy) void (^onToggle)(void);
- (NSUInteger)indexAtPosition:(double)position fraction:(double *)fraction;
- (double)offset:(NSUInteger)index;
@end
NSString *PSAudioDateText(double civil);
NSString *PSAudioClockText(double civil);
NSColor *PSAudioClipColor(unsigned int state,NSUInteger index);
