#import "audio_timeline_view.h"
#include <math.h>
@implementation PSAudioClipViewModel
@end
NSString *PSAudioDateText(double civil) {
    if(!isfinite(civil))return @"Datum wird geprüft";
    static NSDateFormatter *formatter;static dispatch_once_t once;
    dispatch_once(&once,^{formatter=[NSDateFormatter new];formatter.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];formatter.timeZone=[NSTimeZone timeZoneForSecondsFromGMT:0];formatter.dateFormat=@"dd.MM.yyyy";});
    return [formatter stringFromDate:[NSDate dateWithTimeIntervalSince1970:civil]];
}
NSString *PSAudioClockText(double civil) {
    if(!isfinite(civil))return @"";
    static NSDateFormatter *formatter;static dispatch_once_t once;
    dispatch_once(&once,^{formatter=[NSDateFormatter new];formatter.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];formatter.timeZone=[NSTimeZone timeZoneForSecondsFromGMT:0];formatter.dateFormat=@"HH:mm:ss";});
    return [formatter stringFromDate:[NSDate dateWithTimeIntervalSince1970:civil]];
}
NSColor *PSAudioClipColor(unsigned int state,NSUInteger index) {
    if(state==0)return [NSColor colorWithSRGBRed:0.28 green:0.30 blue:0.34 alpha:1];
    if(state==2)return [NSColor colorWithSRGBRed:0.65 green:0.25 blue:0.20 alpha:1];
    return [NSColor colorWithSRGBRed:(index%2?0.36:0.23) green:(index%2?0.55:0.40) blue:(index%2?0.79:0.65) alpha:1];
}
@interface PSAudioTimeline ()
@property(nonatomic,strong) NSTrackingArea *tracking;
@end
@implementation PSAudioTimeline
- (BOOL)acceptsFirstResponder {return YES;}
- (BOOL)isFlipped {return YES;}
- (double)total {double sum=0;for(PSAudioClipViewModel *clip in self.clips)sum+=clip.weight;return sum;}
- (double)offset:(NSUInteger)index {double sum=0;for(NSUInteger i=0;i<MIN(index,self.clips.count);++i)sum+=self.clips[i].weight;return sum;}
- (NSUInteger)indexAtPosition:(double)position fraction:(double *)fraction {
    double offset=0;position=MAX(0,MIN(self.total,position));
    for(NSUInteger i=0;i<self.clips.count;++i){
        double weight=self.clips[i].weight;
        if(position<offset+weight || i+1==self.clips.count){*fraction=MIN(1,MAX(0,(position-offset)/weight));return i;}
        offset+=weight;
    }
    *fraction=0;return NSNotFound;
}
- (void)drawRect:(NSRect)dirty {
    double total=MAX(1,self.total),offset=0;CGFloat width=MAX(1,self.bounds.size.width-16);
    for(NSUInteger i=0;i<self.clips.count;++i){
        PSAudioClipViewModel *clip=self.clips[i];
        NSRect segment=NSMakeRect(8+width*offset/total,15,width*clip.weight/total,28);
        [PSAudioClipColor(clip.verification,i) setFill];NSRectFill(segment);
        [[NSColor colorWithWhite:0.92 alpha:1] setFill];NSRectFill(NSMakeRect(segment.origin.x,15,1,28));offset+=clip.weight;
    }
    CGFloat x=8+width*MIN(1,MAX(0,self.position/total));
    [NSColor.whiteColor setFill];NSRectFill(NSMakeRect(x-1,8,2,42));
    [[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(x-5,4,10,10)] fill];
}
- (void)updateTrackingAreas {
    [super updateTrackingAreas];if(self.tracking)[self removeTrackingArea:self.tracking];
    self.tracking=[[NSTrackingArea alloc] initWithRect:NSZeroRect options:NSTrackingMouseMoved|NSTrackingActiveInKeyWindow|NSTrackingInVisibleRect owner:self userInfo:nil];[self addTrackingArea:self.tracking];
}
- (double)positionForEvent:(NSEvent *)event {NSPoint point=[self convertPoint:event.locationInWindow fromView:nil];return self.total*MIN(1,MAX(0,(point.x-8)/MAX(1,self.bounds.size.width-16)));}
- (void)mouseMoved:(NSEvent *)event {
    double fraction=0;NSUInteger index=[self indexAtPosition:[self positionForEvent:event] fraction:&fraction];
    if(index!=NSNotFound)self.toolTip=self.clips[index].label;
}
- (void)mouseDown:(NSEvent *)event {[self mouseDragged:event];}
- (void)mouseDragged:(NSEvent *)event {if(self.onSeek)self.onSeek([self positionForEvent:event]);}
- (void)keyDown:(NSEvent *)event {
    if(event.keyCode==49 && !event.isARepeat && self.onToggle)self.onToggle();
    else if(event.keyCode==123 && self.onRelativeSeek)self.onRelativeSeek(-5);
    else if(event.keyCode==124 && self.onRelativeSeek)self.onRelativeSeek(5);
    else if(event.keyCode==53)[self.window performClose:nil];else [super keyDown:event];
}
@end
