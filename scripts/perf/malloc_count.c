/*
 * malloc_count.c — count heap allocations of an unmodified binary.
 *
 * Build (macOS, Command Line Tools clang):
 *   clang -O2 -dynamiclib -o scripts/perf/libmalloc_count.dylib scripts/perf/malloc_count.c
 *
 * Run:
 *   DYLD_INSERT_LIBRARIES="$PWD/scripts/perf/libmalloc_count.dylib" \
 *     target/release/cloth-fit --garment tshirt --mesh-mm 30 --steps 60 ...
 *
 * The process prints one line to stderr on exit:
 *   MALLOC_COUNT allocations=<n> bytes=<n> callocs=<n> reallocs=<n>
 *
 * malloc/calloc/realloc cover Rust's System allocator on macOS, which routes
 * through the default malloc zone. Only allocations are counted; `free` is left
 * untouched so the interposer can never free a pointer through the wrong zone.
 *
 * This counts *all* heap traffic in the process, including metrics
 * serialisation and I/O buffers. Treat the difference of two run lengths as the
 * per-step figure, and do not label it solver-only.
 *
 * Real implementations go through malloc_zone_* so the interposed symbols are
 * never re-entered.
 */
#include <malloc/malloc.h>
#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>

static unsigned long long g_malloc;
static unsigned long long g_calloc;
static unsigned long long g_realloc;
static unsigned long long g_bytes;
static int g_active = 1;

static void report(void) __attribute__((destructor));
static void report(void) {
    char buffer[256];
    int length = snprintf(
        buffer,
        sizeof buffer,
        "MALLOC_COUNT allocations=%llu bytes=%llu callocs=%llu reallocs=%llu\n",
        g_malloc, g_bytes, g_calloc, g_realloc);
    if (length > 0) {
        ssize_t ignored = write(2, buffer, (size_t)length);
        (void)ignored;
    }
}

static void *counted_malloc(size_t size) {
    if (g_active) {
        g_malloc++;
        g_bytes += size;
    }
    return malloc_zone_malloc(malloc_default_zone(), size);
}

static void *counted_calloc(size_t count, size_t size) {
    if (g_active) {
        g_calloc++;
        g_bytes += count * size;
    }
    return malloc_zone_calloc(malloc_default_zone(), count, size);
}

static void *counted_realloc(void *pointer, size_t size) {
    if (g_active) {
        g_realloc++;
        g_bytes += size;
    }
    return malloc_zone_realloc(malloc_default_zone(), pointer, size);
}

#define DYLD_INTERPOSE(replacement, replacee)                                       \
    __attribute__((used)) static struct {                                           \
        const void *replacement;                                                    \
        const void *replacee;                                                       \
    } interpose_##replacee __attribute__((section("__DATA,__interpose"))) = {        \
        (const void *)(unsigned long)&replacement,                                  \
        (const void *)(unsigned long)&replacee,                                     \
    }

DYLD_INTERPOSE(counted_malloc, malloc);
DYLD_INTERPOSE(counted_calloc, calloc);
DYLD_INTERPOSE(counted_realloc, realloc);
