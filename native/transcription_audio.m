#import <Foundation/Foundation.h>
#import <AVFoundation/AVFoundation.h>
#import <IOKit/pwr_mgt/IOPMLib.h>
#include "transcription.h"
int photosuite_transcription_audio(const char *path,void *samples_context,int (*samples)(void *,const float *,size_t),void *control_context,int (*cancel)(void *),double *duration,char *error,size_t capacity){@autoreleasepool {
    if(cancel(control_context))return 2;
    NSURL *url=[NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
    AVURLAsset *asset=[AVURLAsset URLAssetWithURL:url options:nil];
    NSArray<AVAssetTrack *> *tracks=[asset tracksWithMediaType:AVMediaTypeAudio];
    if(!tracks.count){if(!asset.playable){snprintf(error,capacity,"Datei ist nicht abspielbar oder enthält keine lesbare Tonspur");return -1;}return 1;}
    *duration=CMTimeGetSeconds(asset.duration);
    NSError *failure=nil;AVAssetReader *reader=[[AVAssetReader alloc] initWithAsset:asset error:&failure];
    if(!reader){snprintf(error,capacity,"%s",failure.localizedDescription.UTF8String?:"Audio kann nicht gelesen werden");return -1;}
    NSDictionary *settings=@{AVFormatIDKey:@(kAudioFormatLinearPCM),AVSampleRateKey:@16000,AVNumberOfChannelsKey:@1,AVLinearPCMBitDepthKey:@32,AVLinearPCMIsFloatKey:@YES,AVLinearPCMIsBigEndianKey:@NO,AVLinearPCMIsNonInterleaved:@NO};
    AVAssetReaderTrackOutput *output=[[AVAssetReaderTrackOutput alloc] initWithTrack:tracks.firstObject outputSettings:settings];output.alwaysCopiesSampleData=NO;
    if(![reader canAddOutput:output]){snprintf(error,capacity,"Audiospur wird von macOS nicht unterstützt");return -1;}[reader addOutput:output];
    if(![reader startReading]){snprintf(error,capacity,"%s",reader.error.localizedDescription.UTF8String?:"Audio konnte nicht gestartet werden");return -1;}
    while(!cancel(control_context)){
        CMSampleBufferRef sample=[output copyNextSampleBuffer];if(!sample)break;
        CMBlockBufferRef block=CMSampleBufferGetDataBuffer(sample);size_t length=block?CMBlockBufferGetDataLength(block):0;
        if(length>64*1024*1024||length%sizeof(float)){CFRelease(sample);[reader cancelReading];snprintf(error,capacity,"Ungültiger PCM-Audioblock");return -1;}
        char *bytes=NULL;size_t contiguous=0,total=0;int status=block?CMBlockBufferGetDataPointer(block,0,&contiguous,&total,&bytes):-1;
        NSData *copy=nil;if(status||contiguous<length){NSMutableData *data=[NSMutableData dataWithLength:length];if(!block||CMBlockBufferCopyDataBytes(block,0,length,data.mutableBytes)!=kCMBlockBufferNoErr){CFRelease(sample);[reader cancelReading];snprintf(error,capacity,"Audioblock konnte nicht gelesen werden");return -1;}copy=data;bytes=(char *)copy.bytes;}
        int stopped=length?samples(samples_context,(const float *)bytes,length/sizeof(float)):0;CFRelease(sample);
        if(stopped){[reader cancelReading];return 2;}
    }
    if(cancel(control_context)){[reader cancelReading];return 2;}
    if(reader.status==AVAssetReaderStatusFailed){snprintf(error,capacity,"%s",reader.error.localizedDescription.UTF8String?:"Audio-Lesefehler");return -1;}return 0;
}}
uint32_t photosuite_transcription_awake(void){IOPMAssertionID id=kIOPMNullAssertionID;IOReturn result=IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleSystemSleep,kIOPMAssertionLevelOn,CFSTR("PhotoSuite verarbeitet Transkripte"),&id);return result==kIOReturnSuccess?id:0;}
void photosuite_transcription_allow_sleep(uint32_t id){if(id)IOPMAssertionRelease(id);}
