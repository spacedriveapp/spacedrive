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

#include <stdio.h>
#include <time.h>

#include "libavcodec/bsf.h"
#include "libavcodec/codec.h"
#include "libavcodec/codec_desc.h"
#include "libavdevice/avdevice.h"
#include "libavfilter/avfilter.h"
#include "libavformat/avformat.h"
#include "libavutil/avassert.h"
#include "libavutil/avstring.h"
#include "libavutil/bprint.h"
#include "libavutil/channel_layout.h"
#include "libavutil/cpu.h"
#include "libavutil/error.h"
#include "libavutil/log.h"
#include "libavutil/mem.h"
#include "libavutil/opt.h"
#include "libavutil/version.h"

#include "cmdutils.h"
#include "opt_common.h"

static FILE *report_file;
static int report_file_level = AV_LOG_DEBUG;

#define INDENT 1
#define SHOW_VERSION 2
#define SHOW_CONFIG 4
#define SHOW_COPYRIGHT 8

#define PRINT_CODEC_SUPPORTED(codec, field, type, list_name, term, get_name)   \
  if (codec->field) {                                                          \
    const type *p = codec->field;                                              \
                                                                               \
    printf("    Supported " list_name ":");                                    \
    while (*p != term) {                                                       \
      get_name(*p);                                                            \
      printf(" %s", name);                                                     \
      p++;                                                                     \
    }                                                                          \
    printf("\n");                                                              \
  }

static void print_codec(const AVCodec *c) {
  int encoder = av_codec_is_encoder(c);

  printf("%s %s [%s]:\n", encoder ? "Encoder" : "Decoder", c->name,
         c->long_name ? c->long_name : "");

  printf("    General capabilities: ");
  if (c->capabilities & AV_CODEC_CAP_DRAW_HORIZ_BAND)
    printf("horizband ");
  if (c->capabilities & AV_CODEC_CAP_DR1)
    printf("dr1 ");
  if (c->capabilities & AV_CODEC_CAP_DELAY)
    printf("delay ");
  if (c->capabilities & AV_CODEC_CAP_SMALL_LAST_FRAME)
    printf("small ");
  if (c->capabilities & AV_CODEC_CAP_EXPERIMENTAL)
    printf("exp ");
  if (c->capabilities & AV_CODEC_CAP_CHANNEL_CONF)
    printf("chconf ");
  if (c->capabilities & AV_CODEC_CAP_PARAM_CHANGE)
    printf("paramchange ");
  if (c->capabilities & AV_CODEC_CAP_VARIABLE_FRAME_SIZE)
    printf("variable ");
  if (c->capabilities &
      (AV_CODEC_CAP_FRAME_THREADS | AV_CODEC_CAP_SLICE_THREADS |
       AV_CODEC_CAP_OTHER_THREADS))
    printf("threads ");
  if (c->capabilities & AV_CODEC_CAP_AVOID_PROBING)
    printf("avoidprobe ");
  if (c->capabilities & AV_CODEC_CAP_HARDWARE)
    printf("hardware ");
  if (c->capabilities & AV_CODEC_CAP_HYBRID)
    printf("hybrid ");
  if (!c->capabilities)
    printf("none");
  printf("\n");

  if (c->type == AVMEDIA_TYPE_VIDEO || c->type == AVMEDIA_TYPE_AUDIO) {
    printf("    Threading capabilities: ");
    switch (c->capabilities &
            (AV_CODEC_CAP_FRAME_THREADS | AV_CODEC_CAP_SLICE_THREADS |
             AV_CODEC_CAP_OTHER_THREADS)) {
    case AV_CODEC_CAP_FRAME_THREADS | AV_CODEC_CAP_SLICE_THREADS:
      printf("frame and slice");
      break;
    case AV_CODEC_CAP_FRAME_THREADS:
      printf("frame");
      break;
    case AV_CODEC_CAP_SLICE_THREADS:
      printf("slice");
      break;
    case AV_CODEC_CAP_OTHER_THREADS:
      printf("other");
      break;
    default:
      printf("none");
      break;
    }
    printf("\n");
  }

  if (avcodec_get_hw_config(c, 0)) {
    printf("    Supported hardware devices: ");
    for (int i = 0;; i++) {
      const AVCodecHWConfig *config = avcodec_get_hw_config(c, i);
      const char *name;
      if (!config)
        break;
      name = av_hwdevice_get_type_name(config->device_type);
      if (name)
        printf("%s ", name);
    }
    printf("\n");
  }

  if (c->supported_framerates) {
    const AVRational *fps = c->supported_framerates;

    printf("    Supported framerates:");
    while (fps->num) {
      printf(" %d/%d", fps->num, fps->den);
      fps++;
    }
    printf("\n");
  }
  PRINT_CODEC_SUPPORTED(c, pix_fmts, enum AVPixelFormat, "pixel formats",
                        AV_PIX_FMT_NONE, GET_PIX_FMT_NAME);
  PRINT_CODEC_SUPPORTED(c, supported_samplerates, int, "sample rates", 0,
                        GET_SAMPLE_RATE_NAME);
  PRINT_CODEC_SUPPORTED(c, sample_fmts, enum AVSampleFormat, "sample formats",
                        AV_SAMPLE_FMT_NONE, GET_SAMPLE_FMT_NAME);

  if (c->ch_layouts) {
    const AVChannelLayout *p = c->ch_layouts;

    printf("    Supported channel layouts:");
    while (p->nb_channels) {
      char name[128];
      av_channel_layout_describe(p, name, sizeof(name));
      printf(" %s", name);
      p++;
    }
    printf("\n");
  }

  if (c->priv_class) {
    show_help_children(c->priv_class,
                       AV_OPT_FLAG_ENCODING_PARAM | AV_OPT_FLAG_DECODING_PARAM);
  }
}

