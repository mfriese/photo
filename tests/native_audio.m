// Automated integration: a real AVFoundation composition, seek across files and cleanup.
#import "../native/audio.m"
#include <assert.h>
static void pump(double seconds) {
    NSDate *end=[NSDate dateWithTimeIntervalSinceNow:seconds];
    while ([end timeIntervalSinceNow]>0) [[NSRunLoop currentRunLoop] runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];
}
static void checkPhotoView(void) {
    PSContextPhotos *view=[[PSContextPhotos alloc] initWithFrame:NSMakeRect(0,0,700,160)];
    view.statusText=@"";
    for(NSNumber *width in @[@400,@700,@1560]) {
        [view setFrameSize:NSMakeSize(width.doubleValue,160)];
        for(NSUInteger slot=0;slot<5;++slot) {
            NSRect image=[view slotRect:slot],caption=[view captionRect:slot];
            assert(NSMaxY(caption)<NSMinY(image) && NSMinY(caption)>=18);
            assert(NSMaxY(image)<=160 && image.size.height>0);
            assert(caption.size.width==image.size.width);
            for(NSValue *size in @[[NSValue valueWithSize:NSMakeSize(4,16)],[NSValue valueWithSize:NSMakeSize(16,4)]]) {
                NSRect fill=PSContextPhotoFillRect(size.sizeValue,image);
                assert(NSContainsRect(fill,image));
                assert(fabs(NSMidX(fill)-NSMidX(image))<0.001 && fabs(NSMidY(fill)-NSMidY(image))<0.001);
            }
        }
        assert(fabs(NSMaxX([view slotRect:4])-width.doubleValue)<0.001);
    }
    [view setFrameSize:NSMakeSize(700,160)];
    for(NSNumber *portrait in @[@YES,@NO]) {
        NSSize size=portrait.boolValue?NSMakeSize(4,16):NSMakeSize(16,4);
        NSImage *red=[NSImage imageWithSize:size flipped:NO drawingHandler:^BOOL(NSRect rect){
            [NSColor.redColor setFill];NSRectFill(rect);return YES;
        }];
        for(PSContextPhotoSlot *slot in view.slots){slot.image=red;slot.dateText=@"27.12.2023";slot.clockText=@"22:51:32";}
        NSBitmapImageRep *bitmap=[[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:700 pixelsHigh:160 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
        [NSGraphicsContext saveGraphicsState];
        [NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap]];
        [NSColor.blackColor setFill];NSRectFill(view.bounds);[view drawRect:view.bounds];
        [NSGraphicsContext restoreGraphicsState];
        for(NSUInteger i=0;i<5;++i){
            NSRect rect=[view slotRect:i];
            for(NSValue *point in @[[NSValue valueWithPoint:NSMakePoint(NSMinX(rect)+3,NSMidY(rect))],
                [NSValue valueWithPoint:NSMakePoint(NSMidX(rect),NSMinY(rect)+3)]]) {
                NSColor *pixel=[[bitmap colorAtX:(NSInteger)point.pointValue.x y:159-(NSInteger)point.pointValue.y] colorUsingColorSpace:NSColorSpace.deviceRGBColorSpace];
                assert(pixel.redComponent>0.8 && pixel.greenComponent<0.2);
            }
        }
    }
    __block NSUInteger selected=NSNotFound;
    view.onSelect=^(NSUInteger slot){selected=slot;};
    NSRect rect=[view slotRect:2];
    NSEvent *click=[NSEvent mouseEventWithType:NSEventTypeLeftMouseDown location:NSMakePoint(NSMidX(rect),NSMidY(rect)) modifierFlags:0 timestamp:0 windowNumber:0 context:nil eventNumber:1 clickCount:1 pressure:1];
    [view mouseDown:click];assert(selected==2);
    selected=NSNotFound;view.loading=YES;[view mouseDown:click];assert(selected==NSNotFound);
    view.loading=NO;view.slots[2].image=nil;[view mouseDown:click];assert(selected==NSNotFound);
}


static void loaded(PSAudioController *controller) {
    NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:5];
    do{pump(0.01);[controller tick:nil];assert(deadline.timeIntervalSinceNow>0);}while(controller.transport.loading || controller.transport.seeking);
    assert(controller.transport.error.length==0);
}
static NSColor *timelinePixel(PSAudioTimeline *view) {
    NSBitmapImageRep *bitmap=[[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:300 pixelsHigh:58 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    [NSGraphicsContext saveGraphicsState];[NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap]];
    [view drawRect:view.bounds];[NSGraphicsContext restoreGraphicsState];
    return [[bitmap colorAtX:20 y:58-1-25] colorUsingColorSpace:NSColorSpace.deviceRGBColorSpace];
}
int main(int argc,char **argv) {
    assert(argc==3);
    @autoreleasepool {
        [NSApplication sharedApplication];checkPhotoView();
        NSWindow *audioWindow=PSAudioCreateWindow();
        assert(audioWindow.level>NSNormalWindowLevel);
        assert(audioWindow.hidesOnDeactivate);
        assert((audioWindow.styleMask & NSWindowStyleMaskClosable)!=0);
        [audioWindow close];
        PSAudioController *controller=[PSAudioController new];PSAudio=controller;controller.photoSession=42;
        controller.transport=[PSAudioTransport new];controller.transport.player.muted=YES;controller.transport.wantsPlay=NO;
        controller.timeline=[[PSAudioTimeline alloc] initWithFrame:NSMakeRect(0,0,300,58)];
        controller.contextPhotos=[[PSContextPhotos alloc] initWithFrame:NSMakeRect(0,0,700,160)];
        controller.titleLabel=[NSTextField labelWithString:@""];controller.detailLabel=[NSTextField labelWithString:@""];controller.verificationLabel=[NSTextField labelWithString:@""];
        [controller connectControls];
        const char *paths[]={argv[1],argv[2]},*labels[]={"provisional a","provisional b"};
        double weights[]={16000,144000},durations[]={0,0};unsigned int states[]={0,0};
        NSDateFormatter *date=[NSDateFormatter new];date.locale=[NSLocale localeWithLocaleIdentifier:@"en_US_POSIX"];date.timeZone=[NSTimeZone timeZoneForSecondsFromGMT:0];date.dateFormat=@"yyyyMMdd_HHmmss";
        double late=[[date dateFromString:@"20231227_235959"] timeIntervalSince1970];
        double dates[]={late-3600,late};
        controller.timeline.clips=PSAudioReadClips(paths,labels,weights,dates,durations,states,2);
        assert(controller.timeline.total==160000 && controller.timeline.clips[0].verification==0);
        NSColor *gray=timelinePixel(controller.timeline);assert(fabs(gray.redComponent-gray.blueComponent)<0.1);
        controller.timeline.clips[0].verification=2;
        NSColor *failed=timelinePixel(controller.timeline);assert(failed.redComponent>failed.blueComponent+0.2);
        controller.timeline.clips[0].verification=0;
        [controller.transport openPath:PSAudioPath(argv[2]) fraction:0.5 offset:0];loaded(controller);
        assert(fabs(controller.transport.duration-3)<0.01);
        assert(fabs(CMTimeGetSeconds(controller.transport.player.currentTime)-1.5)<0.03);
        assert([controller.titleLabel.stringValue isEqualToString:@"28.12.2023"]);
        assert([controller.detailLabel.stringValue isEqualToString:@"00:00:00"]);
        assert(controller.photoRequest==0 && photosuite_audio_state(42)==1);
        controller.transport.wantsPlay=YES;[controller.transport.player play];pump(0.25);[controller tick:nil];
        assert(CMTimeGetSeconds(controller.transport.player.currentTime)>1.55);
        assert(controller.transport.hasStarted && controller.photoRequest==1 && photosuite_audio_state(42)==3);
        [controller tick:nil];assert(controller.photoRequest==1);
        controller.transport.wantsPlay=NO;[controller.transport.player pause];
        double paused=CMTimeGetSeconds(controller.transport.player.currentTime);AVPlayerItem *item=controller.transport.player.currentItem;
        [controller.transport useVerifiedDuration:3.25 forPath:controller.transport.path];
        assert(fabs(controller.transport.duration-3.25)<0.001 && controller.transport.player.currentItem==item);
        assert(fabs(CMTimeGetSeconds(controller.transport.player.currentTime)-paused)<0.01);
        [controller.transport useVerifiedDuration:99 forPath:@"/unrelated.wav"];
        assert(fabs(controller.transport.duration-3.25)<0.001);
        [controller.transport useVerifiedDuration:3 forPath:controller.transport.path];
        const char *reordered[]={argv[2],argv[1]},*checkedLabels[]={"verified b","verified a"};
        double correctedWeights[]={48000,16000},correctedDates[]={late-86400,late},correctedDurations[]={3,1};unsigned int checked[]={1,1};
        assert(photosuite_audio_revise(41,reordered,checkedLabels,correctedWeights,correctedDates,correctedDurations,checked,2,"stale")==1);
        assert(photosuite_audio_revise(42,reordered,checkedLabels,correctedWeights,correctedDates,correctedDurations,checked,2,"all verified")==0);
        assert(controller.transport.player.currentItem==item);
        assert([controller.transport.path isEqualToString:PSAudioPath(argv[2])]);
        assert(fabs(CMTimeGetSeconds(controller.transport.player.currentTime)-paused)<0.03);
        assert(controller.timeline.clips[0].verification==1 && controller.timeline.total==64000);
        assert([controller currentIndex]==0 && controller.photoRequest==2);
        NSColor *blue=timelinePixel(controller.timeline);assert(blue.blueComponent>blue.redComponent+0.2);
        assert(controller.timeline.position<48000);
        [controller tick:nil];assert(controller.photoRequest==2);
        double fraction=0;assert([controller.timeline indexAtPosition:52000 fraction:&fraction]==1 && fabs(fraction-0.25)<0.001);
        [controller seek:52000];loaded(controller);pump(0.3);[controller tick:nil];
        assert([controller.transport.path isEqualToString:PSAudioPath(argv[1])]);
        assert(fabs(CMTimeGetSeconds(controller.transport.player.currentTime)-0.25)<0.03);
        unsigned long long session=0,request=0;double civil=0;
        assert(photosuite_audio_context_state(&session,&request,&civil)==3 && session==42);
        assert(photosuite_audio_context_state(&session,&request,&civil)==1);
        photosuite_audio_context_caption(42,request,2,"27.12.2023","22:51:32");
        assert([controller.contextPhotos.slots[2].dateText isEqualToString:@"27.12.2023"]);
        photosuite_audio_context_caption(42,request-1,2,"stale","stale");
        assert([controller.contextPhotos.slots[2].dateText isEqualToString:@"27.12.2023"]);
        photosuite_audio_context_status(42,request-1,"stale");assert(![controller.contextPhotos.statusText isEqualToString:@"stale"]);
        controller.contextPhotos.onSelect(2);unsigned int clicked=0;
        assert(photosuite_audio_context_click(&session,&request,&clicked)==1 && clicked==2);
        assert(photosuite_audio_context_click(&session,&request,&clicked)==0);
        // Natural end follows the corrected order, not the initial filename order.
        controller.transport.wantsPlay=YES;[controller.transport openPath:PSAudioPath(argv[2]) fraction:0.97 offset:0];
        loaded(controller);NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:5];
        while(![controller.transport.path isEqualToString:PSAudioPath(argv[1])]){pump(0.01);[controller tick:nil];assert(deadline.timeIntervalSinceNow>0);}
        loaded(controller);assert(controller.transport.hasStarted);
        // Late asynchronous loads cannot reactivate a closed/replaced transport.
        [controller.transport openPath:PSAudioPath(argv[1]) fraction:0 offset:0];
        [controller.transport openPath:PSAudioPath(argv[2]) fraction:0 offset:0];loaded(controller);
        assert([controller.transport.path isEqualToString:PSAudioPath(argv[2])]);
        [controller.transport openPath:PSAudioPath(argv[1]) fraction:0 offset:0];[controller shutdown];pump(0.1);
        assert(!controller.transport.player.currentItem && controller.transport.player.rate==0);
        assert(photosuite_audio_state(42)==0 && photosuite_audio_context_state(&session,&request,&civil)==0);
        puts("Native audio: provisional weights/colors, real playback, in-place reorder, position preservation, seeks, corrected auto-advance, photo clicks and cancellation OK");
    }
    return 0;
}
