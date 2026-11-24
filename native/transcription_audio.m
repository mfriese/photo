#import <Foundation/Foundation.h>
#import <AVFoundation/AVFoundation.h>
#import <IOKit/pwr_mgt/IOPMLib.h>
#include "transcription.h"
#import <math.h>
#import <string.h>

typedef struct {
    double sample_rate;
    double next_output_frame;
    uint64_t source_frames;
    float previous;
    BOOL has_previous;
    float output[4096];
    size_t output_count;
    void *samples_context;
    int (*samples)(void *, const float *, size_t);
    void *control_context;
    int (*cancel)(void *);
} PSPcmResampler;

static void PSSetAudioReaderError(char *buffer,size_t capacity,NSError *failure,NSString *fallback){
    NSString *message=failure.localizedDescription?:fallback;
    NSError *underlying=failure.userInfo[NSUnderlyingErrorKey];
    snprintf(buffer,capacity,"%s (domain=%s code=%ld; underlying=%s/%ld: %s)",message.UTF8String?:"Audio-Lesefehler",failure.domain.UTF8String?:"unknown",(long)failure.code,underlying.domain.UTF8String?:"none",(long)underlying.code,underlying.localizedDescription.UTF8String?:"none");
}

static double PSReadPcmSample(const uint8_t *bytes,const AudioStreamBasicDescription *format){
    size_t width=format->mBitsPerChannel/8;
    uint64_t raw=0;
    for(size_t index=0;index<width;index++){
        size_t shift=(format->mFormatFlags&kAudioFormatFlagIsBigEndian)?(width-index-1)*8:index*8;
        raw|=(uint64_t)bytes[index]<<shift;
    }
    if(format->mFormatFlags&kAudioFormatFlagIsFloat){
        if(format->mBitsPerChannel==32){uint32_t bits=(uint32_t)raw;float value;memcpy(&value,&bits,sizeof(value));return value;}
        if(format->mBitsPerChannel==64){double value;memcpy(&value,&raw,sizeof(value));return value;}
        return NAN;
    }
    double scale=ldexp(1.0,(int)format->mBitsPerChannel-1);
    if(format->mFormatFlags&kAudioFormatFlagIsSignedInteger){
        int64_t value=(int64_t)(raw<<(64-format->mBitsPerChannel))>>(64-format->mBitsPerChannel);
        return (double)value/scale;
    }
    return ((double)raw-scale)/scale;
}

static int PSFlushPcm(PSPcmResampler *state){
    if(!state->output_count)return 0;
    size_t count=state->output_count;state->output_count=0;
    return state->samples(state->samples_context,state->output,count);
}

static int PSForwardPcm(PSPcmResampler *state,const uint8_t *bytes,size_t length,const AudioStreamBasicDescription *format){
    size_t channels=format->mChannelsPerFrame,width=format->mBitsPerChannel/8,frame_bytes=format->mBytesPerFrame;
    if(!isfinite(state->sample_rate)||state->sample_rate<=0||!channels||!width||format->mBitsPerChannel>64||format->mBitsPerChannel%8||!frame_bytes||frame_bytes<channels*width||length%frame_bytes)return -1;
    if((format->mFormatFlags&kAudioFormatFlagIsFloat)&&format->mBitsPerChannel!=32&&format->mBitsPerChannel!=64)return -1;
    if((format->mFormatFlags&kAudioFormatFlagIsNonInterleaved)&&channels>1)return -1;
    size_t frames=length/frame_bytes;
    double step=state->sample_rate/16000.0;
    for(size_t frame=0;frame<frames;frame++){
        if(state->cancel(state->control_context)){PSFlushPcm(state);return 2;}
        double sum=0;
        for(size_t channel=0;channel<channels;channel++)sum+=PSReadPcmSample(bytes+frame*frame_bytes+channel*width,format);
        float current=(float)(sum/channels);
        uint64_t index=state->source_frames++;
        while(state->next_output_frame<=(double)index){
            double value=current;
            if(state->has_previous&&index>0){double fraction=state->next_output_frame-(double)(index-1);value=state->previous+(current-state->previous)*fraction;}
            state->output[state->output_count++]=isfinite(value)?(float)fmax(-1.,fmin(1.,value)):0.f;
            state->next_output_frame+=step;
            if(state->output_count==sizeof(state->output)/sizeof(state->output[0])){
                if(PSFlushPcm(state))return 2;
            }
        }
        state->previous=current;state->has_previous=YES;
    }
    return 0;
}