static const AVCodec *next_codec_for_id(enum AVCodecID id, void **iter,
                                        int encoder) {
  const AVCodec *c;
  while ((c = av_codec_iterate(iter))) {
    if (c->id == id &&
        (encoder ? av_codec_is_encoder(c) : av_codec_is_decoder(c)))
      return c;
  }
  return NULL;
}

static void show_help_codec(const char *name, int encoder) {
  const AVCodecDescriptor *desc;
  const AVCodec *codec;

  if (!name) {
    av_log(NULL, AV_LOG_ERROR, "No codec name specified.\n");
    return;
  }

  codec = encoder ? avcodec_find_encoder_by_name(name)
                  : avcodec_find_decoder_by_name(name);

  if (codec)
    print_codec(codec);
  else if ((desc = avcodec_descriptor_get_by_name(name))) {
    void *iter = NULL;
    int printed = 0;

    while ((codec = next_codec_for_id(desc->id, &iter, encoder))) {
      printed = 1;
      print_codec(codec);
    }

    if (!printed) {
      av_log(NULL, AV_LOG_ERROR,
             "Codec '%s' is known to FFmpeg, "
             "but no %s for it are available. FFmpeg might need to be "
             "recompiled with additional external libraries.\n",
             name, encoder ? "encoders" : "decoders");
    }
  } else {
    av_log(NULL, AV_LOG_ERROR, "Codec '%s' is not recognized by FFmpeg.\n",
           name);
  }
}

static void show_help_demuxer(const char *name) {
  const AVInputFormat *fmt = av_find_input_format(name);

  if (!fmt) {
    av_log(NULL, AV_LOG_ERROR, "Unknown format '%s'.\n", name);
    return;
  }

  printf("Demuxer %s [%s]:\n", fmt->name, fmt->long_name);

  if (fmt->extensions)
    printf("    Common extensions: %s.\n", fmt->extensions);

  if (fmt->priv_class)
    show_help_children(fmt->priv_class, AV_OPT_FLAG_DECODING_PARAM);
}

static void show_help_protocol(const char *name) {
  const AVClass *proto_class;

  if (!name) {
    av_log(NULL, AV_LOG_ERROR, "No protocol name specified.\n");
    return;
  }

  proto_class = avio_protocol_get_class(name);
  if (!proto_class) {
    av_log(NULL, AV_LOG_ERROR, "Unknown protocol '%s'.\n", name);
    return;
  }

  show_help_children(proto_class,
                     AV_OPT_FLAG_DECODING_PARAM | AV_OPT_FLAG_ENCODING_PARAM);
}

