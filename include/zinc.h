#ifndef ZINC_H
#define ZINC_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define MAGIC 6505817187982394695

#define VERSION 2

typedef void *ZincHandle;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * # Safety
 * Non-null `name` must be a readable, NUL-terminated string and `out` writable.
 */
int32_t zinc_create(const char *aName,
                    uintptr_t aCapacity,
                    ZincHandle *aOut);

/**
 * # Safety
 * Non-null `name` must be a readable, NUL-terminated string and `out` writable.
 */
int32_t zinc_open(const char *aName,
                  ZincHandle *aOut);

/**
 * # Safety
 * Non-null `h` must be a live Zinc handle. The returned pointer expires on close.
 */
uint8_t *zinc_ptr(ZincHandle aH);

/**
 * # Safety
 * Non-null `h` must be a live Zinc handle.
 */
uintptr_t zinc_capacity(ZincHandle aH);

/**
 * # Safety
 * Non-null `h` must be a live Zinc handle, closed exactly once after all uses finish.
 */
void zinc_close(ZincHandle aH);

/**
 * # Safety
 * Non-null `h` must be a live Zinc handle.
 */
void zinc_notify(ZincHandle aH);

/**
 * # Safety
 * Non-null `h` must be a live Zinc handle for the duration of the wait.
 */
int32_t zinc_wait(ZincHandle aH,
                  uint32_t aTimeoutMs);

/**
 * Return 0 for a pending notification, -11 otherwise, or -22 for a null handle.
 * # Safety
 * Non-null `h` must be a live Zinc handle.
 */
int32_t zinc_try_wait(ZincHandle aH);

uint32_t zinc_version(void);

#ifdef __cplusplus
}  // extern "C"
#endif  // __cplusplus

#endif  /* ZINC_H */
