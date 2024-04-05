/*
 * Option handlers shared between the tools.
 *
 * This file is part of FFmpeg.
 *
 * FFmpeg is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2.1 of the License, or (at your option) any later version.
 *
 * FFmpeg is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with FFmpeg; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA 02110-1301 USA
 */

#ifndef FFTOOLS_OPT_COMMON_H
#define FFTOOLS_OPT_COMMON_H

#include "cmdutils.h"

/**
 * Print a listing containing autodetected sinks of the output device.
 * Device name with options may be passed as an argument to limit results.
 */
int show_sinks(void *optctx, const char *opt, const char *arg);

/**
 * Print a listing containing autodetected sources of the input device.
 * Device name with options may be passed as an argument to limit results.
 */
int show_sources(void *optctx, const char *opt, const char *arg);

// clang-format off
#define CMDUTILS_COMMON_OPTIONS_AVDEVICE                                                                                \
    { "sources"    , OPT_EXIT | HAS_ARG, { .func_arg = show_sources }, "list sources of the input device", "device" },  \
    { "sinks"      , OPT_EXIT | HAS_ARG, { .func_arg = show_sinks },   "list sinks of the output device",  "device" },
// clang-format on

/**
 * Generic -h handler common to all fftools.
 */
int show_help(void *optctx, const char *opt, const char *arg);

/**
 * Set the libav* libraries log level.
 */
int opt_loglevel(void *optctx, const char *opt, const char *arg);

int opt_report(void *optctx, const char *opt, const char *arg);
int init_report(const char *env, FILE **file);

int opt_max_alloc(void *optctx, const char *opt, const char *arg);

/**
 * Override the cpuflags.
 */
int opt_cpuflags(void *optctx, const char *opt, const char *arg);

/**
 * Override the cpucount.
 */
int opt_cpucount(void *optctx, const char *opt, const char *arg);

// clang-format off
#define CMDUTILS_COMMON_OPTIONS                                                                                         \
    { "h",           OPT_EXIT,             { .func_arg = show_help },        "show help", "topic" },                    \
    { "?",           OPT_EXIT,             { .func_arg = show_help },        "show help", "topic" },                    \
    { "help",        OPT_EXIT,             { .func_arg = show_help },        "show help", "topic" },                    \
    { "-help",       OPT_EXIT,             { .func_arg = show_help },        "show help", "topic" },                    \
    { "loglevel",    HAS_ARG,              { .func_arg = opt_loglevel },     "set logging level", "loglevel" },         \
    { "v",           HAS_ARG,              { .func_arg = opt_loglevel },     "set logging level", "loglevel" },         \
    { "report",      0,                    { .func_arg = opt_report },       "generate a report" },                     \
    { "max_alloc",   HAS_ARG,              { .func_arg = opt_max_alloc },    "set maximum size of a single allocated block", "bytes" }, \
    { "cpuflags",    HAS_ARG | OPT_EXPERT, { .func_arg = opt_cpuflags },     "force specific cpu flags", "flags" },     \
    { "cpucount",    HAS_ARG | OPT_EXPERT, { .func_arg = opt_cpucount },     "force specific cpu count", "count" },     \
    { "hide_banner", OPT_BOOL | OPT_EXPERT, {&hide_banner},     "do not show program banner", "hide_banner" },          \
    CMDUTILS_COMMON_OPTIONS_AVDEVICE
// clang-format on

#endif /* FFTOOLS_OPT_COMMON_H */
