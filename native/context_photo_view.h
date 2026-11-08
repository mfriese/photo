#pragma once
#import <AppKit/AppKit.h>

// Presentation only: no dates parsed, media searched or files read here.
@interface PSContextPhotoSlot : NSObject
@property(nonatomic,strong) NSImage *image;
@property(nonatomic,copy) NSString *dateText;
@property(nonatomic,copy) NSString *clockText;
@property(nonatomic,copy) NSString *tooltip;
@property(nonatomic,copy) NSString *message;
@end

@interface PSContextPhotos : NSView
@property(nonatomic,strong,readonly) NSArray<PSContextPhotoSlot *> *slots;
@property(nonatomic,copy) NSString *statusText;
@property(nonatomic) BOOL loading;
@property(nonatomic,copy) void (^onSelect)(NSUInteger slot);
- (NSRect)slotRect:(NSUInteger)slot;
- (NSRect)captionRect:(NSUInteger)slot;
- (void)rebuildTips;
@end

// Centered aspect-fill geometry; the caller clips to the thumbnail frame.
NSRect PSContextPhotoFillRect(NSSize image, NSRect frame);
