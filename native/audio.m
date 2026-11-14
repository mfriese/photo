#include "photosuite.h"
#import "context_photo_view.h"
#import "audio_timeline_view.h"
#import "audio_transport.h"
#include <math.h>
#import "theme.h"
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

typedef NS_ENUM(NSInteger,PSAudioSymbol){PSAudioBack,PSAudioPlay,PSAudioPause,PSAudioRecord,PSAudioForward};
static NSImage *PSAudioSymbolImage(PSAudioSymbol symbol){
    static NSImage *images[5];if(images[symbol])return images[symbol];
    NSImage *image=[NSImage imageWithSize:NSMakeSize(18,18) flipped:NO drawingHandler:^BOOL(NSRect rect){
        [(symbol==PSAudioRecord?PS_THEME_RECORD_RED:PS_THEME_TEXT) setFill];
        if(symbol==PSAudioRecord){[[NSBezierPath bezierPathWithOvalInRect:NSMakeRect(3,3,12,12)] fill];}
        else if(symbol==PSAudioPause){NSRectFill(NSMakeRect(4,2.5,3,13));NSRectFill(NSMakeRect(11,2.5,3,13));}
        else if(symbol==PSAudioPlay){NSBezierPath *p=[NSBezierPath bezierPath];[p moveToPoint:NSMakePoint(5,3)];[p lineToPoint:NSMakePoint(15,9)];[p lineToPoint:NSMakePoint(5,15)];[p closePath];[p fill];}
        else {for(int i=0;i<2;i++){CGFloat x=2+i*7;NSBezierPath *p=[NSBezierPath bezierPath];if(symbol==PSAudioBack){[p moveToPoint:NSMakePoint(x,9)];[p lineToPoint:NSMakePoint(x+7,15)];[p lineToPoint:NSMakePoint(x+7,3)];}else{[p moveToPoint:NSMakePoint(x+7,9)];[p lineToPoint:NSMakePoint(x,15)];[p lineToPoint:NSMakePoint(x,3)];}[p closePath];[p fill];}}
        return YES;
    }];image.template=NO;images[symbol]=image;return image;
}
// Match egui transport visuals, including border, hover, pressed and disabled states.
@interface PSAudioButton : NSButton
@property(nonatomic,strong) NSTrackingArea *hoverArea;
@property(nonatomic) BOOL hovered;
@end
@implementation PSAudioButton
- (void)updateTrackingAreas {
    [super updateTrackingAreas];if(self.hoverArea)[self removeTrackingArea:self.hoverArea];
    self.hoverArea=[[NSTrackingArea alloc] initWithRect:NSZeroRect options:NSTrackingMouseEnteredAndExited|NSTrackingActiveInActiveApp|NSTrackingInVisibleRect owner:self userInfo:nil];[self addTrackingArea:self.hoverArea];
}
- (void)mouseEntered:(NSEvent *)event {self.hovered=YES;self.needsDisplay=YES;}
- (void)mouseExited:(NSEvent *)event {self.hovered=NO;self.needsDisplay=YES;}
- (void)drawRect:(NSRect)dirty {
    BOOL active=self.enabled&&self.cell.highlighted,hover=self.enabled&&self.hovered;
    NSColor *fill=!self.enabled?PS_THEME_DISABLED_BG:active?PS_THEME_SELECTED:hover?PS_THEME_HOVER:PS_THEME_RAISED;
    NSColor *border=!self.enabled?PS_THEME_DISABLED_BORDER:(active||hover)?PS_THEME_ACCENT:PS_THEME_BORDER;
    NSBezierPath *path=[NSBezierPath bezierPathWithRoundedRect:NSInsetRect(self.bounds,.5,.5) xRadius:5 yRadius:5];
    [fill setFill];[path fill];[border setStroke];path.lineWidth=1;[path stroke];
    NSRect icon=NSMakeRect(NSMidX(self.bounds)-9,NSMidY(self.bounds)-9,18,18);
    if(self.enabled){[self.image drawInRect:icon fromRect:NSZeroRect operation:NSCompositingOperationSourceOver fraction:1 respectFlipped:YES hints:nil];}
    else {NSImage *muted=[NSImage imageWithSize:NSMakeSize(18,18) flipped:NO drawingHandler:^BOOL(NSRect rect){[self.image drawInRect:rect];[PS_THEME_DISABLED_TEXT setFill];NSRectFillUsingOperation(rect,NSCompositingOperationSourceIn);return YES;}];[muted drawInRect:icon];}
}
@end
@interface PSAudioController : NSWindowController <NSWindowDelegate, NSTableViewDataSource>
@property(nonatomic,strong) PSAudioTransport *transport;
@property(nonatomic,strong) PSAudioTimeline *timeline;
@property(nonatomic,strong) PSContextPhotos *contextPhotos;
@property(nonatomic,strong) NSTimer *timer;
@property(nonatomic,strong) NSButton *playButton;
@property(nonatomic,strong) NSButton *recordButton;
@property(nonatomic,strong) NSStackView *controlGroup;
@property(nonatomic,strong) NSSegmentedControl *categoryControl;
@property(nonatomic,copy) NSArray<PSAudioClipViewModel *> *sourceClips;
@property(nonatomic) NSInteger category;
@property(nonatomic) BOOL wantsBeforeEmpty;
@property(nonatomic) BOOL photoRecordPending;
@property(nonatomic) unsigned long long recordedPhotoRequest;
@property(nonatomic,strong) NSTextField *titleLabel;
@property(nonatomic,copy) NSString *periodLabel;
@property(nonatomic,strong) NSTableView *fileTable;
@property(nonatomic,strong) NSTextField *detailLabel;
@property(nonatomic,strong) NSTextField *verificationLabel;
@property(nonatomic) BOOL cancelled;
@property(nonatomic) unsigned long long photoSession;
@property(nonatomic) unsigned long long photoRequest;
@property(nonatomic) BOOL photoPending;
@property(nonatomic) BOOL photosStarted;
@property(nonatomic,copy) NSString *photoPath;
@property(nonatomic) double photoRecordedAt;
@property(nonatomic) NSUInteger settledPhotoSeek;
@property(nonatomic) NSUInteger lastPhotoSeek;
@property(nonatomic) double photoSeekFinishedAt;
@property(nonatomic) double photoCivil;
@property(nonatomic) BOOL photoClickPending;
@property(nonatomic) NSUInteger clickedPhotoSlot;
@property(nonatomic) unsigned long long clickedPhotoRequest;
- (void)seek:(double)position;
- (void)seekRelative:(double)seconds;
- (void)shutdown;
- (void)connectControls;
@end
static PSAudioController *PSAudio=nil;
static unsigned long long PSNextPhotoSession=0;
@implementation PSAudioController
- (NSInteger)numberOfRowsInTableView:(NSTableView *)tableView { return self.timeline.clips.count; }
- (id)tableView:(NSTableView *)tableView objectValueForTableColumn:(NSTableColumn *)column row:(NSInteger)row {
    if(row<0||(NSUInteger)row>=self.timeline.clips.count)return @"";
    PSAudioClipViewModel *clip=self.timeline.clips[row];
    if([column.identifier isEqualToString:@"file"])return clip.path.lastPathComponent;
    double value=[column.identifier isEqualToString:@"reference"]?clip.recordedAt:clip.actualRecordedAt;
    return isfinite(value)?[NSString stringWithFormat:@"%@ %@",PSAudioDateText(value),PSAudioClockText(value)]:@"Unbekannt";
}
- (void)playSelectedFile:(id)sender {
    NSInteger row=self.fileTable.clickedRow;if(row<0||(NSUInteger)row>=self.timeline.clips.count)return;
    [self seek:[self.timeline offset:row]];
}
- (void)applyCategory {
    BOOL wasEmpty=self.timeline.clips.count==0;
    BOOL desired=wasEmpty?self.wantsBeforeEmpty:self.transport.wantsPlay;
    NSUInteger mask=1u<<self.category;
    NSArray *visible=self.category==0?self.sourceClips:[self.sourceClips filteredArrayUsingPredicate:[NSPredicate predicateWithBlock:^BOOL(PSAudioClipViewModel *clip,NSDictionary *bindings){return (clip.visibilityMask&mask)!=0;}]];
    self.timeline.clips=visible;self.timeline.needsDisplay=YES;[self.fileTable reloadData];
    if(visible.count==0){
        if(!wasEmpty)self.wantsBeforeEmpty=desired;self.transport.wantsPlay=NO;[self.transport.player pause];self.timeline.position=0;
        self.photoRequest++;self.photoPending=NO;self.photoClickPending=NO;self.photoRecordPending=NO;self.photosStarted=NO;
        self.contextPhotos.loading=NO;self.contextPhotos.statusText=@"Keine Aufnahmen für diese Auswahl";
        for(PSContextPhotoSlot *slot in self.contextPhotos.slots){slot.image=nil;slot.dateText=@"";slot.clockText=@"";slot.tooltip=@"";}[self.contextPhotos rebuildTips];self.contextPhotos.needsDisplay=YES;
        [self tick:nil];return;
    }
    self.transport.wantsPlay=desired;
    if([self currentIndex]==NSNotFound){self.photosStarted=NO;self.photoPending=NO;self.photoRequest++;self.contextPhotos.loading=YES;for(PSContextPhotoSlot *slot in self.contextPhotos.slots){slot.image=nil;slot.dateText=@"";slot.clockText=@"";slot.tooltip=@"";}[self.contextPhotos rebuildTips];[self.transport openPath:self.timeline.clips[0].path fraction:0 offset:0];}
    else if(desired&&!self.transport.loading&&!self.transport.seeking){[self.transport.player play];}
    [self tick:nil];
}
- (void)categoryChanged:(NSSegmentedControl *)sender {
    NSInteger values[]={1,2,0};NSInteger index=sender.selectedSegment;if(index<0||index>2)return;
    self.category=values[index];[self applyCategory];
}
- (void)connectControls {
        __weak PSAudioController *owner=self;
        self.timeline.onSeek=^(double position){[owner seek:position];};self.timeline.onRelativeSeek=^(double seconds){[owner seekRelative:seconds];};self.timeline.onToggle=^{[owner toggle:nil];};
        self.contextPhotos.onSelect=^(NSUInteger slot){PSAudioController *current=owner;if(!current || current.cancelled)return;current.clickedPhotoSlot=slot;current.clickedPhotoRequest=current.photoRequest;current.photoClickPending=YES;};
        self.transport.onEnd=^{[owner nextClip];};
        self.transport.onSeek=^{PSAudioController *current=owner;if(current && !current.cancelled){current.settledPhotoSeek++;current.photoSeekFinishedAt=NSProcessInfo.processInfo.systemUptime;}};
}
- (NSUInteger)currentIndex {
    return [self.timeline.clips indexOfObjectPassingTest:^BOOL(PSAudioClipViewModel *clip,NSUInteger index,BOOL *stop){return [clip.path isEqualToString:self.transport.path];}];
}
- (void)shutdown {
    if(self.cancelled)return;
    self.cancelled=YES;self.photoPending=NO;self.photoClickPending=NO;self.photoRecordPending=NO;self.contextPhotos.onSelect=nil;
    self.timeline.onSeek=nil;self.timeline.onToggle=nil;self.timeline.onRelativeSeek=nil;
    for(PSContextPhotoSlot *slot in self.contextPhotos.slots){slot.image=nil;slot.dateText=@"";slot.clockText=@"";slot.tooltip=@"";}[self.contextPhotos rebuildTips];
    [self.timer invalidate];self.timer=nil;[self.transport shutdown];self.timeline.clips=@[];
}
- (void)windowWillClose:(NSNotification *)notification { /* Window visibility is independent of the shared transport. */ }
- (void)dealloc {[self shutdown];}
- (void)tick:(NSTimer *)timer {
    [self.transport tick];
    NSUInteger index=[self currentIndex];if(index==NSNotFound){self.titleLabel.stringValue=self.periodLabel ?: @"Audio";self.detailLabel.stringValue=@"Keine Aufnahmen für diese Auswahl";for(NSButton *button in self.controlGroup.arrangedSubviews)button.enabled=NO;return;}
    PSAudioClipViewModel *clip=self.timeline.clips[index];
    if(clip.verification==1)[self.transport useVerifiedDuration:clip.duration forPath:clip.path];
    double local=CMTimeGetSeconds(self.transport.player.currentTime);if(!isfinite(local))local=0;
    double fraction=self.transport.duration>0?MIN(1,MAX(0,local/self.transport.duration)):0;
    if(!self.transport.loading && !self.transport.seeking)self.timeline.position=[self.timeline offset:index]+clip.weight*fraction;
    self.timeline.needsDisplay=YES;
    double civil=clip.recordedAt+local;
    self.titleLabel.stringValue=self.periodLabel ?: @"Audio";
    self.detailLabel.stringValue=self.transport.error.length?self.transport.error:[NSString stringWithFormat:@"%@  %@",PSAudioDateText(civil),PSAudioClockText(civil)];
    self.detailLabel.toolTip=clip.label;
    if(!isfinite(civil) && self.transport.hasStarted){
        self.contextPhotos.loading=YES;
        self.contextPhotos.statusText=clip.verification==2?@"Kein Aufnahmedatum verfügbar":@"Fotozeit wird geprüft …";
        self.contextPhotos.needsDisplay=YES;
    }
    BOOL first=!self.photosStarted && self.transport.hasStarted;
    BOOL pendingSeek=self.lastPhotoSeek!=self.settledPhotoSeek;
    BOOL changed=self.photosStarted && (![self.photoPath isEqualToString:clip.path] || self.photoRecordedAt!=clip.recordedAt) && !pendingSeek;
    BOOL settled=self.photosStarted && pendingSeek && NSProcessInfo.processInfo.systemUptime-self.photoSeekFinishedAt>=0.25;
    if(isfinite(civil) && !self.transport.loading && !self.transport.seeking && (first || changed || settled)){
        self.photosStarted=YES;self.photoPath=clip.path;self.photoRecordedAt=clip.recordedAt;self.lastPhotoSeek=self.settledPhotoSeek;
        self.photoCivil=civil;self.photoRequest++;self.photoPending=YES;
        self.contextPhotos.loading=YES;self.contextPhotos.statusText=@"Fotos werden gesucht …";self.contextPhotos.needsDisplay=YES;
    }
    self.playButton.image=PSAudioSymbolImage(self.transport.wantsPlay?PSAudioPause:PSAudioPlay);
    self.playButton.toolTip=self.transport.wantsPlay?@"Wiedergabe pausieren":@"Wiedergabe starten";
    [self.playButton setAccessibilityLabel:self.playButton.toolTip];
    self.recordButton.enabled=!self.contextPhotos.loading && self.contextPhotos.slots[2].image!=nil && !self.transport.loading && !self.transport.seeking && self.lastPhotoSeek==self.settledPhotoSeek;
    for(NSButton *button in self.controlGroup.arrangedSubviews)if(button!=self.recordButton)button.enabled=YES;
    self.playButton.enabled=!self.transport.loading && self.transport.error.length==0;
}
- (void)toggle:(id)sender {
    if(self.timeline.clips.count==0)return;
    self.transport.wantsPlay=!self.transport.wantsPlay;
    if(!self.transport.wantsPlay)[self.transport.player pause];
    else if(CMTimeGetSeconds(self.transport.player.currentTime)>=self.transport.duration-0.01 && [self currentIndex]+1==self.timeline.clips.count)[self seek:0];
    else [self.transport.player play];
    [self tick:nil];
}
- (void)recordCurrentPhoto:(id)sender {
    if(self.cancelled||self.contextPhotos.loading||!self.contextPhotos.slots[2].image||self.transport.loading||self.transport.seeking||self.lastPhotoSeek!=self.settledPhotoSeek)return;
    self.recordedPhotoRequest=self.photoRequest;self.photoRecordPending=YES;
    self.transport.wantsPlay=NO;[self.transport.player pause];[self tick:nil];
    [self.window orderOut:nil];
}
- (void)nextClip {
    NSUInteger index=[self currentIndex];
    if(index!=NSNotFound && index+1<self.timeline.clips.count){[self.transport openPath:self.timeline.clips[index+1].path fraction:0 offset:0];}
    else {self.transport.wantsPlay=NO;[self.transport.player pause];}
}
- (void)seek:(double)position {
    if(self.cancelled || !isfinite(position))return;
    double fraction=0;NSUInteger index=[self.timeline indexAtPosition:position fraction:&fraction];if(index==NSNotFound)return;
    self.timeline.position=MIN(self.timeline.total,MAX(0,position));self.timeline.needsDisplay=YES;
    NSString *path=self.timeline.clips[index].path;
    if([path isEqualToString:self.transport.path] && !self.transport.loading && self.transport.duration>0)
        [self.transport seekSeconds:self.transport.duration*fraction];
    else [self.transport openPath:path fraction:fraction offset:0];
}
- (void)seekRelative:(double)seconds {
    NSUInteger index=[self currentIndex];if(index==NSNotFound || self.transport.loading)return;
    double local=CMTimeGetSeconds(self.transport.player.currentTime);if(!isfinite(local))local=0;
    double target=local+seconds;
    if(target>=0 && target<=self.transport.duration){[self.transport seekSeconds:target];return;}
    // Use measured neighbours where available; unmeasured neighbours load their own duration.
    if(target<0){
        while(index>0){index--;PSAudioClipViewModel *clip=self.timeline.clips[index];
            if(clip.verification!=1){[self.transport openPath:clip.path fraction:1 offset:target];return;}
            double duration=clip.duration;target+=duration;
            if(target>=0){[self.transport openPath:clip.path fraction:0 offset:target];return;}
        }
        [self.transport openPath:self.timeline.clips[0].path fraction:0 offset:0];
    }else{
        target-=self.transport.duration;
        while(index+1<self.timeline.clips.count){index++;PSAudioClipViewModel *clip=self.timeline.clips[index];
            if(clip.verification!=1 || target<=clip.duration){[self.transport openPath:clip.path fraction:0 offset:target];return;}
            target-=clip.duration;
        }
        [self.transport openPath:self.timeline.clips.lastObject.path fraction:1 offset:0];
    }
}
- (void)back:(id)sender {[self seekRelative:-15];}
- (void)forward:(id)sender {[self seekRelative:15];}
- (void)volume:(NSSlider *)sender {self.transport.player.volume=sender.floatValue;}
@end
static NSTextField *PSAudioLabel(NSRect frame,CGFloat size){
    NSTextField *label=[NSTextField labelWithString:@""];label.frame=frame;label.font=[NSFont systemFontOfSize:size];label.textColor=PS_THEME_TEXT;
    label.alignment=NSTextAlignmentCenter;label.lineBreakMode=NSLineBreakByTruncatingMiddle;label.autoresizingMask=NSViewWidthSizable;return label;
}
static NSArray<PSAudioClipViewModel *> *PSAudioReadClips(const char **paths,const char **labels,const double *weights,const double *dates,const double *durations,const unsigned int *states,size_t count){
    if(!count)return nil;NSMutableArray *clips=[NSMutableArray arrayWithCapacity:count];NSMutableSet *seen=[NSMutableSet set];double total=0;
    for(size_t i=0;i<count;++i){
        if(!isfinite(weights[i]) || weights[i]<=0 || (!isfinite(dates[i]) && !isnan(dates[i])) || states[i]>2 || !isfinite(durations[i]) || durations[i]<0 || (states[i]==1 && durations[i]<=0))return nil;
        PSAudioClipViewModel *clip=[PSAudioClipViewModel new];clip.path=PSAudioPath(paths[i]);clip.label=[NSString stringWithUTF8String:labels[i]] ?: @"";
        if([seen containsObject:clip.path])return nil;[seen addObject:clip.path];clip.weight=weights[i];clip.recordedAt=dates[i];clip.actualRecordedAt=NAN;clip.verification=states[i];clip.duration=durations[i];[clips addObject:clip];total+=weights[i];
    }
    return isfinite(total)?clips:nil;
}
// App-local companion: main-window focus must not cover the audio controls.
// Hiding on app deactivation prevents floating above other applications.
static NSWindow *PSAudioCreateWindow(void) {
    NSWindow *window=[[NSWindow alloc] initWithContentRect:NSMakeRect(0,0,740,550) styleMask:NSWindowStyleMaskTitled|NSWindowStyleMaskClosable|NSWindowStyleMaskMiniaturizable|NSWindowStyleMaskResizable backing:NSBackingStoreBuffered defer:NO];
    window.backgroundColor=PS_THEME_SURFACE;
    window.level=NSFloatingWindowLevel;
    window.hidesOnDeactivate=YES;
    window.acceptsMouseMovedEvents=YES;
    window.appearance=[NSAppearance appearanceNamed:NSAppearanceNameDarkAqua];
    window.minSize=NSMakeSize(440,572);window.maxSize=NSMakeSize(1600,572);window.releasedWhenClosed=NO;
    return window;
}
int photosuite_audio_open_mode(const char **paths,const char **labels,const double *weights,const double *dates,const double *durations,const unsigned int *states,size_t count,const char *year,const char *selected,unsigned long long *session,int showWindow){
    if(![NSThread isMainThread])return 1;
    @autoreleasepool {
        NSArray *clips=PSAudioReadClips(paths,labels,weights,dates,durations,states,count);NSString *chosen=PSAudioPath(selected);
        if(!clips || [clips indexOfObjectPassingTest:^BOOL(PSAudioClipViewModel *clip,NSUInteger i,BOOL *stop){return [clip.path isEqualToString:chosen];}]==NSNotFound)return 2;
        showWindow=showWindow || (PSAudio && PSAudio.window.visible);
        NSRect previousFrame=PSAudio?PSAudio.window.frame:NSZeroRect;
        if(PSAudio){[PSAudio shutdown];[PSAudio close];}
        NSWindow *window=PSAudioCreateWindow();
        window.title=[NSString stringWithFormat:@"%@ · Audiowiedergabe",[NSString stringWithUTF8String:year]];
        PSAudioController *controller=[[PSAudioController alloc] initWithWindow:window];PSAudio=controller;window.delegate=controller;
        controller.periodLabel=[NSString stringWithUTF8String:year];controller.photoSession=++PSNextPhotoSession;*session=controller.photoSession;
        controller.transport=[PSAudioTransport new];controller.transport.player.volume=0.8;
        NSView *view=window.contentView;
        controller.titleLabel=PSAudioLabel(NSMakeRect(20,328,700,24),20);controller.titleLabel.stringValue=controller.periodLabel;[view addSubview:controller.titleLabel];
        controller.detailLabel=PSAudioLabel(NSMakeRect(20,302,700,22),16);[view addSubview:controller.detailLabel];
        controller.verificationLabel=PSAudioLabel(NSMakeRect(20,281,700,16),12);controller.verificationLabel.stringValue=@"";controller.verificationLabel.hidden=YES;[view addSubview:controller.verificationLabel];
        controller.timeline=[[PSAudioTimeline alloc] initWithFrame:NSMakeRect(12,235,716,58)];controller.sourceClips=clips;controller.timeline.clips=clips;controller.timeline.autoresizingMask=NSViewWidthSizable;[view addSubview:controller.timeline];
        controller.contextPhotos=[[PSContextPhotos alloc] initWithFrame:NSMakeRect(20,65,700,160)];controller.contextPhotos.autoresizingMask=NSViewWidthSizable;[view addSubview:controller.contextPhotos];
        controller.categoryControl=[NSSegmentedControl segmentedControlWithLabels:@[@"Tagebuch",@"Kommentare",@"Beide"] trackingMode:NSSegmentSwitchTrackingSelectOne target:controller action:@selector(categoryChanged:)];
        controller.categoryControl.selectedSegment=2;controller.categoryControl.frame=NSMakeRect(view.bounds.size.width-240,328,220,24);controller.categoryControl.autoresizingMask=NSViewMinXMargin;controller.categoryControl.font=[NSFont systemFontOfSize:12];controller.categoryControl.toolTip=@"Aufnahmen im Player filtern";[controller.categoryControl setAccessibilityLabel:@"Aufnahmeart"];[view addSubview:controller.categoryControl];
        controller.titleLabel.frame=NSMakeRect(20,328,170,24);controller.titleLabel.autoresizingMask=NSViewMaxXMargin;
        [controller connectControls];
        NSStackView *buttons=[NSStackView stackViewWithViews:@[]];buttons.orientation=NSUserInterfaceLayoutOrientationHorizontal;buttons.spacing=6;buttons.distribution=NSStackViewDistributionFillEqually;buttons.alignment=NSLayoutAttributeCenterY;
        buttons.frame=NSMakeRect((view.bounds.size.width-154)/2,22,154,34);buttons.translatesAutoresizingMaskIntoConstraints=NO;controller.controlGroup=buttons;
        NSArray *tips=@[@"15 Sekunden zurück",@"Wiedergabe pausieren",@"Kommentar zum mittleren Foto aufnehmen",@"15 Sekunden vorwärts"];
        PSAudioSymbol symbols[]={PSAudioBack,PSAudioPause,PSAudioRecord,PSAudioForward};SEL selectors[]={@selector(back:),@selector(toggle:),@selector(recordCurrentPhoto:),@selector(forward:)};
        for(NSUInteger i=0;i<4;++i){NSButton *button=[PSAudioButton buttonWithTitle:@"" target:controller action:selectors[i]];button.image=PSAudioSymbolImage(symbols[i]);button.imagePosition=NSImageOnly;button.imageScaling=NSImageScaleProportionallyDown;button.bordered=NO;button.toolTip=tips[i];[button setAccessibilityLabel:tips[i]];[buttons addArrangedSubview:button];if(i==1){controller.playButton=button;button.enabled=NO;}if(i==2){controller.recordButton=button;button.enabled=NO;}}
        [view addSubview:buttons];
        [NSLayoutConstraint activateConstraints:@[[buttons.centerXAnchor constraintEqualToAnchor:view.centerXAnchor],[buttons.bottomAnchor constraintEqualToAnchor:view.bottomAnchor constant:-202],[buttons.widthAnchor constraintEqualToConstant:154],[buttons.heightAnchor constraintEqualToConstant:34]]];
        for(NSView *button in buttons.arrangedSubviews){[NSLayoutConstraint activateConstraints:@[[button.widthAnchor constraintEqualToConstant:34],[button.heightAnchor constraintEqualToConstant:34]]];}
        NSSlider *volume=[NSSlider sliderWithValue:0.8 minValue:0 maxValue:1 target:controller action:@selector(volume:)];volume.frame=NSMakeRect(615,29,105,20);volume.autoresizingMask=NSViewMinXMargin;volume.toolTip=@"Lautstärke";[view addSubview:volume];
        // Upper player keeps its compact geometry; the file list occupies the lower area.
        for(NSView *child in [view.subviews copy])if(child!=buttons){NSRect frame=child.frame;frame.origin.y+=180;child.frame=frame;child.autoresizingMask|=NSViewMinYMargin;}
        NSBox *separator=[[NSBox alloc] initWithFrame:NSMakeRect(20,190,view.bounds.size.width-40,1)];separator.boxType=NSBoxSeparator;separator.autoresizingMask=NSViewWidthSizable;[view addSubview:separator];
        NSScrollView *scroll=[[NSScrollView alloc] initWithFrame:NSMakeRect(20,16,view.bounds.size.width-40,162)];scroll.hasVerticalScroller=YES;scroll.hasHorizontalScroller=YES;scroll.autoresizingMask=NSViewWidthSizable;
        controller.fileTable=[[NSTableView alloc] initWithFrame:scroll.bounds];controller.fileTable.dataSource=controller;controller.fileTable.target=controller;controller.fileTable.doubleAction=@selector(playSelectedFile:);controller.fileTable.rowHeight=22;controller.fileTable.backgroundColor=PS_THEME_SURFACE;
        NSArray *ids=@[@"file",@"reference",@"recorded"],*titles=@[@"Dateiname",@"Bezugsdatum",@"Aufnahmedatum"];
        for(NSUInteger i=0;i<ids.count;i++){NSTableColumn *column=[[NSTableColumn alloc] initWithIdentifier:ids[i]];column.title=titles[i];column.width=i==0?260:180;column.minWidth=i==0?140:170;[(NSTextFieldCell *)column.dataCell setTextColor:PS_THEME_TEXT];column.editable=NO;[controller.fileTable addTableColumn:column];}
        scroll.documentView=controller.fileTable;[view addSubview:scroll];
        if(NSIsEmptyRect(previousFrame))[window center];else [window setFrame:previousFrame display:NO];if(showWindow){[controller showWindow:nil];[window makeKeyAndOrderFront:nil];[window makeFirstResponder:controller.timeline];}
        controller.timer=[NSTimer scheduledTimerWithTimeInterval:0.1 target:controller selector:@selector(tick:) userInfo:nil repeats:YES];
        [controller.transport openPath:chosen fraction:0 offset:0];
        return 0;
    }
}
int photosuite_audio_open(const char **paths,const char **labels,const double *weights,const double *dates,const double *durations,const unsigned int *states,size_t count,const char *year,const char *selected,unsigned long long *session){return photosuite_audio_open_mode(paths,labels,weights,dates,durations,states,count,year,selected,session,1);}
int photosuite_audio_transport_state(unsigned long long session,double *fraction,double *civil){
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || PSAudio.photoSession!=session)return 0;
    [PSAudio tick:nil];NSUInteger index=[PSAudio currentIndex];
    *fraction=PSAudio.timeline.total>0?MIN(1,MAX(0,PSAudio.timeline.position/PSAudio.timeline.total)):0;
    *civil=NAN;if(index!=NSNotFound){double local=CMTimeGetSeconds(PSAudio.transport.player.currentTime);if(isfinite(local))*civil=PSAudio.timeline.clips[index].recordedAt+local;}
    return 1|(PSAudio.transport.wantsPlay?2:0)|((PSAudio.transport.loading || PSAudio.transport.seeking)?4:0)|(PSAudio.transport.error.length?8:0)|((int)PSAudio.category<<4);
}
int photosuite_audio_control(unsigned long long session,int command,double fraction){
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || PSAudio.photoSession!=session)return 1;
    if(command==0){[PSAudio toggle:nil];}
    else if(command==1){if(!isfinite(fraction))return 2;[PSAudio seek:MIN(1,MAX(0,fraction))*PSAudio.timeline.total];}
    else if(command==2){[PSAudio showWindow:nil];[PSAudio.window makeKeyAndOrderFront:nil];[PSAudio.window makeFirstResponder:PSAudio.timeline];}
    else if(command==3){[PSAudio shutdown];[PSAudio.window orderOut:nil];}
    else if(command==4){PSAudio.category=0;PSAudio.categoryControl.selectedSegment=2;[PSAudio applyCategory];if(!PSAudio.transport.wantsPlay)[PSAudio toggle:nil];}
    else if(command==5){
        if(!isfinite(fraction)||fraction<0||fraction>6||floor(fraction)!=fraction||((int)fraction&3)>2)return 2;
        BOOL playing=((int)fraction&4)!=0;PSAudio.category=(int)fraction&3;PSAudio.categoryControl.selectedSegment=PSAudio.category==0?2:PSAudio.category-1;
        PSAudio.wantsBeforeEmpty=playing;PSAudio.transport.wantsPlay=playing;[PSAudio applyCategory];
        if(!playing)[PSAudio.transport.player pause];[PSAudio tick:nil];
    }
    else return 2;return 0;
}
int photosuite_audio_state(unsigned long long session){
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || PSAudio.photoSession!=session)return 0;
    return 1|(PSAudio.transport.hasStarted?2:0)|(PSAudio.transport.error.length?4:0)|(PSAudio.timeline.clips.count==0?8:0);
}
int photosuite_audio_revise(unsigned long long session,const char **paths,const char **labels,const double *weights,const double *dates,const double *durations,const unsigned int *states,size_t count,const char *status){
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || PSAudio.photoSession!=session)return 1;
    NSArray *clips=PSAudioReadClips(paths,labels,weights,dates,durations,states,count);
    if(!clips || clips.count!=PSAudio.sourceClips.count)return 2;
    NSSet *before=[NSSet setWithArray:[PSAudio.sourceClips valueForKey:@"path"]],*after=[NSSet setWithArray:[clips valueForKey:@"path"]];
    if(![before isEqualToSet:after])return 2;
    NSMutableDictionary *masks=[NSMutableDictionary new];for(PSAudioClipViewModel *old in PSAudio.sourceClips)masks[old.path]=@(old.visibilityMask);
    for(PSAudioClipViewModel *clip in clips)clip.visibilityMask=[masks[clip.path] unsignedIntValue];
    PSAudio.sourceClips=clips;[PSAudio applyCategory];
    PSAudio.verificationLabel.toolTip=[NSString stringWithUTF8String:status] ?: @"";
    [PSAudio tick:nil];return 0;
}