static void show_help_muxer(const char *name) {
  const AVCodecDescriptor *desc;
  const AVOutputFormat *fmt = av_guess_format(name, NULL, NULL);

  if (!fmt) {
    av_log(NULL, AV_LOG_ERROR, "Unknown format '%s'.\n", name);
    return;
  }

  printf("Muxer %s [%s]:\n", fmt->name, fmt->long_name);

  if (fmt->extensions)
    printf("    Common extensions: %s.\n", fmt->extensions);
  if (fmt->mime_type)
    printf("    Mime type: %s.\n", fmt->mime_type);
  if (fmt->video_codec != AV_CODEC_ID_NONE &&
      (desc = avcodec_descriptor_get(fmt->video_codec))) {
    printf("    Default video codec: %s.\n", desc->name);
  }
  if (fmt->audio_codec != AV_CODEC_ID_NONE &&
      (desc = avcodec_descriptor_get(fmt->audio_codec))) {
    printf("    Default audio codec: %s.\n", desc->name);
  }
  if (fmt->subtitle_codec != AV_CODEC_ID_NONE &&
      (desc = avcodec_descriptor_get(fmt->subtitle_codec))) {
    printf("    Default subtitle codec: %s.\n", desc->name);
  }

  if (fmt->priv_class)
    show_help_children(fmt->priv_class, AV_OPT_FLAG_ENCODING_PARAM);
}

static void show_help_filter(const char *name) {
  const AVFilter *f = avfilter_get_by_name(name);
  int i, count;

  if (!name) {
    av_log(NULL, AV_LOG_ERROR, "No filter name specified.\n");
    return;
  } else if (!f) {
    av_log(NULL, AV_LOG_ERROR, "Unknown filter '%s'.\n", name);
    return;
  }

  printf("Filter %s\n", f->name);
  if (f->description)
    printf("  %s\n", f->description);

  if (f->flags & AVFILTER_FLAG_SLICE_THREADS)
    printf("    slice threading supported\n");

  printf("    Inputs:\n");
  count = avfilter_filter_pad_count(f, 0);
  for (i = 0; i < count; i++) {
    printf("       #%d: %s (%s)\n", i, avfilter_pad_get_name(f->inputs, i),
           av_get_media_type_string(avfilter_pad_get_type(f->inputs, i)));
  }
  if (f->flags & AVFILTER_FLAG_DYNAMIC_INPUTS)
    printf("        dynamic (depending on the options)\n");
  else if (!count)
    printf("        none (source filter)\n");

  printf("    Outputs:\n");
  count = avfilter_filter_pad_count(f, 1);
  for (i = 0; i < count; i++) {
    printf("       #%d: %s (%s)\n", i, avfilter_pad_get_name(f->outputs, i),
           av_get_media_type_string(avfilter_pad_get_type(f->outputs, i)));
  }
  if (f->flags & AVFILTER_FLAG_DYNAMIC_OUTPUTS)
    printf("        dynamic (depending on the options)\n");
  else if (!count)
    printf("        none (sink filter)\n");

  if (f->priv_class)
    show_help_children(f->priv_class, AV_OPT_FLAG_VIDEO_PARAM |
                                          AV_OPT_FLAG_FILTERING_PARAM |
                                          AV_OPT_FLAG_AUDIO_PARAM);
  if (f->flags & AVFILTER_FLAG_SUPPORT_TIMELINE)
    printf(
        "This filter has support for timeline through the 'enable' option.\n");
}

static void show_help_bsf(const char *name) {
  const AVBitStreamFilter *bsf = av_bsf_get_by_name(name);

  if (!name) {
    av_log(NULL, AV_LOG_ERROR, "No bitstream filter name specified.\n");
    return;
  } else if (!bsf) {
    av_log(NULL, AV_LOG_ERROR, "Unknown bit stream filter '%s'.\n", name);
    return;
  }

  printf("Bit stream filter %s\n", bsf->name);
  PRINT_CODEC_SUPPORTED(bsf, codec_ids, enum AVCodecID, "codecs",
                        AV_CODEC_ID_NONE, GET_CODEC_NAME);
  if (bsf->priv_class)
    show_help_children(bsf->priv_class, AV_OPT_FLAG_BSF_PARAM);
}

int show_help(void *optctx, const char *opt, const char *arg) {
  char *topic, *par;
  av_log_set_callback(log_callback_help);

  topic = av_strdup(arg ? arg : "");
  if (!topic)
    return AVERROR(ENOMEM);
  par = strchr(topic, '=');
  if (par)
    *par++ = 0;

  if (!*topic) {
    show_help_default(topic, par);
  } else if (!strcmp(topic, "decoder")) {
    show_help_codec(par, 0);
  } else if (!strcmp(topic, "encoder")) {
    show_help_codec(par, 1);
  } else if (!strcmp(topic, "demuxer")) {
    show_help_demuxer(par);
  } else if (!strcmp(topic, "muxer")) {
    show_help_muxer(par);
  } else if (!strcmp(topic, "protocol")) {
    show_help_protocol(par);
  } else if (!strcmp(topic, "filter")) {
    show_help_filter(par);
  } else if (!strcmp(topic, "bsf")) {
    show_help_bsf(par);
  } else {
    show_help_default(topic, par);
  }

  av_freep(&topic);
  return 0;
}

