// Worker-thread only; callbacks/floats are borrowed for each call. No whisper engine ABI; whisper-rs owns inference.
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
// 0 audio read; 1 no audio track; 2 cancelled/callback stop; negative error.
int photosuite_transcription_audio(const char *path,void *samples_context,int (*samples)(void *,const float *,size_t),void *control_context,int (*cancel)(void *),double *duration,char *error,size_t capacity);
uint32_t photosuite_transcription_awake(void);
void photosuite_transcription_allow_sleep(uint32_t id);
#ifdef __cplusplus
}
#endif