int photosuite_audio_context_state(unsigned long long *session,unsigned long long *request,double *civil) {
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled)return 0;
    *session=PSAudio.photoSession;*request=PSAudio.photoRequest;*civil=PSAudio.photoCivil;
    int flags=1|(PSAudio.photoPending?2:0);PSAudio.photoPending=NO;return flags;
}
void photosuite_audio_context_slot(unsigned long long session,unsigned long long request,unsigned int slot,const unsigned char *png,size_t length,const char *tooltip,int state) {
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || PSAudio.photoSession!=session || PSAudio.photoRequest!=request || slot>=5)return;
    PSContextPhotos *view=PSAudio.contextPhotos;if(!view)return;
    PSContextPhotoSlot *item=view.slots[slot];
    if(tooltip)item.tooltip=[NSString stringWithUTF8String:tooltip] ?: @"";
    NSImage *image=nil;
    if(state==2 && png && length>0 && length<=8*1024*1024)image=[[NSImage alloc] initWithData:[NSData dataWithBytes:png length:length]];
    item.image=image;
    item.message=state==0?@"Kein Foto":state==1?@"Wird geladen":@"Nicht verfügbar";
    if(state==0 || state==1){item.dateText=@"";item.clockText=@"";}
    [view rebuildTips];view.needsDisplay=YES;
}
void photosuite_audio_context_status(unsigned long long session,unsigned long long request,const char *status) {
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || PSAudio.photoSession!=session || PSAudio.photoRequest!=request)return;
    PSAudio.contextPhotos.loading=NO;PSAudio.contextPhotos.statusText=[NSString stringWithUTF8String:status] ?: @"";PSAudio.contextPhotos.needsDisplay=YES;
}

