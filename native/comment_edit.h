#import <AVFoundation/AVFoundation.h>
// Own-temp audio only. Build a prefix + replacement composition; never modify base.
AVAssetExportSession *PSCommentReplacement(NSURL *base,double prefix,NSURL *segment,NSURL *destination,NSString *recordedAt,NSError **error);
