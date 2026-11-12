#import <AVFoundation/AVFoundation.h>
#import <Foundation/Foundation.h>
#import "photosuite.h"
#import <math.h>

// All capture commands run on one queue; snapshots are copied under the monitor.
@interface PSCommentRecorder : NSObject <AVCaptureFileOutputRecordingDelegate,AVAudioPlayerDelegate>
@property dispatch_queue_t queue;
@property AVCaptureSession *capture;
@property AVCaptureAudioFileOutput *output;
@property AVAudioPlayer *preview;
@property NSURL *directory;
@property NSURL *file;
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
- (instancetype)init{if((self=[super init])){_queue=dispatch_queue_create("de.photosuite.comment.capture",DISPATCH_QUEUE_SERIAL);_phase=@"idle";_message=@"";_wave=[NSMutableArray new];}return self;}
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
            if(s.output && isfinite(duration)&&duration>=0)s.seconds=duration;
            if([s.phase isEqualToString:@"recording"]||s.preview.isPlaying){[s.wave addObject:@(s.level)];if(s.wave.count>120)[s.wave removeObjectAtIndex:0];}}
    });dispatch_resume(self.meter);
}
- (void)removeObservers {for(id observer in self.observers){[NSNotificationCenter.defaultCenter removeObserver:observer];}self.observers=nil;}
- (void)deviceFailed:(NSString *)message {
    if(!self.capture || self.cancelled)return;
    @synchronized(self){self.message=message?:@"Mikrofonverbindung unterbrochen";}
    if(self.output.isRecording){@synchronized(self){self.phase=@"finishing";}[self.output stopRecording];}
    else {[self.capture stopRunning];[self removeObservers];[self fail:message];self.keepAlive=nil;}
}
- (void)cleanup {
    [self removeObservers];
    [self stopMeter];[self.preview stop];self.preview=nil;
    if(self.directory){NSError *error=nil;if(![[NSFileManager defaultManager] removeItemAtURL:self.directory error:&error] && error.code!=NSFileNoSuchFileError){@synchronized(self){self.message=[NSString stringWithFormat:@"Temporäre Aufnahme konnte nicht entfernt werden: %@",error.localizedDescription];}}}
    self.directory=nil;self.file=nil;
}
- (void)fail:(NSString *)message {@synchronized(self){self.phase=@"error";self.message=message?:@"Aufnahme fehlgeschlagen";}[self stopMeter];}
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
    self.directory=[NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:[@"PhotoSuiteRecording-" stringByAppendingString:self.identifier]] isDirectory:YES];
    if(![[NSFileManager defaultManager] createDirectoryAtURL:self.directory withIntermediateDirectories:NO attributes:@{NSFilePosixPermissions:@0700} error:&error]){[self fail:error.localizedDescription];return;}
    self.file=[self.directory URLByAppendingPathComponent:@"recording.m4a"];
    self.keepAlive=self;
    __weak PSCommentRecorder *weak=self;
    self.observers=[NSMutableArray new];
    [self.observers addObject:[NSNotificationCenter.defaultCenter addObserverForName:AVCaptureSessionRuntimeErrorNotification object:self.capture queue:nil usingBlock:^(NSNotification *note){PSCommentRecorder *s=weak;if(s)dispatch_async(s.queue,^{if(s.generation==generation)[s deviceFailed:[note.userInfo[AVCaptureSessionErrorKey] localizedDescription]];});}]];
    [self.observers addObject:[NSNotificationCenter.defaultCenter addObserverForName:AVCaptureDeviceWasDisconnectedNotification object:inputDevice queue:nil usingBlock:^(NSNotification *note){PSCommentRecorder *s=weak;if(s)dispatch_async(s.queue,^{if(s.generation==generation)[s deviceFailed:@"Das Mikrofon wurde getrennt. Die bisherige Aufnahme wird abgeschlossen."];});}]];
    [self.capture startRunning];
    if(self.cancelled||self.generation!=generation){[self.capture stopRunning];[self cleanup];self.keepAlive=nil;return;}
    self.startedAt=PSRecordDate();
    AVMutableMetadataItem *date=[AVMutableMetadataItem metadataItem];date.keySpace=AVMetadataKeySpaceQuickTimeMetadata;date.key=AVMetadataQuickTimeMetadataKeyCreationDate;date.value=self.startedAt;self.output.metadata=@[date];
    [self.output startRecordingToOutputFileURL:self.file outputFileType:AVFileTypeAppleM4A recordingDelegate:self];
    [self startMeter];
}
- (void)start:(NSString *)device {
    dispatch_async(self.queue,^{@autoreleasepool{
        if(![self.phase isEqualToString:@"idle"]&&![self.phase isEqualToString:@"ready"]&&![self.phase isEqualToString:@"error"]){@synchronized(self){self.message=@"Vorherige Aufnahme wird noch abgeschlossen. Bitte kurz warten.";}return;}
        @synchronized(self){self.phase=@"preparing";}
        [self cleanup];self.cancelled=NO;self.identifier=NSUUID.UUID.UUIDString.lowercaseString;self.startedAt=nil;NSUInteger generation=++self.generation;
        @synchronized(self){self.phase=@"preparing";self.message=@"Mikrofon wird vorbereitet …";self.seconds=0;self.level=0;self.peak=-160;[self.wave removeAllObjects];}
        AVAuthorizationStatus status=[AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
        if(status==AVAuthorizationStatusAuthorized){[self beginDevice:device generation:generation];}
        else if(status==AVAuthorizationStatusNotDetermined){[AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL granted){dispatch_async(self.queue,^{if(self.cancelled||self.generation!=generation)return;if(granted)[self beginDevice:device generation:generation];else[self fail:@"Mikrofonzugriff verweigert. Bitte in den macOS-Datenschutzeinstellungen erlauben."];});}];}
        else {[self fail:@"Mikrofonzugriff nicht erlaubt. Bitte in Systemeinstellungen → Datenschutz & Sicherheit → Mikrofon erlauben."];}
    }});
}
- (void)command:(int)command {
    dispatch_async(self.queue,^{@autoreleasepool{
        if(command==0 && self.output.isRecording){[self.output pauseRecording];@synchronized(self){self.phase=@"paused";}}
        else if(command==1 && self.output.isRecordingPaused){[self.output resumeRecording];@synchronized(self){self.phase=@"recording";}}
        else if(command==2 && self.output.isRecording){@synchronized(self){self.phase=@"finishing";self.message=@"Aufnahme wird abgeschlossen …";}[self.output stopRecording];}
        else if(command==3 && [self.phase isEqualToString:@"ready"]){
            if(self.preview.isPlaying){[self.preview pause];}
            else {NSError *error=nil;if(!self.preview){self.preview=[[AVAudioPlayer alloc] initWithContentsOfURL:self.file error:&error];self.preview.delegate=self;self.preview.meteringEnabled=YES;}
                if(!self.preview||![self.preview play]){@synchronized(self){self.message=error.localizedDescription?:@"Probehören nicht möglich";}return;}[self startMeter];}
        }
        else if(command==4){self.cancelled=YES;++self.generation;[self.preview stop];
            if(self.output.isRecording){@synchronized(self){self.phase=@"finishing";}[self.output stopRecording];}
            else {[self.capture stopRunning];self.capture=nil;self.output=nil;[self cleanup];@synchronized(self){self.phase=@"idle";self.seconds=0;self.message=@"";}self.keepAlive=nil;}
        }
    }});
}
- (void)captureOutput:(AVCaptureFileOutput *)output didStartRecordingToOutputFileAtURL:(NSURL *)url fromConnections:(NSArray<AVCaptureConnection *> *)connections {
    dispatch_async(self.queue,^{if(self.cancelled)return;@synchronized(self){self.phase=@"recording";self.message=@"";}});
}
- (void)finishFile:(NSURL *)url error:(NSError *)error {
    [self.capture stopRunning];[self stopMeter];[self removeObservers];
    if(self.cancelled){self.capture=nil;self.output=nil;[self cleanup];@synchronized(self){self.phase=@"idle";self.message=@"";}self.keepAlive=nil;return;}
    AVURLAsset *asset=[AVURLAsset URLAssetWithURL:url options:nil];double duration=CMTimeGetSeconds(asset.duration);
    if(!isfinite(duration)||duration<=0||![asset tracksWithMediaType:AVMediaTypeAudio].count||[asset tracksWithMediaType:AVMediaTypeVideo].count){[self fail:error.localizedDescription?:@"Keine gültige Audioaufnahme vorhanden."];self.keepAlive=nil;return;}
    @synchronized(self){self.phase=@"ready";self.seconds=duration;self.message=error.localizedDescription?:([self.message isEqualToString:@"Aufnahme wird abgeschlossen …"]?@"":self.message);self.level=0;}
    self.capture=nil;self.output=nil;self.keepAlive=nil;
}
- (void)captureOutput:(AVCaptureFileOutput *)output didFinishRecordingToOutputFileAtURL:(NSURL *)url fromConnections:(NSArray<AVCaptureConnection *> *)connections error:(NSError *)error {dispatch_async(self.queue,^{[self finishFile:url error:error];});}
- (void)dealloc {[self removeObservers];[self stopMeter];}
- (void)audioPlayerDidFinishPlaying:(AVAudioPlayer *)player successfully:(BOOL)flag {dispatch_async(self.queue,^{[self stopMeter];});}
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
    return PSRecordJSON(@{@"phase":r.phase?:@"idle",@"message":r.message?:@"",@"seconds":@(r.seconds),@"level":@(r.level),@"peak":@(r.peak),@"wave":r.wave.copy?:@[],@"playing":@(r.preview.isPlaying),@"id":r.identifier?:@"",@"recorded_at":r.startedAt?:@"",@"path":[r.phase isEqualToString:@"ready"]?r.file.path?:@"":@""});
}}}
