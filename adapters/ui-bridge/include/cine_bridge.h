#ifndef CINE_BRIDGE_H
#define CINE_BRIDGE_H
#include <stdint.h>
#include <stddef.h>
/* Experimental ABI v1. No Rust structs cross this boundary. SDK pointers are native-only.
 * Handles are process-local. All output memory belongs to caller.
 * call output_capacity must be >=65536 before any dispatch.
 * Return: byte count >=0; -1 stale/destroyed handle; -2 buffer/limit;
 * -3 internal output bound; hash: -4 busy, -5 suspended, -6 invalid/source.
 * FD is borrowed: Rust duplicates and closes its copy, caller closes original.
 * destroy cancels/joins regular-file worker; call from background isolate if needed.
 */
#ifdef __cplusplus
extern "C" {
#endif
uint64_t cine_bridge_create(void);
int32_t cine_bridge_destroy(uint64_t handle);
int32_t cine_bridge_call(uint64_t handle, const uint8_t *input, size_t length,
                        uint8_t *output, size_t output_capacity);
int32_t cine_bridge_hash_fd(uint64_t handle, int32_t fd);
#ifdef __linux__
/* Native runner only, never Dart. Free mpv_render_context before releasing lease.
 * A lease retains the SDK owner even after cine_bridge_destroy. */
uint64_t cine_bridge_video_acquire(uint64_t handle, void **native_player);
int32_t cine_bridge_video_release(uint64_t lease);
#endif
#ifdef __cplusplus
}
#endif
#endif
