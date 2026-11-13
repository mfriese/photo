#import <AVFoundation/AVFoundation.h>
#import <Foundation/Foundation.h>
#import "photosuite.h"
#import "comment_edit.h"
#import <math.h>

// All capture commands run on one queue; snapshots are copied under the monitor.
@interface PSCommentRecorder : NSObject <AVCaptureFileOutputRecordingDelegate,AVAudioPlayerDelegate>
@property dispatch_queue_t queue;
@property AVCaptureSession *capture;
@property AVCaptureAudioFileOutput *output;
@property AVAudioPlayer *preview;
@property NSURL *directory;
@property NSURL *file;
@property NSURL *segment;
@property NSString *device;
@property NSString *finishPhase;
@property NSString *finishWarning;
@property double insertion;
@property double cursor;
@property double waveStart;
@property BOOL editFailed;
@property NSMutableArray<NSArray<NSNumber *> *> *envelope;
@property NSMutableArray<NSArray<NSNumber *> *> *segmentEnvelope;
@property AVAssetExportSession *exporter;
@property NSString *phase;
@property NSString *message;
@property NSString *identifier;
@property NSString *startedAt;
@property NSMutableArray<NSNumber *> *wave;
@property double seconds;
@property float level;
@property float peak;
@property NSUInteger generation;
@property BOOL cancelled;
@property dispatch_source_t meter;
@property PSCommentRecorder *keepAlive;
@property NSMutableArray *observers;
- (void)start:(NSString *)device;
- (void)command:(int)command;
- (void)finishFile:(NSURL *)url error:(NSError *)error;
@end
static NSString *PSRecordDate(void){NSDateFormatter *f=[NSDateFormatter new];f.locale=[[NSLocale alloc] initWithLocaleIdentifier:@"en_US_POSIX"];f.timeZone=NSTimeZone.localTimeZone;f.dateFormat=@"yyyy-MM-dd'T'HH:mm:ssXXX";return [f stringFromDate:NSDate.date];}
static char *PSRecordJSON(id object){NSData *data=[NSJSONSerialization dataWithJSONObject:object options:0 error:nil];if(!data)return NULL;char *bytes=malloc(data.length+1);if(!bytes)return NULL;memcpy(bytes,data.bytes,data.length);bytes[data.length]=0;return bytes;}
@implementation PSCommentRecorder
- (instancetype)init{if((self=[super init])){_queue=dispatch_queue_create("de.photosuite.comment.capture",DISPATCH_QUEUE_SERIAL);_phase=@"idle";_message=@"";_wave=[NSMutableArray new];_envelope=[NSMutableArray new];_segmentEnvelope=[NSMutableArray new];}return self;}
// Keep every 30-Hz envelope measurement; 960 points span approximately 32 seconds.
- (void)appendWaveLevel:(float)level {
    [self.wave addObject:@(level)];
    if(self.wave.count>PHOTOSUITE_COMMENT_WAVE_SAMPLES)[self.wave removeObjectAtIndex:0];
}
- (void)stopMeter {if(self.meter){dispatch_source_cancel(self.meter);self.meter=nil;}}
- (void)startMeter {
    if(self.meter)return;
    self.meter=dispatch_source_create(DISPATCH_SOURCE_TYPE_TIMER,0,0,self.queue);
    dispatch_source_set_timer(self.meter,DISPATCH_TIME_NOW,NSEC_PER_SEC/30,NSEC_PER_SEC/100);
    __weak PSCommentRecorder *weak=self;
    dispatch_source_set_event_handler(self.meter,^{PSCommentRecorder *s=weak;if(!s)return;
        float db=-160,peak=-160;
        if(s.preview.isPlaying){[s.preview updateMeters];db=[s.preview averagePowerForChannel:0];peak=[s.preview peakPowerForChannel:0];}
        else if(s.output.isRecording && !s.output.isRecordingPaused){for(AVCaptureConnection *c in s.output.connections){for(AVCaptureAudioChannel *channel in c.audioChannels){db=fmaxf(db,channel.averagePowerLevel);peak=fmaxf(peak,channel.peakHoldLevel);}}}
        double duration=CMTimeGetSeconds(s.output.recordedDuration);
        @synchronized(s){s.level=isfinite(db)?fminf(1,powf(10,db/20)):0;s.peak=isfinite(peak)?peak:-160;
            if([s.phase isEqualToString:@"recording"] && isfinite(duration)&&duration>=0){
                s.seconds=s.insertion+duration;s.cursor=s.seconds;s.waveStart=fmax(0,s.seconds-32.);
                [s appendWaveLevel:s.level];
                if(s.segmentEnvelope.count<108001)[s.segmentEnvelope addObject:@[@(s.cursor),@(s.level)]];
            }
            if(s.preview.isPlaying)s.cursor=s.preview.currentTime;}
    });dispatch_resume(self.meter);
}
- (void)removeObservers {for(id observer in self.observers){[NSNotificationCenter.defaultCenter removeObserver:observer];}self.observers=nil;}
- (void)deviceFailed:(NSString *)message {
    if(!self.capture || self.cancelled)return;
    @synchronized(self){self.message=message?:@"Mikrofonverbindung unterbrochen";}
    if(self.output.isRecording || self.output.isRecordingPaused){@synchronized(self){self.phase=@"finishing";}[self.output stopRecording];}
    else {[self.capture stopRunning];[self removeObservers];[self fail:message];self.keepAlive=nil;}
}
- (void)cleanup {
    [self removeObservers];
    [self.exporter cancelExport];self.exporter=nil;
    [self stopMeter];[self.preview stop];self.preview=nil;
    if(self.directory){NSError *error=nil;if(![[NSFileManager defaultManager] removeItemAtURL:self.directory error:&error] && error.code!=NSFileNoSuchFileError){@synchronized(self){self.message=[NSString stringWithFormat:@"Temporäre Aufnahme konnte nicht entfernt werden: %@",error.localizedDescription];}}}
    self.directory=nil;self.file=nil;self.segment=nil;
}
- (void)fail:(NSString *)message {if(self.file){[self restoreMaster:message];return;}@synchronized(self){self.phase=@"error";self.message=message?:@"Aufnahme fehlgeschlagen";}[self stopMeter];}
- (void)beginDevice:(NSString *)device generation:(NSUInteger)generation {
    if(self.cancelled||self.generation!=generation)return;
    NSError *error=nil;
    AVCaptureDevice *inputDevice=device.length?[AVCaptureDevice deviceWithUniqueID:device]:[AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeAudio];
    if(!inputDevice){[self fail:@"Das ausgewählte Mikrofon ist nicht verfügbar."];return;}
    AVCaptureDeviceInput *input=[AVCaptureDeviceInput deviceInputWithDevice:inputDevice error:&error];
    if(!input){[self fail:error.localizedDescription];return;}
    self.capture=[AVCaptureSession new];self.output=[AVCaptureAudioFileOutput new];
    if(![self.capture canAddInput:input]||![self.capture canAddOutput:self.output]){[self fail:@"Mikrofon und Audioausgabe konnten nicht verbunden werden."];return;}
    [self.capture addInput:input];[self.capture addOutput:self.output];
    self.output.audioSettings=@{AVFormatIDKey:@(kAudioFormatMPEG4AAC),AVSampleRateKey:@48000,AVNumberOfChannelsKey:@1,AVEncoderBitRateKey:@96000};
    self.output.maxRecordedDuration=CMTimeMake(60*60,1);self.output.minFreeDiskSpaceLimit=20*1024*1024;
    self.output.maxRecordedDuration=CMTimeMakeWithSeconds(fmax(.01,3600-self.insertion),48000);
    if(!self.directory){
        self.directory=[NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:[@"PhotoSuiteRecording-" stringByAppendingString:self.identifier]] isDirectory:YES];
        if(![[NSFileManager defaultManager] createDirectoryAtURL:self.directory withIntermediateDirectories:NO attributes:@{NSFilePosixPermissions:@0700} error:&error]){[self fail:error.localizedDescription];return;}
    }
    self.segment=[self.directory URLByAppendingPathComponent:[NSUUID.UUID.UUIDString stringByAppendingString:@".m4a"]];
    [self.segmentEnvelope removeAllObjects];
    self.keepAlive=self;
    __weak PSCommentRecorder *weak=self;
    self.observers=[NSMutableArray new];
    [self.observers addObject:[NSNotificationCenter.defaultCenter addObserverForName:AVCaptureSessionRuntimeErrorNotification object:self.capture queue:nil usingBlock:^(NSNotification *note){PSCommentRecorder *s=weak;if(s)dispatch_async(s.queue,^{if(s.generation==generation)[s deviceFailed:[note.userInfo[AVCaptureSessionErrorKey] localizedDescription]];});}]];
    [self.observers addObject:[NSNotificationCenter.defaultCenter addObserverForName:AVCaptureDeviceWasDisconnectedNotification object:inputDevice queue:nil usingBlock:^(NSNotification *note){PSCommentRecorder *s=weak;if(s)dispatch_async(s.queue,^{if(s.generation==generation)[s deviceFailed:@"Das Mikrofon wurde getrennt. Die bisherige Aufnahme wird abgeschlossen."];});}]];
    [self.capture startRunning];
    if(self.cancelled||self.generation!=generation){[self.capture stopRunning];[self cleanup];self.keepAlive=nil;return;}
    if(!self.startedAt)self.startedAt=PSRecordDate();
    AVMutableMetadataItem *date=[AVMutableMetadataItem metadataItem];date.keySpace=AVMetadataKeySpaceQuickTimeMetadata;date.key=AVMetadataQuickTimeMetadataKeyCreationDate;date.value=self.startedAt;self.output.metadata=@[date];
    [self.output startRecordingToOutputFileURL:self.segment outputFileType:AVFileTypeAppleM4A recordingDelegate:self];
    [self startMeter];
}
- (void)start:(NSString *)device {
    dispatch_async(self.queue,^{@autoreleasepool{
        if(![self.phase isEqualToString:@"idle"]&&![self.phase isEqualToString:@"ready"]&&![self.phase isEqualToString:@"error"]){@synchronized(self){self.message=@"Vorherige Aufnahme wird noch abgeschlossen. Bitte kurz warten.";}return;}
        @synchronized(self){self.phase=@"preparing";}
        [self cleanup];self.device=device;self.editFailed=NO;self.insertion=0;self.cursor=0;self.waveStart=0;[self.envelope removeAllObjects];self.finishPhase=@"ready";self.cancelled=NO;self.identifier=NSUUID.UUID.UUIDString.lowercaseString;self.startedAt=nil;NSUInteger generation=++self.generation;
        @synchronized(self){self.phase=@"preparing";self.message=@"";self.seconds=0;self.level=0;self.peak=-160;[self.wave removeAllObjects];}
        AVAuthorizationStatus status=[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
        if(status==AVAuthorizationStatusAuthorized){[self beginDevice:device generation:generation];}
        else if(status==AVAuthorizationStatusNotDetermined){[AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted){dispatch_async(self.queue,^{if(self.cancelled||self.generation!=generation)return;if(granted)[self beginDevice:device generation:generation];else[self fail:@"Mikrofonzugriff verweigert. Bitte in den macOS-Datenschutzeinstellungen erlauben."];});}];}
        else {[self fail:@"Mikrofonzugriff nicht erlaubt. Bitte in Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon erlauben."];}
    }});
}
- (void)command:(int)command {
    dispatch_async(self.queue,^{@autoreleasepool{
        if(command==0 && self.output.isRecording){self.finishPhase=@"paused";@synchronized(self){self.phase=@"finishing";self.message=@"";}[self.output stopRecording];}
        else if(command==1 && [self.phase isEqualToString:@"paused"]){
            if(self.preview.isPlaying)self.cursor=self.preview.currentTime;[self.preview stop];self.preview=nil;self.editFailed=NO;self.insertion=self.cursor;self.finishPhase=@"paused";
            @synchronized(self){self.phase=@"preparing";self.message=@"";}
            [self prepareRollingWave];++self.generation;[self beginDevice:self.device generation:self.generation];
        }
        else if(command==2 && (self.output.isRecording || self.output.isRecordingPaused)){
            self.finishPhase=@"ready";@synchronized(self){self.phase=@"finishing";self.message=@"";}[self.output stopRecording];
        }
        else if(command==2 && [self.phase isEqualToString:@"paused"]){@synchronized(self){[self.preview pause];self.editFailed=NO;self.phase=@"ready";self.message=@"";}}
        else if(command==3 && ([self.phase isEqualToString:@"ready"]||[self.phase isEqualToString:@"paused"])){[self togglePreview];}
        else if(command==4){self.cancelled=YES;++self.generation;[self.preview stop];
            if(self.output.isRecording || self.output.isRecordingPaused){@synchronized(self){self.phase=@"finishing";}[self.output stopRecording];}
            else {[self.capture stopRunning];self.capture=nil;self.output=nil;[self cleanup];@synchronized(self){self.phase=@"idle";self.seconds=0;self.message=@"";}self.keepAlive=nil;}
        }
    }});
}
- (void)captureOutput:(AVCaptureFileOutput *)output didStartRecordingToOutputFileAtURL:(NSURL *)url fromConnections:(NSArray<AVCaptureConnection *> *)connections {
    dispatch_async(self.queue,^{if(self.cancelled)return;@synchronized(self){self.phase=@"recording";self.message=@"";}});
}
- (void)prepareRollingWave {
    @synchronized(self){[self.wave removeAllObjects];
        double from=fmax(0,self.insertion-32.);self.waveStart=from;
        for(NSArray *point in self.envelope){double t=[point[0] doubleValue];if(t>=from&&t<self.insertion)[self appendWaveLevel:[point[1] floatValue]];}}
}
- (void)rebuildWave {
    @synchronized(self){[self.wave removeAllObjects];for(NSUInteger i=0;i<PHOTOSUITE_COMMENT_WAVE_SAMPLES;i++)[self.wave addObject:@0];
        for(NSArray *point in self.envelope){double relative=[point[0] doubleValue]-self.waveStart;NSUInteger i=relative>=0?(NSUInteger)(relative/32.*PHOTOSUITE_COMMENT_WAVE_SAMPLES):NSUIntegerMax;if(i<self.wave.count)self.wave[i]=@(fmaxf(self.wave[i].floatValue,[point[1] floatValue]));}}
}
- (void)seek:(double)position {
    dispatch_async(self.queue,^{if(!isfinite(position)||(![self.phase isEqualToString:@"paused"]&&![self.phase isEqualToString:@"ready"]))return;
        @synchronized(self){self.cursor=fmin(self.seconds,fmax(0,position));self.preview.currentTime=self.cursor;}
    });
}
- (void)togglePreview {
    if(self.preview.isPlaying){[self.preview pause];@synchronized(self){self.cursor=self.preview.currentTime;}return;}
    NSError *error=nil;if(!self.preview){self.preview=[[AVAudioPlayer alloc] initWithContentsOfURL:self.file error:&error];self.preview.delegate=self;self.preview.meteringEnabled=YES;}
    if(!self.preview){self.message=error.localizedDescription?:@"Probehören nicht möglich";return;}
    if(self.cursor>=self.seconds-.001)self.cursor=0;
    self.preview.currentTime=self.cursor;
    if(![self.preview play]){self.message=@"Probehören nicht möglich";return;}[self startMeter];
}
- (void)restoreMaster:(NSString *)message {
    [self.capture stopRunning];[self stopMeter];[self removeObservers];self.capture=nil;self.output=nil;self.keepAlive=nil;
    double duration=CMTimeGetSeconds([AVURLAsset URLAssetWithURL:self.file options:nil].duration);
    @synchronized(self){self.seconds=duration;self.cursor=fmin(fmax(0,self.insertion),duration);self.phase=@"paused";self.editFailed=YES;self.message=message?:@"Neuer Abschnitt nicht übernommen; bisherige Aufnahme bleibt erhalten.";}
    [self rebuildWave];
}
- (void)acceptFile:(NSURL *)url duration:(double)duration {
    NSMutableArray *points=[NSMutableArray new];
    for(NSArray *point in self.envelope)if([point[0] doubleValue]<self.insertion)[points addObject:point];
    [points addObjectsFromArray:self.segmentEnvelope];self.envelope=points;
    NSURL *previous=self.file,*segment=self.segment;
    @synchronized(self){self.file=url;self.editFailed=NO;self.seconds=duration;self.cursor=duration;self.phase=self.finishPhase?:@"ready";self.message=self.finishWarning?:@"";self.level=0;}
    self.capture=nil;self.output=nil;self.keepAlive=nil;
    for(NSURL *old in @[previous?:url,segment?:url]){if(![old isEqual:url]&&[old.URLByDeletingLastPathComponent isEqual:self.directory])[[NSFileManager defaultManager] removeItemAtURL:old error:nil];}
}
- (void)finishFile:(NSURL *)url error:(NSError *)error {
    [self.capture stopRunning];[self stopMeter];[self removeObservers];self.capture=nil;self.output=nil;
    self.finishWarning=error.localizedDescription?:@"";
    if(self.cancelled){self.capture=nil;self.output=nil;[self cleanup];@synchronized(self){self.phase=@"idle";self.message=@"";}self.keepAlive=nil;return;}
    AVURLAsset *asset=[AVURLAsset URLAssetWithURL:url options:nil];double duration=CMTimeGetSeconds(asset.duration);
    if(!isfinite(duration)||duration<=0||![asset tracksWithMediaType:AVMediaTypeAudio].count||[asset tracksWithMediaType:AVMediaTypeVideo].count){
        if(self.file){[self restoreMaster:@"Neuer Abschnitt ungültig; bisherige Aufnahme bleibt erhalten."];}
        else {[self fail:error.localizedDescription?:@"Keine gültige Audioaufnahme vorhanden."];self.keepAlive=nil;}return;
    }
    if(!self.file || [self.file isEqual:url]){[self acceptFile:url duration:duration];return;}
    NSURL *candidate=[self.directory URLByAppendingPathComponent:[NSUUID.UUID.UUIDString stringByAppendingString:@".m4a"]];
    NSError *failure=nil;self.exporter=PSCommentReplacement(self.file,self.insertion,url,candidate,self.startedAt,&failure);
    if(!self.exporter){[self restoreMaster:failure.localizedDescription?:@"Abschnitt konnte nicht eingefügt werden; bisherige Aufnahme bleibt erhalten."];return;}
    NSUInteger generation=self.generation;AVAssetExportSession *exporter=self.exporter;
    [exporter exportAsynchronouslyWithCompletionHandler:^{dispatch_async(self.queue,^{
        if(self.cancelled||self.generation!=generation)return;
        self.exporter=nil;self.capture=nil;self.output=nil;
        double total=CMTimeGetSeconds([AVURLAsset URLAssetWithURL:candidate options:nil].duration);
        if(exporter.status==AVAssetExportSessionStatusCompleted&&isfinite(total)&&total>0){[self acceptFile:candidate duration:total];}
        else {[self restoreMaster:exporter.error.localizedDescription?:@"Einfügen fehlgeschlagen; bisherige Aufnahme bleibt erhalten."];}
    });}];
}
- (void)captureOutput:(AVCaptureFileOutput *)output didFinishRecordingToOutputFileAtURL:(NSURL *)url fromConnections:(NSArray<AVCaptureConnection *> *)connections error:(NSError *)error {dispatch_async(self.queue,^{[self finishFile:url error:error];});}
- (void)dealloc {[self removeObservers];[self stopMeter];}
- (void)audioPlayerDidFinishPlaying:(AVAudioPlayer *)player successfully:(BOOL)flag {dispatch_async(self.queue,^{@synchronized(self){self.cursor=self.seconds;}[self stopMeter];});}
@end
void *photosuite_comment_recorder_create(void){@autoreleasepool{return (__bridge_retained void *)[PSCommentRecorder new];}}
void photosuite_comment_recorder_destroy(void *pointer){if(!pointer)return;PSCommentRecorder *recorder=(__bridge_transfer PSCommentRecorder *)pointer;[recorder command:4];}
char *photosuite_comment_devices(void){@autoreleasepool{
    NSMutableArray *devices=[NSMutableArray new];AVCaptureDevice *standard=[AVCaptureDevice defaultDeviceWithMediaType:AVMediaTypeAudio];
    for(AVCaptureDevice *device in [AVCaptureDevice devicesWithMediaType:AVMediaTypeAudio]){[devices addObject:@{@"id":device.uniqueID,@"name":device.localizedName,@"is_default":@([device.uniqueID isEqualToString:standard.uniqueID])}];}
    return PSRecordJSON(devices);
}}
int photosuite_comment_start(void *pointer,const char *device){@autoreleasepool{PSCommentRecorder *r=(__bridge PSCommentRecorder *)pointer;@synchronized(r){if(![r.phase isEqualToString:@"idle"]&&![r.phase isEqualToString:@"ready"]&&![r.phase isEqualToString:@"error"])return 1;}[r start:device?[NSString stringWithUTF8String:device]:@""];return 0;}}
void photosuite_comment_command(void *pointer,int command){[(__bridge PSCommentRecorder *)pointer command:command];}
char *photosuite_comment_snapshot(void *pointer){@autoreleasepool{PSCommentRecorder *r=(__bridge PSCommentRecorder *)pointer;@synchronized(r){
    return PSRecordJSON(@{@"phase":r.phase?:@"idle",@"message":r.message?:@"",@"seconds":@(r.seconds),@"position":@(r.cursor),@"edit_failed":@(r.editFailed),@"wave_start":@(r.waveStart),@"wave_span":@32.,@"level":@(r.level),@"peak":@(r.peak),@"wave":r.wave.copy?:@[],@"playing":@(r.preview.isPlaying),@"id":r.identifier?:@"",@"recorded_at":r.startedAt?:@"",@"path":[r.phase isEqualToString:@"ready"]?r.file.path?:@"":@""});
}}}

void photosuite_comment_seek(void *pointer,double seconds){[(__bridge PSCommentRecorder *)pointer seek:seconds];}
