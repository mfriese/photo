// Real M4A finalization/preview/cleanup, using generated silence. Never opens a microphone.
#import "../native/comment_recorder.m"
#include <assert.h>
// Simulate AVFoundation's paused state (isRecording may be false). No input device.
@interface PSTestPausedOutput : AVCaptureAudioFileOutput
@property BOOL paused;
@property BOOL stopped;
@end
@implementation PSTestPausedOutput
- (BOOL)isRecording {return NO;}
- (BOOL)isRecordingPaused {return self.paused;}
- (void)resumeRecording {self.paused=NO;}
- (void)stopRecording {self.stopped=YES;}
@end
static void pump(double seconds){NSDate *end=[NSDate dateWithTimeIntervalSinceNow:seconds];while(end.timeIntervalSinceNow>0)[NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];}
static NSDictionary *snapshot(PSCommentRecorder *r){char *bytes=photosuite_comment_snapshot((__bridge void*)r);assert(bytes);NSData *data=[NSData dataWithBytes:bytes length:strlen(bytes)];free(bytes);return [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];}
int main(int argc,char **argv){@autoreleasepool{
 // Every measurement is preserved; the bounded window still spans ~32 seconds.
 PSCommentRecorder *envelope=[PSCommentRecorder new];
 for(int i=0;i<8;i++)[envelope appendWaveLevel:i==2?.8f:.1f];
 assert(envelope.wave.count==8 && fabsf(envelope.wave[2].floatValue-.8f)<.0001f);
 for(int i=0;i<PHOTOSUITE_COMMENT_WAVE_SAMPLES;i++)[envelope appendWaveLevel:.3f];
 assert(envelope.wave.count==960 && fabsf(envelope.wave.firstObject.floatValue-.3f)<.0001f);
 assert(argc==2);PSCommentRecorder *r=[PSCommentRecorder new];assert([snapshot(r)[@"phase"] isEqual:@"idle"]);
 r.identifier=@"12345678-1234-1234-1234-123456789abc";r.startedAt=@"2026-11-12T15:30:00+01:00";
 r.directory=[NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString] isDirectory:YES];assert([NSFileManager.defaultManager createDirectoryAtURL:r.directory withIntermediateDirectories:NO attributes:nil error:nil]);r.file=[r.directory URLByAppendingPathComponent:@"test.m4a"];
 AVURLAsset *source=[AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[1]]] options:nil];AVAssetExportSession *exporter=[[AVAssetExportSession alloc] initWithAsset:source presetName:AVAssetExportPresetAppleM4A];assert(exporter);exporter.outputURL=r.file;exporter.outputFileType=AVFileTypeAppleM4A;__block BOOL done=NO;[exporter exportAsynchronouslyWithCompletionHandler:^{done=YES;}];NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:10];while(!done&&deadline.timeIntervalSinceNow>0)pump(.01);assert(done&&exporter.status==AVAssetExportSessionStatusCompleted);
 dispatch_sync(r.queue,^{[r finishFile:r.file error:nil];});NSDictionary *ready=snapshot(r);assert([ready[@"phase"] isEqual:@"ready"]);assert([ready[@"seconds"] doubleValue]>.9);assert([ready[@"recorded_at"] isEqual:r.startedAt]);NSString *file=r.file.path;
 [r command:3];pump(.1);dispatch_sync(r.queue,^{});assert(r.preview);[r command:3];dispatch_sync(r.queue,^{});assert(!r.preview.isPlaying);assert([snapshot(r)[@"seconds"] doubleValue]>.9);
 [r command:4];dispatch_sync(r.queue,^{});assert([snapshot(r)[@"phase"] isEqual:@"idle"]);assert(![NSFileManager.defaultManager fileExistsAtPath:file]);
 // A bad output remains an explicit error, never a ready take.
 dispatch_sync(r.queue,^{r.cancelled=NO;[r finishFile:[NSURL fileURLWithPath:@"/nonexistent/photosuite-test.m4a"] error:nil];});assert([snapshot(r)[@"phase"] isEqual:@"error"]);[r command:4];dispatch_sync(r.queue,^{});
 PSTestPausedOutput *paused=[PSTestPausedOutput new];paused.paused=YES;r.output=paused;r.phase=@"paused";
 [r command:1];dispatch_sync(r.queue,^{});assert(!paused.paused);assert([snapshot(r)[@"phase"] isEqual:@"recording"]);
 paused.paused=YES;r.phase=@"paused";[r command:2];dispatch_sync(r.queue,^{});assert(paused.stopped);assert([snapshot(r)[@"phase"] isEqual:@"finishing"]);
 r.output=nil;[r command:4];dispatch_sync(r.queue,^{});
 puts("Native recorder OK: M4A finalize, validation, preview/pause and owned-temp cleanup; no microphone accessed.");
}return 0;}
