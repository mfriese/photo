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
@interface PSTestResumeRecorder : PSCommentRecorder
@property BOOL resumed;
@end
@implementation PSTestResumeRecorder
- (void)beginDevice:(NSString *)device generation:(NSUInteger)generation {self.resumed=YES;self.phase=@"recording";}
@end
static void pump(double seconds){NSDate *end=[NSDate dateWithTimeIntervalSinceNow:seconds];while(end.timeIntervalSinceNow>0)[NSRunLoop.currentRunLoop runUntilDate:[NSDate dateWithTimeIntervalSinceNow:0.01]];}
static NSDictionary *snapshot(PSCommentRecorder *r){char *bytes=photosuite_comment_snapshot((__bridge void*)r);assert(bytes);NSData *data=[NSData dataWithBytes:bytes length:strlen(bytes)];free(bytes);return [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];}
static double frequency(NSURL *url,double from){
 AVURLAsset *asset=[AVURLAsset URLAssetWithURL:url options:nil];NSError *error=nil;AVAssetReader *reader=[[AVAssetReader alloc] initWithAsset:asset error:&error];assert(reader);
 reader.timeRange=CMTimeRangeMake(CMTimeMakeWithSeconds(from,48000),CMTimeMakeWithSeconds(.1,48000));
 AVAssetReaderTrackOutput *output=[[AVAssetReaderTrackOutput alloc] initWithTrack:[asset tracksWithMediaType:AVMediaTypeAudio].firstObject outputSettings:@{AVFormatIDKey:@(kAudioFormatLinearPCM),AVLinearPCMBitDepthKey:@16,AVLinearPCMIsFloatKey:@NO,AVLinearPCMIsBigEndianKey:@NO,AVLinearPCMIsNonInterleaved:@NO}];
 [reader addOutput:output];assert([reader startReading]);NSMutableData *data=[NSMutableData new];double rate=0;
 CMSampleBufferRef sample;while((sample=[output copyNextSampleBuffer])){const AudioStreamBasicDescription *format=CMAudioFormatDescriptionGetStreamBasicDescription(CMSampleBufferGetFormatDescription(sample));rate=format->mSampleRate;assert(format->mChannelsPerFrame==1);CMBlockBufferRef buffer=CMSampleBufferGetDataBuffer(sample);size_t length=CMBlockBufferGetDataLength(buffer),offset=data.length;[data increaseLengthBy:length];assert(CMBlockBufferCopyDataBytes(buffer,0,length,(char*)data.mutableBytes+offset)==kCMBlockBufferNoErr);CFRelease(sample);}
 assert(data.length>20&&rate>0);const int16_t *samples=data.bytes;NSUInteger count=data.length/2,crossings=0;for(NSUInteger i=1;i<count;i++)if((samples[i-1]<0&&samples[i]>=0)||(samples[i-1]>=0&&samples[i]<0))crossings++;
 return crossings*rate/(2.*count);
}
int main(int argc,char **argv){@autoreleasepool{
 // Every measurement is preserved; the bounded window still spans ~32 seconds.
 PSCommentRecorder *envelope=[PSCommentRecorder new];
 for(int i=0;i<8;i++)[envelope appendWaveLevel:i==2?.8f:.1f];
 assert(envelope.wave.count==8 && fabsf(envelope.wave[2].floatValue-.8f)<.0001f);
 for(int i=0;i<PHOTOSUITE_COMMENT_WAVE_SAMPLES;i++)[envelope appendWaveLevel:.3f];
 assert(envelope.wave.count==960 && fabsf(envelope.wave.firstObject.floatValue-.3f)<.0001f);
 assert(argc==3);PSCommentRecorder *r=[PSCommentRecorder new];assert([snapshot(r)[@"phase"] isEqual:@"idle"]);
 r.identifier=@"12345678-1234-1234-1234-123456789abc";r.startedAt=@"2026-11-12T15:30:00+01:00";
 r.directory=[NSURL fileURLWithPath:[NSTemporaryDirectory() stringByAppendingPathComponent:NSUUID.UUID.UUIDString] isDirectory:YES];assert([NSFileManager.defaultManager createDirectoryAtURL:r.directory withIntermediateDirectories:NO attributes:nil error:nil]);r.file=[r.directory URLByAppendingPathComponent:@"test.m4a"];
 AVURLAsset *source=[AVURLAsset URLAssetWithURL:[NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[1]]] options:nil];AVAssetExportSession *exporter=[[AVAssetExportSession alloc] initWithAsset:source presetName:AVAssetExportPresetAppleM4A];assert(exporter);exporter.outputURL=r.file;exporter.outputFileType=AVFileTypeAppleM4A;__block BOOL done=NO;[exporter exportAsynchronouslyWithCompletionHandler:^{done=YES;}];NSDate *deadline=[NSDate dateWithTimeIntervalSinceNow:10];while(!done&&deadline.timeIntervalSinceNow>0)pump(.01);assert(done&&exporter.status==AVAssetExportSessionStatusCompleted);
 [r.wave addObjectsFromArray:@[@0.1,@0.6,@0.3]];NSArray *frozen=r.wave.copy;double frozenStart=r.waveStart;
 dispatch_sync(r.queue,^{[r finishFile:r.file error:nil];});assert([r.wave isEqualToArray:frozen]);assert(r.waveStart==frozenStart);assert([snapshot(r)[@"wave_span"] doubleValue]==32.);NSDictionary *ready=snapshot(r);assert([ready[@"phase"] isEqual:@"ready"]);assert([ready[@"seconds"] doubleValue]>.9);assert([ready[@"recorded_at"] isEqual:r.startedAt]);NSString *file=r.file.path;
 [r command:3];pump(.1);dispatch_sync(r.queue,^{});assert(r.preview);[r command:3];dispatch_sync(r.queue,^{});assert(!r.preview.isPlaying);assert([snapshot(r)[@"seconds"] doubleValue]>.9);
 // Pause cursor is seekable and preview begins at that exact mark.
 r.phase=@"paused";[r seek:.25];dispatch_sync(r.queue,^{});assert(fabs(r.cursor-.25)<.001);
 [r command:3];dispatch_sync(r.queue,^{});assert(fabs(r.preview.currentTime-.25)<.1);[r command:3];dispatch_sync(r.queue,^{});
 // Replace the old tail: 0.25s of 220Hz prefix + new 440Hz segment; no old tail.
 NSURL *replacement=[NSURL fileURLWithPath:[NSString stringWithUTF8String:argv[2]]];NSData *sourceBytes=[NSData dataWithContentsOfURL:replacement];
 r.insertion=.25;r.finishPhase=@"paused";r.segment=replacement;r.phase=@"finishing";
 NSArray *beforePause=r.wave.copy;double beforeStart=r.waveStart;
 dispatch_sync(r.queue,^{[r finishFile:replacement error:nil];});NSDate *editDeadline=[NSDate dateWithTimeIntervalSinceNow:10];while([snapshot(r)[@"phase"] isEqual:@"finishing"]&&editDeadline.timeIntervalSinceNow>0)pump(.01);
 assert([snapshot(r)[@"phase"] isEqual:@"paused"]);assert([r.wave isEqualToArray:beforePause]);assert(r.waveStart==beforeStart);assert([snapshot(r)[@"wave_span"] doubleValue]==32.);assert(fabs(r.seconds-3.25)<.08);
 assert(fabs(frequency(r.file,.08)-220)<45);assert(fabs(frequency(r.file,.7)-440)<45);
 assert([[NSData dataWithContentsOfURL:replacement] isEqual:sourceBytes]);
 NSURL *master=r.file;NSData *masterBytes=[NSData dataWithContentsOfURL:master];
 dispatch_sync(r.queue,^{[r finishFile:[NSURL fileURLWithPath:@"/missing/segment.wav"] error:nil];});assert(r.editFailed);assert([snapshot(r)[@"phase"] isEqual:@"paused"]);assert([[NSData dataWithContentsOfURL:master] isEqual:masterBytes]);
 [r command:2];dispatch_sync(r.queue,^{});assert([snapshot(r)[@"phase"] isEqual:@"ready"]);assert(!r.editFailed);
 file=r.file.path;
 [r command:4];dispatch_sync(r.queue,^{});assert([snapshot(r)[@"phase"] isEqual:@"idle"]);assert(![NSFileManager.defaultManager fileExistsAtPath:file]);
 // A bad output remains an explicit error, never a ready take.
 dispatch_sync(r.queue,^{r.cancelled=NO;[r finishFile:[NSURL fileURLWithPath:@"/nonexistent/photosuite-test.m4a"] error:nil];});assert([snapshot(r)[@"phase"] isEqual:@"error"]);[r command:4];dispatch_sync(r.queue,^{});
 PSTestResumeRecorder *resume=[PSTestResumeRecorder new];resume.phase=@"paused";resume.cursor=.25;
 [resume command:1];dispatch_sync(resume.queue,^{});assert(resume.resumed);assert(resume.insertion==.25);assert([resume.phase isEqual:@"recording"]);
 resume.phase=@"paused";[resume command:2];dispatch_sync(resume.queue,^{});assert([resume.phase isEqual:@"ready"]);
 [resume command:4];dispatch_sync(resume.queue,^{});
 puts("Native recorder OK: M4A finalize, validation, preview/pause and owned-temp cleanup; no microphone accessed.");
}return 0;}