void photosuite_audio_context_caption(unsigned long long session,unsigned long long request,unsigned int slot,const char *date,const char *clock) {
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || PSAudio.photoSession!=session || PSAudio.photoRequest!=request || slot>=5)return;
    PSContextPhotos *view=PSAudio.contextPhotos;if(!view)return;
    view.slots[slot].dateText=date ? ([NSString stringWithUTF8String:date] ?: @"") : @"";
    view.slots[slot].clockText=clock ? ([NSString stringWithUTF8String:clock] ?: @"") : @"";
    view.needsDisplay=YES;
}

int photosuite_audio_context_click(unsigned long long *session,unsigned long long *request,unsigned int *slot) {
    if(![NSThread isMainThread] || !PSAudio || PSAudio.cancelled || !PSAudio.photoClickPending)return 0;
    PSAudio.photoClickPending=NO;
    if(PSAudio.clickedPhotoRequest!=PSAudio.photoRequest)return 0;
    *session=PSAudio.photoSession;*request=PSAudio.clickedPhotoRequest;*slot=(unsigned int)PSAudio.clickedPhotoSlot;
    return 1;
}

int photosuite_audio_context_record(unsigned long long *session,unsigned long long *request,unsigned int *slot){
    if(![NSThread isMainThread]||!PSAudio||PSAudio.cancelled||!PSAudio.photoRecordPending)return 0;
    PSAudio.photoRecordPending=NO;
    if(PSAudio.recordedPhotoRequest!=PSAudio.photoRequest)return 0;
    *session=PSAudio.photoSession;*request=PSAudio.recordedPhotoRequest;*slot=2;return 1;
}

