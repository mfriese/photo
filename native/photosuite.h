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