int opt_cpuflags(void *optctx, const char *opt, const char *arg) {
  int ret;
  unsigned flags = av_get_cpu_flags();

  if ((ret = av_parse_cpu_caps(&flags, arg)) < 0)
    return ret;

  av_force_cpu_flags(flags);
  return 0;
}

int opt_cpucount(void *optctx, const char *opt, const char *arg) {
  int ret;
  int count;

  static const AVOption opts[] = {
      {"count", NULL, 0, AV_OPT_TYPE_INT, {.i64 = -1}, -1, INT_MAX},
      {NULL},
  };
  static const AVClass class = {
      .class_name = "cpucount",
      .item_name = av_default_item_name,
      .option = opts,
      .version = LIBAVUTIL_VERSION_INT,
  };
  const AVClass *pclass = &class;

  ret = av_opt_eval_int(&pclass, opts, arg, &count);

  if (!ret) {
    av_cpu_force_count(count);
  }

  return ret;
}

static void expand_filename_template(AVBPrint *bp, const char *template,
                                     struct tm *tm) {
  int c;

  while ((c = *(template ++))) {
    if (c == '%') {
      if (!(c = *(template ++)))
        break;
      switch (c) {
      case 'p':
        av_bprintf(bp, "%s", program_name);
        break;
      case 't':
        av_bprintf(bp, "%04d%02d%02d-%02d%02d%02d", tm->tm_year + 1900,
                   tm->tm_mon + 1, tm->tm_mday, tm->tm_hour, tm->tm_min,
                   tm->tm_sec);
        break;
      case '%':
        av_bprint_chars(bp, c, 1);
        break;
      }
    } else {
      av_bprint_chars(bp, c, 1);
    }
  }
}

static void log_callback_report(void *ptr, int level, const char *fmt,
                                va_list vl) {
  va_list vl2;
  char line[1024];
  static int print_prefix = 1;

  va_copy(vl2, vl);
  av_log_default_callback(ptr, level, fmt, vl);
  av_log_format_line(ptr, level, fmt, vl2, line, sizeof(line), &print_prefix);
  va_end(vl2);
  if (report_file_level >= level) {
    fputs(line, report_file);
    fflush(report_file);
  }
}

int init_report(const char *env, FILE **file) {
  char *filename_template = NULL;
  char *key, *val;
  int ret, count = 0;
  int prog_loglevel, envlevel = 0;
  time_t now;
  struct tm *tm;
  AVBPrint filename;

  if (report_file) /* already opened */
    return 0;
  time(&now);
  tm = localtime(&now);

  while (env && *env) {
    if ((ret = av_opt_get_key_value(&env, "=", ":", 0, &key, &val)) < 0) {
      if (count)
        av_log(NULL, AV_LOG_ERROR,
               "Failed to parse FFREPORT environment variable: %s\n",
               av_err2str(ret));
      break;
    }
    if (*env)
      env++;
    count++;
    if (!strcmp(key, "file")) {
      av_free(filename_template);
      filename_template = val;
      val = NULL;
    } else if (!strcmp(key, "level")) {
      char *tail;
      report_file_level = strtol(val, &tail, 10);
      if (*tail) {
        av_log(NULL, AV_LOG_FATAL, "Invalid report file level\n");
        av_free(key);
        av_free(val);
        av_free(filename_template);
        return AVERROR(EINVAL);
      }
      envlevel = 1;
    } else {
      av_log(NULL, AV_LOG_ERROR, "Unknown key '%s' in FFREPORT\n", key);
    }
    av_free(val);
    av_free(key);
  }

  av_bprint_init(&filename, 0, AV_BPRINT_SIZE_AUTOMATIC);
  expand_filename_template(&filename,
                           av_x_if_null(filename_template, "%p-%t.log"), tm);
  av_free(filename_template);
  if (!av_bprint_is_complete(&filename)) {
    av_log(NULL, AV_LOG_ERROR, "Out of memory building report file name\n");
    return AVERROR(ENOMEM);
  }

  prog_loglevel = av_log_get_level();
  if (!envlevel)
    report_file_level = FFMAX(report_file_level, prog_loglevel);

  report_file = fopen(filename.str, "w");
  if (!report_file) {
    int ret = AVERROR(errno);
    av_log(NULL, AV_LOG_ERROR, "Failed to open report \"%s\": %s\n",
           filename.str, strerror(errno));
    return ret;
  }
  av_log_set_callback(log_callback_report);
  av_log(NULL, AV_LOG_INFO,
         "%s started on %04d-%02d-%02d at %02d:%02d:%02d\n"
         "Report written to \"%s\"\n"
         "Log level: %d\n",
         program_name, tm->tm_year + 1900, tm->tm_mon + 1, tm->tm_mday,
         tm->tm_hour, tm->tm_min, tm->tm_sec, filename.str, report_file_level);
  av_bprint_finalize(&filename, NULL);

  if (file)
    *file = report_file;

  return 0;
}

