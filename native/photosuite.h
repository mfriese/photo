#pragma once
#include <stddef.h>
// Encoded PNG buffer ownership is transferred to the caller; release with photosuite_free.
int photosuite_video_frame(const char *path, double seconds, double *actualSeconds, unsigned char **bytes, size_t *length);
void photosuite_free(void *bytes);
double photosuite_video_duration(const char *path);
// UI main thread only; media loading continues asynchronously.
int photosuite_video_play(const char *path);
// Explicit user action; moves to the OS Trash, never permanently deletes.
int photosuite_trash_file(const char *path);

// Probe on a worker thread. Birth timestamp buffer is caller-owned and always NUL-terminated.
int photosuite_audio_probe(const char *path, double *duration, int *audioOnly, char *created, size_t capacity);
// Main thread only; values copied synchronously. Weight is a relative axis unit, never seconds.
// Dates are civil clock seconds (no implicit timezone shift), NAN when not yet known.
// States: 0 pending, 1 verified, 2 failed. Only the selected asset loads at open.
int photosuite_audio_open(const char **paths,const char **labels,const double *weights,const double *dates,const double *durations,const unsigned int *states,size_t count,const char *year,const char *selected,unsigned long long *session);
// Read-only flags: 1 open, 2 has actually played, 4 transport error. Session guards replacement.
int photosuite_audio_state(unsigned long long session);
// Reorder/resize the same file set, without replacing or seeking the active AVPlayerItem.
int photosuite_audio_revise(unsigned long long session,const char **paths,const char **labels,const double *weights,const double *dates,const double *durations,const unsigned int *states,size_t count,const char *status);

// All context UI functions are main-thread-only. Flags: 1=player open, 2=new clip/settled seek.
int photosuite_audio_context_state(unsigned long long *session, unsigned long long *request, double *civil);
// Slots: 0/1 earlier, 2 nearest, 3/4 later. PNG bytes and tooltip copied synchronously.
void photosuite_audio_context_slot(unsigned long long session, unsigned long long request, unsigned int slot, const unsigned char *png, size_t length, const char *tooltip, int state);
void photosuite_audio_context_status(unsigned long long session, unsigned long long request, const char *status);
// Worker thread; malloc PNG output uses photosuite_free; orientation is already applied.
int photosuite_context_thumbnail(const char *path, unsigned int edge, unsigned char **bytes, size_t *length);

// Main thread; explicit date/clock captions, copied synchronously. NULL clears the text.
void photosuite_audio_context_caption(unsigned long long session, unsigned long long request, unsigned int slot, const char *date, const char *clock);

// Main thread; consumes at most one click. IDs ensure the slot still belongs to this selection.
int photosuite_audio_context_click(unsigned long long *session, unsigned long long *request, unsigned int *slot);

// Main-thread shared transport. showWindow=0 starts without displaying/focusing a window.
int photosuite_audio_open_mode(const char **paths,const char **labels,const double *weights,const double *dates,const double *durations,const unsigned int *states,size_t count,const char *year,const char *selected,unsigned long long *session,int showWindow);
// Flags: 1 active (also when hidden), 2 wants play, 4 loading/seeking, 8 failed. Civil time may be NAN.
int photosuite_audio_transport_state(unsigned long long session,double *fraction,double *civil);
// Commands: 0 toggle, 1 seek normalized [0,1], 2 show existing window, 3 stop.
int photosuite_audio_control(unsigned long long session,int command,double fraction);

// Recorder is opaque/retained. Commands are serialized internally; no UI-thread I/O.
// Destroy schedules cancellation; only its own temporary recording directory is removed.
void *photosuite_comment_recorder_create(void);
void photosuite_comment_recorder_destroy(void *recorder);
// UTF-8 JSON outputs are malloc-owned; release with photosuite_free. devices runs on a worker.
char *photosuite_comment_devices(void);
// Returns 1 while a previous capture is still finishing, otherwise queues start.
int photosuite_comment_start(void *recorder,const char *device);
// 0 pause, 1 resume, 2 finish, 3 preview play/pause, 4 discard/cancel.
void photosuite_comment_command(void *recorder,int command);
char *photosuite_comment_snapshot(void *recorder);

// Matches application CAPTURE_WAVE_SAMPLES: 30-Hz envelope history, approximately 32 s.
#define PHOTOSUITE_COMMENT_WAVE_SAMPLES 960

// Queue a cursor update in a paused/finalized own-temp take.
void photosuite_comment_seek(void *recorder,double seconds);

// Main-thread photo-record intent. IDs bind it to the displayed center photo; no microphone access.
int photosuite_audio_context_record(unsigned long long *session,unsigned long long *request,unsigned int *slot);
