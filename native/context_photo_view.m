#import "theme.h"
#import "context_photo_view.h"

@implementation PSContextPhotoSlot
- (instancetype)init {
    self=[super init];
    if(self){_dateText=@"";_clockText=@"";_tooltip=@"";_message=@"Wird geladen";}
    return self;
}
@end

NSRect PSContextPhotoFillRect(NSSize image, NSRect frame) {
    CGFloat scale=MAX(frame.size.width/MAX(1,image.width),frame.size.height/MAX(1,image.height));
    NSSize size=NSMakeSize(image.width*scale,image.height*scale);
    return NSMakeRect(NSMidX(frame)-size.width/2,NSMidY(frame)-size.height/2,size.width,size.height);
}

@implementation PSContextPhotos
- (instancetype)initWithFrame:(NSRect)frame {
    self=[super initWithFrame:frame];
    if(self){
        NSMutableArray *slots=[NSMutableArray arrayWithCapacity:5];
        for(NSUInteger i=0;i<5;++i)[slots addObject:[PSContextPhotoSlot new]];
        _slots=[slots copy];_statusText=@"Fotos folgen nach dem Wiedergabestart";
    }
    return self;
}
- (NSRect)slotRect:(NSUInteger)slot {
    CGFloat gap=8,unit=MAX(1,(self.bounds.size.width-4*gap)/5.28),x=0;
    for(NSUInteger i=0;i<slot;++i)x+=unit*(i==2?1.28:1)+gap;
    CGFloat width=unit*(slot==2?1.28:1);
    CGFloat available=MAX(1,self.bounds.size.height-56);
    CGFloat centerHeight=MIN(available,unit*1.28*0.75);
    CGFloat height=centerHeight*(slot==2?1:0.8);
    return NSMakeRect(x,56+(available-height)/2,width,height);
}
- (NSRect)captionRect:(NSUInteger)slot {
    NSRect image=[self slotRect:slot];
    return NSMakeRect(image.origin.x,image.origin.y-34,image.size.width,30);
}
- (NSDictionary *)textAttributes {
    NSMutableParagraphStyle *paragraph=[NSMutableParagraphStyle new];
    paragraph.alignment=NSTextAlignmentCenter;
    paragraph.lineBreakMode=NSLineBreakByTruncatingTail;
    return @{NSFontAttributeName:[NSFont monospacedDigitSystemFontOfSize:12 weight:NSFontWeightRegular],
             NSForegroundColorAttributeName:PS_THEME_TEXT,
             NSParagraphStyleAttributeName:paragraph};
}
- (void)drawRect:(NSRect)dirty {
    NSDictionary *attributes=[self textAttributes];
    for(NSUInteger i=0;i<self.slots.count;++i){
        PSContextPhotoSlot *slot=self.slots[i];
        NSRect rect=[self slotRect:i];
        NSBezierPath *frame=[NSBezierPath bezierPathWithRoundedRect:rect xRadius:6 yRadius:6];
        [PS_THEME_CANVAS setFill];[frame fill];
        if(slot.image){
            [NSGraphicsContext saveGraphicsState];[frame addClip];
            [slot.image drawInRect:PSContextPhotoFillRect(slot.image.size,rect) fromRect:NSZeroRect
                operation:NSCompositingOperationSourceOver fraction:(self.loading?0.4:1)
                respectFlipped:YES hints:nil];
            [NSGraphicsContext restoreGraphicsState];
        }else{
            NSSize size=[slot.message sizeWithAttributes:attributes];
            NSString *text=size.width<=rect.size.width-4 ? slot.message : @"…";
            [text drawInRect:NSMakeRect(rect.origin.x,NSMidY(rect)-8,rect.size.width,16) withAttributes:attributes];
        }
        [(i==2?PS_THEME_ACCENT:PS_THEME_BORDER) setStroke];
        frame.lineWidth=i==2?2:0.5;[frame stroke];
        NSRect caption=[self captionRect:i];
        [slot.dateText drawInRect:NSMakeRect(caption.origin.x,caption.origin.y+15,caption.size.width,15) withAttributes:attributes];
        [slot.clockText drawInRect:NSMakeRect(caption.origin.x,caption.origin.y,caption.size.width,15) withAttributes:attributes];
    }
    // Only progress/errors belong here; normal content is the date under each photo.
    [self.statusText drawInRect:NSMakeRect(0,0,self.bounds.size.width,18) withAttributes:attributes];
}
- (void)resetCursorRects {
    [super resetCursorRects];
    if(self.loading)return;
    for(NSUInteger i=0;i<self.slots.count;++i) {
        if(self.slots[i].image)[self addCursorRect:NSUnionRect([self slotRect:i],[self captionRect:i]) cursor:NSCursor.pointingHandCursor];
    }
}
- (BOOL)acceptsFirstMouse:(NSEvent *)event { return YES; }
- (void)mouseDown:(NSEvent *)event {
    if(self.loading || !self.onSelect)return;
    NSPoint point=[self convertPoint:event.locationInWindow fromView:nil];
    for(NSUInteger i=0;i<self.slots.count;++i) {
        if(self.slots[i].image && NSPointInRect(point,NSUnionRect([self slotRect:i],[self captionRect:i]))) {
            self.onSelect(i);return;
        }
    }
}
- (void)rebuildTips {
    [self.window invalidateCursorRectsForView:self];
    [self removeAllToolTips];
    for(NSUInteger i=0;i<self.slots.count;++i)
        [self addToolTipRect:NSUnionRect([self slotRect:i],[self captionRect:i]) owner:self userData:(void *)(uintptr_t)i];
}
- (void)setFrameSize:(NSSize)size {
    [super setFrameSize:size];[self rebuildTips];self.needsDisplay=YES;
}
- (NSString *)view:(NSView *)view stringForToolTip:(NSToolTipTag)tag point:(NSPoint)point userData:(void *)data {
    NSUInteger index=(NSUInteger)(uintptr_t)data;
    return index<self.slots.count ? self.slots[index].tooltip : @"";
}
@end