int opt_report(void *optctx, const char *opt, const char *arg) {
  return init_report(NULL, NULL);
}

int opt_max_alloc(void *optctx, const char *opt, const char *arg) {
  char *tail;
  size_t max;

  max = strtol(arg, &tail, 10);
  if (*tail) {
    av_log(NULL, AV_LOG_FATAL, "Invalid max_alloc \"%s\".\n", arg);
    return AVERROR(EINVAL);
  }
  av_max_alloc(max);
  return 0;
}

int opt_loglevel(void *optctx, const char *opt, const char *arg) {
  const struct {
    const char *name;
    int level;
  } log_levels[] = {
      {"quiet", AV_LOG_QUIET},     {"panic", AV_LOG_PANIC},
      {"fatal", AV_LOG_FATAL},     {"error", AV_LOG_ERROR},
      {"warning", AV_LOG_WARNING}, {"info", AV_LOG_INFO},
      {"verbose", AV_LOG_VERBOSE}, {"debug", AV_LOG_DEBUG},
      {"trace", AV_LOG_TRACE},
  };
  const char *token;
  char *tail;
  int flags = av_log_get_flags();
  int level = av_log_get_level();
  int cmd, i = 0;

  av_assert0(arg);
  while (*arg) {
    token = arg;
    if (*token == '+' || *token == '-') {
      cmd = *token++;
    } else {
      cmd = 0;
    }
    if (!i && !cmd) {
      flags = 0; /* missing relative prefix, build absolute value */
    }
    if (av_strstart(token, "repeat", &arg)) {
      if (cmd == '-') {
        flags |= AV_LOG_SKIP_REPEATED;
      } else {
        flags &= ~AV_LOG_SKIP_REPEATED;
      }
    } else if (av_strstart(token, "level", &arg)) {
      if (cmd == '-') {
        flags &= ~AV_LOG_PRINT_LEVEL;
      } else {
        flags |= AV_LOG_PRINT_LEVEL;
      }
    } else {
      break;
    }
    i++;
  }
  if (!*arg) {
    goto end;
  } else if (*arg == '+') {
    arg++;
  } else if (!i) {
    flags = av_log_get_flags(); /* level value without prefix, reset flags */
  }

  for (i = 0; i < FF_ARRAY_ELEMS(log_levels); i++) {
    if (!strcmp(log_levels[i].name, arg)) {
      level = log_levels[i].level;
      goto end;
    }
  }

  level = strtol(arg, &tail, 10);
  if (*tail) {
    av_log(NULL, AV_LOG_FATAL,
           "Invalid loglevel \"%s\". "
           "Possible levels are numbers or:\n",
           arg);
    for (i = 0; i < FF_ARRAY_ELEMS(log_levels); i++)
      av_log(NULL, AV_LOG_FATAL, "\"%s\"\n", log_levels[i].name);
    return AVERROR(EINVAL);
  }

end:
  av_log_set_flags(flags);
  av_log_set_level(level);
  return 0;
}

static void print_device_list(const AVDeviceInfoList *device_list) {
  // print devices
  for (int i = 0; i < device_list->nb_devices; i++) {
    const AVDeviceInfo *device = device_list->devices[i];
    printf("%c %s [%s] (", device_list->default_device == i ? '*' : ' ',
           device->device_name, device->device_description);
    if (device->nb_media_types > 0) {
      for (int j = 0; j < device->nb_media_types; ++j) {
        const char *media_type =
            av_get_media_type_string(device->media_types[j]);
        if (j > 0)
          printf(", ");
        printf("%s", media_type ? media_type : "unknown");
      }
    } else {
      printf("none");
    }
    printf(")\n");
  }
}

