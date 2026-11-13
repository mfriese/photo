#import "comment_edit.h"
#import <math.h>
AVAssetExportSession *PSCommentReplacement(NSURL *base,double prefix,NSURL *segment,NSURL *destination,NSString *recordedAt,NSError **error){
    if(!isfinite(prefix)||prefix<0)return nil;
    AVURLAsset *old=[AVURLAsset URLAssetWithURL:base options:nil],*next=[AVURLAsset URLAssetWithURL:segment options:nil];
    AVAssetTrack *oldTrack=[old tracksWithMediaType:AVMediaTypeAudio].firstObject,*newTrack=[next tracksWithMediaType:AVMediaTypeAudio].firstObject;
    double duration=CMTimeGetSeconds(old.duration),added=CMTimeGetSeconds(next.duration);
    if(!oldTrack||!newTrack||!isfinite(duration)||!isfinite(added)||added<=0||prefix>duration+0.01||prefix+added>3600.01)return nil;
    AVMutableComposition *mix=[AVMutableComposition composition];AVMutableCompositionTrack *track=[mix addMutableTrackWithMediaType:AVMediaTypeAudio preferredTrackID:kCMPersistentTrackID_Invalid];
    CMTime cut=CMTimeMakeWithSeconds(fmin(prefix,duration),48000);
    if(prefix>0 && ![track insertTimeRange:CMTimeRangeMake(kCMTimeZero,cut) ofTrack:oldTrack atTime:kCMTimeZero error:error])return nil;
    if(![track insertTimeRange:CMTimeRangeMake(kCMTimeZero,next.duration) ofTrack:newTrack atTime:cut error:error])return nil;
    AVAssetExportSession *exporter=[[AVAssetExportSession alloc] initWithAsset:mix presetName:AVAssetExportPresetAppleM4A];
    exporter.outputURL=destination;exporter.outputFileType=AVFileTypeAppleM4A;
    if(recordedAt.length){AVMutableMetadataItem *date=[AVMutableMetadataItem metadataItem];date.keySpace=AVMetadataKeySpaceQuickTimeMetadata;date.key=AVMetadataQuickTimeMetadataKeyCreationDate;date.value=recordedAt;exporter.metadata=@[date];}
    return exporter;
}