int photosuite_audio_visibility(unsigned long long session,const unsigned int *masks,size_t count){
    if(![NSThread isMainThread]||!PSAudio||PSAudio.cancelled||PSAudio.photoSession!=session)return 1;
    if(count!=PSAudio.sourceClips.count||(!masks&&count))return 2;
    for(NSUInteger i=0;i<count;i++)if((masks[i]&1)==0||(masks[i]&~7u)!=0)return 2;
    for(NSUInteger i=0;i<count;i++)PSAudio.sourceClips[i].visibilityMask=masks[i];
    [PSAudio applyCategory];return 0;
}

int photosuite_audio_recorded_dates(unsigned long long session,const double *dates,size_t count){
    if(![NSThread isMainThread]||!PSAudio||PSAudio.cancelled||PSAudio.photoSession!=session)return 1;
    if(count!=PSAudio.sourceClips.count||(!dates&&count))return 2;
    for(NSUInteger i=0;i<count;i++)if(isinf(dates[i]))return 2;
    for(NSUInteger i=0;i<count;i++)PSAudio.sourceClips[i].actualRecordedAt=dates[i];
    [PSAudio.fileTable reloadData];return 0;
}

int photosuite_audio_empty(unsigned long long session,const char *label){
    if(![NSThread isMainThread]||!PSAudio||PSAudio.cancelled||PSAudio.photoSession!=session||!label)return 1;
    PSAudio.periodLabel=[NSString stringWithUTF8String:label] ?: @"Audio";
    PSAudio.window.title=[PSAudio.periodLabel stringByAppendingString:@" · Audiowiedergabe"];
    PSAudio.sourceClips=@[];[PSAudio applyCategory];return 0;
}