static int print_device_sources(const AVInputFormat *fmt, AVDictionary *opts) {
  int ret;
  AVDeviceInfoList *device_list = NULL;

  if (!fmt || !fmt->priv_class ||
      !AV_IS_INPUT_DEVICE(fmt->priv_class->category))
    return AVERROR(EINVAL);

  printf("Auto-detected sources for %s:\n", fmt->name);
  if ((ret = avdevice_list_input_sources(fmt, NULL, opts, &device_list)) < 0) {
    printf("Cannot list sources: %s\n", av_err2str(ret));
    goto fail;
  }

  print_device_list(device_list);

fail:
  avdevice_free_list_devices(&device_list);
  return ret;
}

static int print_device_sinks(const AVOutputFormat *fmt, AVDictionary *opts) {
  int ret;
  AVDeviceInfoList *device_list = NULL;

  if (!fmt || !fmt->priv_class ||
      !AV_IS_OUTPUT_DEVICE(fmt->priv_class->category))
    return AVERROR(EINVAL);

  printf("Auto-detected sinks for %s:\n", fmt->name);
  if ((ret = avdevice_list_output_sinks(fmt, NULL, opts, &device_list)) < 0) {
    printf("Cannot list sinks: %s\n", av_err2str(ret));
    goto fail;
  }

  print_device_list(device_list);

fail:
  avdevice_free_list_devices(&device_list);
  return ret;
}

static int show_sinks_sources_parse_arg(const char *arg, char **dev,
                                        AVDictionary **opts) {
  int ret;
  if (arg) {
    char *opts_str = NULL;
    av_assert0(dev && opts);
    *dev = av_strdup(arg);
    if (!*dev)
      return AVERROR(ENOMEM);
    if ((opts_str = strchr(*dev, ','))) {
      *(opts_str++) = '\0';
      if (opts_str[0] &&
          ((ret = av_dict_parse_string(opts, opts_str, "=", ":", 0)) < 0)) {
        av_freep(dev);
        return ret;
      }
    }
  } else
    printf("\nDevice name is not provided.\n"
           "You can pass devicename[,opt1=val1[,opt2=val2...]] as an "
           "argument.\n\n");
  return 0;
}

int show_sources(void *optctx, const char *opt, const char *arg) {
  const AVInputFormat *fmt = NULL;
  char *dev = NULL;
  AVDictionary *opts = NULL;
  int ret = 0;
  int error_level = av_log_get_level();

  av_log_set_level(AV_LOG_WARNING);

  if ((ret = show_sinks_sources_parse_arg(arg, &dev, &opts)) < 0)
    goto fail;

  do {
    fmt = av_input_audio_device_next(fmt);
    if (fmt) {
      if (!strcmp(fmt->name, "lavfi"))
        continue; // it's pointless to probe lavfi
      if (dev && !av_match_name(dev, fmt->name))
        continue;
      print_device_sources(fmt, opts);
    }
  } while (fmt);
  do {
    fmt = av_input_video_device_next(fmt);
    if (fmt) {
      if (dev && !av_match_name(dev, fmt->name))
        continue;
      print_device_sources(fmt, opts);
    }
  } while (fmt);
fail:
  av_dict_free(&opts);
  av_free(dev);
  av_log_set_level(error_level);
  return ret;
}

int show_sinks(void *optctx, const char *opt, const char *arg) {
  const AVOutputFormat *fmt = NULL;
  char *dev = NULL;
  AVDictionary *opts = NULL;
  int ret = 0;
  int error_level = av_log_get_level();

  av_log_set_level(AV_LOG_WARNING);

  if ((ret = show_sinks_sources_parse_arg(arg, &dev, &opts)) < 0)
    goto fail;

  do {
    fmt = av_output_audio_device_next(fmt);
    if (fmt) {
      if (dev && !av_match_name(dev, fmt->name))
        continue;
      print_device_sinks(fmt, opts);
    }
  } while (fmt);
  do {
    fmt = av_output_video_device_next(fmt);
    if (fmt) {
      if (dev && !av_match_name(dev, fmt->name))
        continue;
      print_device_sinks(fmt, opts);
    }
  } while (fmt);
fail:
  av_dict_free(&opts);
  av_free(dev);
  av_log_set_level(error_level);
  return ret;
}
