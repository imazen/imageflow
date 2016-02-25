/*
 * Copyright (c) Imazen LLC.
 * No part of this project, including this file, may be copied, modified,
 * propagated, or distributed except as permitted in COPYRIGHT.txt.
 * Licensed under the GNU Affero General Public License, Version 3.0.
 * Commercial licenses available at http://imageresizing.net/
 */
#pragma once
#ifdef _MSC_VER
#define inline __inline
#endif

#include <stdint.h>
#include <math.h>
#include <limits.h>

#define IR_PI 3.1415926535897932384626433832795

static inline int min(int a, int b) { return a <= b ? a : b; }
static inline int max(int a, int b) { return a >= b ? a : b; }

static inline unsigned int umin(unsigned int a, unsigned int b) { return a <= b ? a : b; }
static inline unsigned int umax(unsigned int a, unsigned int b) { return a >= b ? a : b; }

static inline uint64_t umin64(uint64_t a, uint64_t b) { return a <= b ? a : b; }
static inline uint64_t umax64(uint64_t a, uint64_t b) { return a >= b ? a : b; }

#define EVIL_CLAMP(x, low, high) (((x) > (high)) ? (high) : (((x) < (low)) ? (low) : (x)))

static inline double ir_guassian(double x, double stdDev)
{
    return (exp((-x * x) / (2 * stdDev * stdDev)) / (sqrt(2 * IR_PI) * stdDev));
}

static inline uint8_t uchar_clamp_ff(float clr)
{
    uint16_t result;

    result = (uint16_t)(int16_t)(clr + 0.5);

    if (result > 255) {
        result = (clr < 0) ? 0 : 255;
    }

    return (uint8_t)result;
}

static inline int intlog2(unsigned int val)
{
    int ret = -1;
    while (val != 0) {
        val >>= 1;
        ret++;
    }
    return ret;
}

static inline int isPowerOfTwo(unsigned int x) { return ((x != 0) && !(x & (x - 1))); }
