#ifndef ZINC_H
#define ZINC_H

#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define MAGIC 6505817187982394695

#define VERSION 1

typedef void *ZincHandle;

int32_t zinc_create(const char *aName,
                    uintptr_t aCapacity,
                    ZincHandle *aOut);

int32_t zinc_open(const char *aName,
                  ZincHandle *aOut);

uint8_t *zinc_ptr(ZincHandle aH);

uintptr_t zinc_capacity(ZincHandle aH);

void zinc_close(ZincHandle aH);

void zinc_notify(ZincHandle aH);

int32_t zinc_wait(ZincHandle aH,
                  uint32_t aTimeoutMs);

uint32_t zinc_version(void);

#endif  /* ZINC_H */
