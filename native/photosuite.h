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
// Main thread only. All arrays/strings are copied synchronously; composition builds on a worker.
int photosuite_audio_open(const char **paths, const char **labels, const double *durations, const double *recorded_at, size_t count, const char *year, double start);

// recorded_at contains civil clock seconds (UTC reference for formatting only, no timezone shift).
