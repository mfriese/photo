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
static void assertColor(NSColor *actual,NSColor *expected) {
    actual=[actual colorUsingColorSpace:NSColorSpace.deviceRGBColorSpace];expected=[expected colorUsingColorSpace:NSColorSpace.deviceRGBColorSpace];
    assert(fabs(actual.redComponent-expected.redComponent)<.035);
    assert(fabs(actual.greenComponent-expected.greenComponent)<.035);
    assert(fabs(actual.blueComponent-expected.blueComponent)<.035);
}
static NSColor *buttonPixel(PSAudioButton *button,NSInteger x,NSInteger y) {
    NSBitmapImageRep *bitmap=[[NSBitmapImageRep alloc] initWithBitmapDataPlanes:NULL pixelsWide:34 pixelsHigh:34 bitsPerSample:8 samplesPerPixel:4 hasAlpha:YES isPlanar:NO colorSpaceName:NSDeviceRGBColorSpace bytesPerRow:0 bitsPerPixel:0];
    [NSGraphicsContext saveGraphicsState];[NSGraphicsContext setCurrentContext:[NSGraphicsContext graphicsContextWithBitmapImageRep:bitmap]];
    [button drawRect:button.bounds];[NSGraphicsContext restoreGraphicsState];return [bitmap colorAtX:x y:y];
}
static void checkTransportTheme(void) {
    PSAudioButton *button=[[PSAudioButton alloc] initWithFrame:NSMakeRect(0,0,34,34)];button.image=PSAudioSymbolImage(PSAudioRecord);button.bordered=NO;
    assertColor(buttonPixel(button,17,17),PS_THEME_RECORD_RED);
    assertColor(buttonPixel(button,5,17),PS_THEME_RAISED);
    assertColor(buttonPixel(button,0,17),PS_THEME_BORDER);
    button.hovered=YES;assertColor(buttonPixel(button,5,17),PS_THEME_HOVER);assertColor(buttonPixel(button,0,17),PS_THEME_ACCENT);
    [button highlight:YES];assertColor(buttonPixel(button,5,17),PS_THEME_SELECTED);[button highlight:NO];
    button.hovered=NO;button.image=PSAudioSymbolImage(PSAudioPlay);assertColor(buttonPixel(button,17,17),PS_THEME_TEXT);
    button.enabled=NO;assertColor(buttonPixel(button,17,17),PS_THEME_DISABLED_TEXT);assertColor(buttonPixel(button,5,17),PS_THEME_DISABLED_BG);assertColor(buttonPixel(button,0,17),PS_THEME_DISABLED_BORDER);
}
int main(int argc,char **argv) {
    assert(argc==3);
    @autoreleasepool {
        [NSApplication sharedApplication];checkPhotoView();checkTransportTheme();
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
        controller.timeline.clips=PSAudioReadClips(paths,labels,weights,dates,durations,states,2);controller.sourceClips=controller.timeline.clips;
        assert(controller.timeline.total==160000 && controller.timeline.clips[0].verification==0);
        NSColor *gray=timelinePixel(controller.timeline);assert(fabs(gray.redComponent-gray.blueComponent)<0.1);
        controller.timeline.clips[0].verification=2;
        NSColor *failed=timelinePixel(controller.timeline);assert(failed.redComponent>failed.blueComponent+0.2);
        controller.timeline.clips[0].verification=0;
        [controller.transport openPath:PSAudioPath(argv[2]) fraction:0.5 offset:0];loaded(controller);
        assert(fabs(controller.transport.duration-3)<0.01);
        assert(fabs(CMTimeGetSeconds(controller.transport.player.currentTime)-1.5)<0.03);
        assert([controller.titleLabel.stringValue isEqualToString:@"Audio"]);
        assert([controller.detailLabel.stringValue isEqualToString:@"28.12.2023  00:00:00"]);
        assert(controller.photoRequest==1 && (photosuite_audio_state(42)&3)==1);
        assert((photosuite_audio_state(42)&16)!=0);
        assert(!controller.transport.hasStarted && controller.transport.player.rate==0);
        controller.transport.wantsPlay=YES;[controller.transport.player play];pump(0.25);[controller tick:nil];
        assert(CMTimeGetSeconds(controller.transport.player.currentTime)>1.55);
        assert(controller.transport.hasStarted && controller.photoRequest==1 && (photosuite_audio_state(42)&3)==3);
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
        photosuite_audio_context_status(42,request,"");
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
        // Embedded playback and the separate window share one transport.
        unsigned long long embedded=0;
        assert(photosuite_audio_open_mode(paths,labels,weights,dates,durations,states,2,"2023",argv[2],&embedded,0)==0);
        PSAudioController *compact=PSAudio;assert(!compact.window.visible);
        assert(photosuite_audio_control(embedded,5,0)==0);
        loaded(compact);
        assert(!compact.transport.hasStarted&&compact.transport.player.rate==0);
        assert(compact.photosStarted&&compact.photoRequest==1);
        unsigned long long previewSession=0,previewRequest=0;double previewTime=0;
        assert(photosuite_audio_context_state(&previewSession,&previewRequest,&previewTime)==3);
        assert(previewSession==embedded&&isfinite(previewTime));
        assert(photosuite_audio_context_state(&previewSession,&previewRequest,&previewTime)==1);
        assert(photosuite_audio_control(embedded,0,0)==0);
        loaded(compact);NSDate *started=[NSDate dateWithTimeIntervalSinceNow:5];
        while(!compact.transport.hasStarted){pump(0.02);[compact tick:nil];assert(started.timeIntervalSinceNow>0);}
        assert(photosuite_audio_control(embedded,0,0)==0);
        AVPlayerItem *shared=compact.transport.player.currentItem;
        double position=0,currentDate=0;
        assert(photosuite_audio_transport_state(embedded,&position,&currentDate)&1);
        unsigned long long requestBeforeOpening=compact.photoRequest;
        assert(photosuite_audio_control(embedded,2,0)==0 && compact.window.visible);
        assert(compact.photoRequest==requestBeforeOpening+1&&!compact.transport.wantsPlay);
        assert(compact.transport.player.currentItem==shared);
        [compact.window performClose:nil];pump(0.02);
        assert(!compact.window.visible && compact.transport.player.currentItem==shared);
        assert(photosuite_audio_transport_state(embedded,&position,&currentDate)&1);
        assert(photosuite_audio_control(embedded,1,0.5)==0);loaded(compact);
        // Native symbol controls remain centered and retain equal dimensions.
        for(NSNumber *width in @[@440,@740,@1600]){
            [compact.window setContentSize:NSMakeSize(width.doubleValue,560)];[compact.window.contentView layoutSubtreeIfNeeded];
            assert(fabs(NSMidX(compact.controlGroup.frame)-compact.window.contentView.bounds.size.width/2)<.5);
            assert(compact.controlGroup.arrangedSubviews.count==4);
            assert(NSMinY(compact.categoryControl.frame)-NSMaxY(compact.detailLabel.frame)>=12);
            assert(NSMinY(compact.titleLabel.frame)-NSMaxY(compact.detailLabel.frame)>=12);
            assert(NSMinY(compact.controlGroup.frame)>=0);
            assert(NSMaxY(compact.controlGroup.frame)<=NSMinY(compact.contextPhotos.frame));
            for(NSView *button in compact.controlGroup.arrangedSubviews){
                NSRect actual=[button convertRect:button.bounds toView:compact.window.contentView];
                assert(actual.size.width>0&&actual.size.height>0);
                assert(NSContainsRect(compact.window.contentView.bounds,actual));
                assert(NSMaxY(actual)<NSMinY(compact.contextPhotos.frame));
                assert(!button.isHiddenOrHasHiddenAncestor);
            }
            NSSize first=compact.controlGroup.arrangedSubviews[0].frame.size;
            assert(fabs(first.width-34)<.5&&fabs(first.height-34)<.5);
            for(NSButton *button in compact.controlGroup.arrangedSubviews){assert(button.title.length==0&&button.image&&!button.image.isTemplate&&!button.isBordered);assert(fabs(button.frame.size.width-first.width)<.5&&fabs(button.frame.size.height-first.height)<.5);}
        }
        compact.transport.wantsPlay=YES;[compact tick:nil];assert(compact.playButton.image==PSAudioSymbolImage(PSAudioPause));
        [compact toggle:nil];assert(!compact.transport.wantsPlay);assert(compact.playButton.image==PSAudioSymbolImage(PSAudioPlay));
        // Record intent is tied to the visible center photo; it never opens a microphone.
        compact.photoRequest=1234;compact.photosStarted=YES;compact.photoPath=compact.transport.path;compact.photoRecordedAt=compact.timeline.clips[[compact currentIndex]].recordedAt;compact.lastPhotoSeek=compact.settledPhotoSeek;
        compact.contextPhotos.loading=NO;compact.contextPhotos.slots[2].image=[[NSImage alloc] initWithSize:NSMakeSize(8,8)];[compact tick:nil];assert(compact.recordButton.enabled);
        [compact.recordButton performClick:nil];unsigned long long recordSession=0,recordRequest=0;unsigned int recordSlot=99;
        assert(photosuite_audio_context_record(&recordSession,&recordRequest,&recordSlot)==1);assert(recordSession==embedded&&recordRequest==1234&&recordSlot==2);assert(!compact.transport.wantsPlay&&!compact.window.visible);assert(photosuite_audio_context_record(&recordSession,&recordRequest,&recordSlot)==0);
        compact.contextPhotos.loading=YES;[compact recordCurrentPhoto:nil];assert(photosuite_audio_context_record(&recordSession,&recordRequest,&recordSlot)==0);
        compact.contextPhotos.loading=NO;[compact recordCurrentPhoto:nil];compact.photoRequest++;assert(photosuite_audio_context_record(&recordSession,&recordRequest,&recordSlot)==0);
        assert([compact.titleLabel.stringValue isEqualToString:@"2023"]);
        double actualDates[]={1800000000,1800000100};
        assert(photosuite_audio_recorded_dates(embedded,actualDates,2)==0);
        assert([compact numberOfRowsInTableView:compact.fileTable]==2);
        assert(![[compact tableView:compact.fileTable objectValueForTableColumn:compact.fileTable.tableColumns[1] row:0] isEqualToString:[compact tableView:compact.fileTable objectValueForTableColumn:compact.fileTable.tableColumns[2] row:0]]);
        NSTableColumn *fileColumn=compact.fileTable.tableColumns[0];
        assert([[compact tableView:compact.fileTable objectValueForTableColumn:fileColumn row:0] isEqualToString:compact.timeline.clips[0].path.lastPathComponent]);
        assert(NSMaxY(compact.fileTable.enclosingScrollView.frame)<NSMinY(compact.controlGroup.frame));
        // Filters change the visible playback queue, never the source timeline.
        unsigned int masks[compact.sourceClips.count];
        for(NSUInteger i=0;i<compact.sourceClips.count;i++)masks[i]=3;
        assert(photosuite_audio_visibility(embedded,masks,compact.sourceClips.count)==0);
        AVPlayerItem *retained=compact.transport.player.currentItem;
        compact.categoryControl.selectedSegment=0;[compact categoryChanged:compact.categoryControl];
        assert(compact.timeline.clips.count==compact.sourceClips.count);
        assert(compact.transport.player.currentItem==retained);
        compact.categoryControl.selectedSegment=1;[compact categoryChanged:compact.categoryControl];
        assert(compact.timeline.clips.count==0&&!compact.transport.wantsPlay);
        assert([compact numberOfRowsInTableView:compact.fileTable]==0);
        assert(!compact.playButton.enabled&&!compact.recordButton.enabled);
        assert(compact.playButton.image==PSAudioSymbolImage(PSAudioPlay));
        assert((photosuite_audio_state(embedded)&8)!=0);
        assert(photosuite_audio_control(embedded,4,0)==0);
        assert(compact.category==0&&compact.timeline.clips.count==compact.sourceClips.count);
        assert(compact.transport.player.currentItem==retained&&compact.transport.wantsPlay);
        assert(photosuite_audio_control(embedded,5,1)==0);
        assert(compact.category==1&&!compact.transport.wantsPlay&&compact.timeline.clips.count==2);
        assert([compact numberOfRowsInTableView:compact.fileTable]==2);
        assert(photosuite_audio_control(embedded,5,6)==0);
        assert(compact.category==2&&compact.timeline.clips.count==0);
        assert(photosuite_audio_control(embedded,4,0)==0);
        masks[0]=0;
        assert(photosuite_audio_visibility(embedded,masks,compact.sourceClips.count)!=0);
        assert(compact.timeline.clips.count==compact.sourceClips.count);
        // Missing reference dates invalidate old photos and never leave an endless spinner.
        compact.contextPhotos.slots[2].image=[[NSImage alloc] initWithSize:NSMakeSize(8,8)];
        compact.contextPhotos.slots[2].dateText=@"stale";
        unsigned long long oldPhotoRequest=compact.photoRequest;
        double unknownDates[]={NAN,NAN};
        assert(photosuite_audio_revise(embedded,paths,labels,weights,unknownDates,durations,states,2,"unknown reference")==0);
        assert(!compact.contextPhotos.slots[2].image&&compact.contextPhotos.slots[2].dateText.length==0);
        assert(!compact.contextPhotos.loading&&!compact.recordButton.enabled);
        unsigned long long unknownRequest=compact.photoRequest;[compact tick:nil];assert(compact.photoRequest==unknownRequest);
        photosuite_audio_context_caption(embedded,oldPhotoRequest,2,"stale","stale");
        assert(compact.contextPhotos.slots[2].dateText.length==0);
        assert(photosuite_audio_revise(embedded,paths,labels,weights,dates,durations,states,2,"reference restored")==0);
        assert(compact.photosStarted&&compact.photoRequest>unknownRequest);
        assert(photosuite_audio_control(embedded,3,0)==0);
        assert(!compact.transport.player.currentItem && photosuite_audio_state(embedded)==0);
        puts("Native audio: provisional weights/colors, real playback, in-place reorder, position preservation, seeks, corrected auto-advance, photo clicks and cancellation OK");
    }
    return 0;
}