int photosuite_transcription_audio(const char *path,void *samples_context,int (*samples)(void *,const float *,size_t),void *control_context,int (*cancel)(void *),double *duration,char *error,size_t capacity){@autoreleasepool {
    if(cancel(control_context))return 2;
    NSURL *url=[NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
    AVURLAsset *asset=[AVURLAsset URLAssetWithURL:url options:nil];
    NSArray<AVAssetTrack *> *tracks=[asset tracksWithMediaType:AVMediaTypeAudio];
    if(!tracks.count){if(!asset.playable){snprintf(error,capacity,"Datei ist nicht abspielbar oder enthält keine lesbare Tonspur");return -1;}return 1;}
    *duration=CMTimeGetSeconds(asset.duration);
    NSError *failure=nil;AVAssetReader *reader=[[AVAssetReader alloc] initWithAsset:asset error:&failure];
    if(!reader){PSSetAudioReaderError(error,capacity,failure,@"Audio kann nicht gelesen werden");return -1;}
    AVAssetTrack *track=tracks.firstObject;
    CMAudioFormatDescriptionRef sourceDescription=track.formatDescriptions.count?(__bridge CMAudioFormatDescriptionRef)track.formatDescriptions.firstObject:NULL;
    const AudioStreamBasicDescription *sourceFormat=sourceDescription?CMAudioFormatDescriptionGetStreamBasicDescription(sourceDescription):NULL;
    BOOL sourceIsPcm=sourceFormat&&sourceFormat->mFormatID==kAudioFormatLinearPCM;
    NSDictionary *settings=@{AVFormatIDKey:@(kAudioFormatLinearPCM),AVSampleRateKey:@16000,AVNumberOfChannelsKey:@1,AVLinearPCMBitDepthKey:@32,AVLinearPCMIsFloatKey:@YES,AVLinearPCMIsBigEndianKey:@NO,AVLinearPCMIsNonInterleaved:@NO};
    AVAssetReaderTrackOutput *output=[[AVAssetReaderTrackOutput alloc] initWithTrack:track outputSettings:sourceIsPcm?nil:settings];output.alwaysCopiesSampleData=NO;
    if(![reader canAddOutput:output]){snprintf(error,capacity,"Audiospur wird von macOS nicht unterstützt");return -1;}[reader addOutput:output];
    if(![reader startReading]){PSSetAudioReaderError(error,capacity,reader.error,@"Audio konnte nicht gestartet werden");return -1;}
    PSPcmResampler pcm={.sample_rate=sourceIsPcm?sourceFormat->mSampleRate:16000,.samples_context=samples_context,.samples=samples,.control_context=control_context,.cancel=cancel};
    while(!cancel(control_context)){
        CMSampleBufferRef sample=[output copyNextSampleBuffer];if(!sample)break;
        CMBlockBufferRef block=CMSampleBufferGetDataBuffer(sample);size_t length=block?CMBlockBufferGetDataLength(block):0;
        if(length>64*1024*1024||(!sourceIsPcm&&length%sizeof(float))){CFRelease(sample);[reader cancelReading];snprintf(error,capacity,"Ungültiger PCM-Audioblock");return -1;}
        if(!length){CFRelease(sample);continue;}
        char *bytes=NULL;size_t contiguous=0,total=0;int status=block?CMBlockBufferGetDataPointer(block,0,&contiguous,&total,&bytes):-1;
        NSData *copy=nil;if(status||contiguous<length){NSMutableData *data=[NSMutableData dataWithLength:length];OSStatus copyStatus=-1;if(block)copyStatus=CMBlockBufferCopyDataBytes(block,0,length,data.mutableBytes);if(copyStatus!=kCMBlockBufferNoErr){CFRelease(sample);[reader cancelReading];snprintf(error,capacity,"Audioblock konnte nicht gelesen werden (pointer=%d contiguous=%zu length=%zu copy=%d)",status,contiguous,length,(int)copyStatus);return -1;}copy=data;bytes=(char *)copy.bytes;}
        int stopped=0;
        if(length)stopped=sourceIsPcm?PSForwardPcm(&pcm,(const uint8_t *)bytes,length,sourceFormat):samples(samples_context,(const float *)bytes,length/sizeof(float));
        CFRelease(sample);
        if(stopped<0){[reader cancelReading];snprintf(error,capacity,"PCM-Audioformat wird nicht unterstützt");return -1;}
        if(stopped){[reader cancelReading];return 2;}
    }
    if(cancel(control_context)){[reader cancelReading];return 2;}
    if(reader.status==AVAssetReaderStatusFailed){PSSetAudioReaderError(error,capacity,reader.error,@"Audio-Lesefehler");return -1;}
    if(sourceIsPcm&&PSFlushPcm(&pcm)){return 2;}
    return 0;
}}
uint32_t photosuite_transcription_awake(void){IOPMAssertionID id=kIOPMNullAssertionID;IOReturn result=IOPMAssertionCreateWithName(kIOPMAssertionTypePreventUserIdleSystemSleep,kIOPMAssertionLevelOn,CFSTR("PhotoSuite verarbeitet Transkripte"),&id);return result==kIOReturnSuccess?id:0;}
void photosuite_transcription_allow_sleep(uint32_t id){if(id)IOPMAssertionRelease(id);}
