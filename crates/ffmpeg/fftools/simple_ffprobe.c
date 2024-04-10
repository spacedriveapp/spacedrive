/*
 * This file was made using various parts of FFmpeg.
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

#include <errno.h>
#include <inttypes.h>
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "libavcodec/avcodec.h"
#include "libavcodec/codec_id.h"
#include "libavcodec/defs.h"
#include "libavformat/avformat.h"
#include "libavutil/avutil.h"
#include "libavutil/bprint.h"
#include "libavutil/channel_layout.h"
#include "libavutil/dict.h"
#include "libavutil/error.h"
#include "libavutil/log.h"
#include "libavutil/macros.h"
#include "libavutil/mathematics.h"
#include "libavutil/mem.h"
#include "libavutil/opt.h"
#include "libavutil/pixdesc.h"
#include "libavutil/pixfmt.h"
#include "libavutil/rational.h"
#include "libavutil/samplefmt.h"

/** Print the metadata dictionary */
static void dump_metadata(void *ctx, const AVDictionary *m,
                          const char *indent) {
  if (m && !(av_dict_count(m) == 1 && av_dict_get(m, "language", NULL, 0))) {
    const AVDictionaryEntry *tag = NULL;

    av_log(ctx, AV_LOG_INFO, "%sMetadata:\n", indent);
    while ((tag = av_dict_iterate(m, tag)))
      if (strcmp("language", tag->key)) {
        const char *p = tag->value;
        av_log(ctx, AV_LOG_INFO, "%s  %-16s: ", indent, tag->key);
        // Some weird print math
        while (*p) {
          size_t len = strcspn(p, "\x8\xa\xb\xc\xd");
          av_log(ctx, AV_LOG_INFO, "%.*s", (int)(FFMIN(255, len)), p);
          p += len;
          if (*p == 0xd)
            av_log(ctx, AV_LOG_INFO, " ");
          if (*p == 0xa)
            av_log(ctx, AV_LOG_INFO, "\n%s  %-16s: ", indent, "");
          if (*p)
            p++;
        }
        av_log(ctx, AV_LOG_INFO, "\n");
      }
  }
}

/** Print fps */
static void print_fps(double d, const char *postfix) {
  uint64_t v = lrintf(d * 100);
  if (!v)
    av_log(NULL, AV_LOG_INFO, "%1.4f %s", d, postfix);
  else if (v % 100)
    av_log(NULL, AV_LOG_INFO, "%3.2f %s", d, postfix);
  else if (v % (100 * 1000))
    av_log(NULL, AV_LOG_INFO, "%1.0f %s", d, postfix);
  else
    av_log(NULL, AV_LOG_INFO, "%1.0fk %s", d / 1000, postfix);
}

static int64_t get_bit_rate(AVCodecContext *ctx) {
  int64_t bit_rate;
  int bits_per_sample;

  switch (ctx->codec_type) {
  case AVMEDIA_TYPE_VIDEO:
  case AVMEDIA_TYPE_DATA:
  case AVMEDIA_TYPE_SUBTITLE:
  case AVMEDIA_TYPE_ATTACHMENT:
    bit_rate = ctx->bit_rate;
    break;
  case AVMEDIA_TYPE_AUDIO:
    bits_per_sample = av_get_bits_per_sample(ctx->codec_id);
    if (bits_per_sample) {
      bit_rate = ctx->sample_rate * (int64_t)ctx->ch_layout.nb_channels;
      if (bit_rate > INT64_MAX / bits_per_sample) {
        bit_rate = 0;
      } else
        bit_rate *= bits_per_sample;
    } else
      bit_rate = ctx->bit_rate;
    break;
  default:
    bit_rate = 0;
    break;
  }
  return bit_rate;
}

static const char *unknown_if_null(const char *str) {
  return str ? str : "unknown";
}

void print_codec(AVCodecContext *enc) {
  const char *codec_type;
  const char *codec_name;
  const char *profile = NULL;
  int64_t bitrate;
  int new_line = 0;
  AVRational display_aspect_ratio;
  const char *separator =
      enc->dump_separator ? (const char *)enc->dump_separator : ", ";
  const char *str;

  codec_type = av_get_media_type_string(enc->codec_type);
  codec_name = avcodec_get_name(enc->codec_id);
  profile = avcodec_profile_name(enc->codec_id, enc->profile);

  av_log(NULL, AV_LOG_INFO, "%s: %s", codec_type ? codec_type : "unknown",
         codec_name);

  if (enc->codec && strcmp(enc->codec->name, codec_name))
    av_log(NULL, AV_LOG_INFO, " (%s)", enc->codec->name);

  if (profile)
    av_log(NULL, AV_LOG_INFO, " (%s)", profile);
  if (enc->codec_type == AVMEDIA_TYPE_VIDEO &&
      av_log_get_level() >= AV_LOG_VERBOSE && enc->refs)
    av_log(NULL, AV_LOG_INFO, ", %d reference frame%s", enc->refs,
           enc->refs > 1 ? "s" : "");

  if (enc->codec_tag)
    av_log(NULL, AV_LOG_INFO, " (%s / 0x%04X)", av_fourcc2str(enc->codec_tag),
           enc->codec_tag);

  switch (enc->codec_type) {
  case AVMEDIA_TYPE_VIDEO: {
    av_log(NULL, AV_LOG_INFO, "%s%s", separator,
           enc->pix_fmt == AV_PIX_FMT_NONE
               ? "none"
               : unknown_if_null(av_get_pix_fmt_name(enc->pix_fmt)));

    if (enc->bits_per_raw_sample && enc->pix_fmt != AV_PIX_FMT_NONE &&
        enc->bits_per_raw_sample <
            av_pix_fmt_desc_get(enc->pix_fmt)->comp[0].depth)
      av_log(NULL, AV_LOG_INFO, "%d bpc, ", enc->bits_per_raw_sample);
    if (enc->color_range != AVCOL_RANGE_UNSPECIFIED &&
        (str = av_color_range_name(enc->color_range)))
      av_log(NULL, AV_LOG_INFO, "%s, ", str);

    if (enc->colorspace != AVCOL_SPC_UNSPECIFIED ||
        enc->color_primaries != AVCOL_PRI_UNSPECIFIED ||
        enc->color_trc != AVCOL_TRC_UNSPECIFIED) {
      const char *col = unknown_if_null(av_color_space_name(enc->colorspace));
      const char *pri =
          unknown_if_null(av_color_primaries_name(enc->color_primaries));
      const char *trc = unknown_if_null(av_color_transfer_name(enc->color_trc));
      if (strcmp(col, pri) || strcmp(col, trc)) {
        new_line = 1;
        av_log(NULL, AV_LOG_INFO, "%s/%s/%s, ", col, pri, trc);
      } else
        av_log(NULL, AV_LOG_INFO, "%s, ", col);
    }

    if (enc->field_order != AV_FIELD_UNKNOWN) {
      const char *field_order = "progressive";
      if (enc->field_order == AV_FIELD_TT)
        field_order = "top first";
      else if (enc->field_order == AV_FIELD_BB)
        field_order = "bottom first";
      else if (enc->field_order == AV_FIELD_TB)
        field_order = "top coded first (swapped)";
      else if (enc->field_order == AV_FIELD_BT)
        field_order = "bottom coded first (swapped)";

      av_log(NULL, AV_LOG_INFO, "%s, ", field_order);
    }

    if (av_log_get_level() >= AV_LOG_VERBOSE &&
        enc->chroma_sample_location != AVCHROMA_LOC_UNSPECIFIED &&
        (str = av_chroma_location_name(enc->chroma_sample_location)))
      av_log(NULL, AV_LOG_INFO, "%s, ", str);
  }

    if (enc->width) {
      av_log(NULL, AV_LOG_INFO, "%s%dx%d", new_line ? separator : ", ",
             enc->width, enc->height);

      if (av_log_get_level() >= AV_LOG_VERBOSE && enc->coded_width &&
          enc->coded_height &&
          (enc->width != enc->coded_width || enc->height != enc->coded_height))
        av_log(NULL, AV_LOG_INFO, " (%dx%d)", enc->coded_width,
               enc->coded_height);

      if (enc->sample_aspect_ratio.num) {
        av_reduce(&display_aspect_ratio.num, &display_aspect_ratio.den,
                  enc->width * (int64_t)enc->sample_aspect_ratio.num,
                  enc->height * (int64_t)enc->sample_aspect_ratio.den,
                  1024 * 1024);
        av_log(NULL, AV_LOG_INFO, " [SAR %d:%d DAR %d:%d]",
               enc->sample_aspect_ratio.num, enc->sample_aspect_ratio.den,
               display_aspect_ratio.num, display_aspect_ratio.den);
      }
      if (av_log_get_level() >= AV_LOG_DEBUG) {
        int g = av_gcd(enc->time_base.num, enc->time_base.den);
        av_log(NULL, AV_LOG_INFO, ", %d/%d", enc->time_base.num / g,
               enc->time_base.den / g);
      }
    }

    if (enc->properties & FF_CODEC_PROPERTY_CLOSED_CAPTIONS)
      av_log(NULL, AV_LOG_INFO, ", Closed Captions");
    if (enc->properties & FF_CODEC_PROPERTY_FILM_GRAIN)
      av_log(NULL, AV_LOG_INFO, ", Film Grain");
    if (enc->properties & FF_CODEC_PROPERTY_LOSSLESS)
      av_log(NULL, AV_LOG_INFO, ", lossless");

    break;
  case AVMEDIA_TYPE_AUDIO:
    av_log(NULL, AV_LOG_INFO, "%s", separator);

    if (enc->sample_rate) {
      av_log(NULL, AV_LOG_INFO, "%d Hz, ", enc->sample_rate);
    }

    char *ret = NULL;
    AVBPrint bprint;
    av_bprint_init(&bprint, 0, AV_BPRINT_SIZE_UNLIMITED);
    av_channel_layout_describe_bprint(&enc->ch_layout, &bprint);
    av_bprint_finalize(&bprint, &ret);
    av_log(NULL, AV_LOG_INFO, "%s", ret);

    if (enc->sample_fmt != AV_SAMPLE_FMT_NONE &&
        (str = av_get_sample_fmt_name(enc->sample_fmt))) {
      av_log(NULL, AV_LOG_INFO, ", %s", str);
    }
    if (enc->bits_per_raw_sample > 0 &&
        enc->bits_per_raw_sample !=
            av_get_bytes_per_sample(enc->sample_fmt) * 8)
      av_log(NULL, AV_LOG_INFO, " (%d bit)", enc->bits_per_raw_sample);
    if (av_log_get_level() >= AV_LOG_VERBOSE) {
      if (enc->initial_padding)
        av_log(NULL, AV_LOG_INFO, ", delay %d", enc->initial_padding);
      if (enc->trailing_padding)
        av_log(NULL, AV_LOG_INFO, ", padding %d", enc->trailing_padding);
    }
    break;
  case AVMEDIA_TYPE_DATA:
    if (av_log_get_level() >= AV_LOG_DEBUG) {
      int g = av_gcd(enc->time_base.num, enc->time_base.den);
      if (g)
        av_log(NULL, AV_LOG_INFO, ", %d/%d", enc->time_base.num / g,
               enc->time_base.den / g);
    }
    break;
  case AVMEDIA_TYPE_SUBTITLE:
    if (enc->width)
      av_log(NULL, AV_LOG_INFO, ", %dx%d", enc->width, enc->height);
    break;
  default:
    return;
  }

  bitrate = get_bit_rate(enc);
  if (bitrate != 0) {
    av_log(NULL, AV_LOG_INFO, ", %" PRId64 " kb/s", bitrate / 1000);
  } else if (enc->rc_max_rate > 0) {
    av_log(NULL, AV_LOG_INFO, ", max. %" PRId64 " kb/s",
           enc->rc_max_rate / 1000);
  }
}

/** Extract a stream info and print it */
static void dump_stream_format(const AVFormatContext *ic, int i, int index) {
  const AVStream *st = ic->streams[i];
  const char *separator = (const char *)ic->dump_separator;
  AVCodecContext *avctx;
  int ret;

  // Get codec type and info
  avctx = avcodec_alloc_context3(NULL);
  if (!avctx)
    return;

  ret = avcodec_parameters_to_context(avctx, st->codecpar);
  if (ret < 0) {
    avcodec_free_context(&avctx);
    return;
  }

  if (separator)
    av_opt_set(avctx, "dump_separator", separator, 0);

  // Stream header
  av_log(NULL, AV_LOG_INFO, "  Stream #%d:%d", index, i);

  // Stream id
  av_log(NULL, AV_LOG_INFO, "[0x%x]", st->id);

  // Stream codec type/info
  av_log(NULL, AV_LOG_INFO, ": ");
  print_codec(avctx);
  avcodec_free_context(&avctx);

  // Stream Sample Aspect Ratio (SAR) and Display Aspect Ratio (DAR)
  if (st->sample_aspect_ratio.num &&
      av_cmp_q(st->sample_aspect_ratio, st->codecpar->sample_aspect_ratio)) {
    AVRational display_aspect_ratio;
    av_reduce(&display_aspect_ratio.num, &display_aspect_ratio.den,
              st->codecpar->width * (int64_t)st->sample_aspect_ratio.num,
              st->codecpar->height * (int64_t)st->sample_aspect_ratio.den,
              1024 * 1024);
    av_log(NULL, AV_LOG_INFO, ", SAR %d:%d DAR %d:%d",
           st->sample_aspect_ratio.num, st->sample_aspect_ratio.den,
           display_aspect_ratio.num, display_aspect_ratio.den);
  }

  // Video stream FPS and some other time metrics
  if (st->codecpar->codec_type == AVMEDIA_TYPE_VIDEO) {
    int fps = st->avg_frame_rate.den && st->avg_frame_rate.num;
    int tbr = st->r_frame_rate.den && st->r_frame_rate.num;
    int tbn = st->time_base.den && st->time_base.num;

    if (fps || tbr || tbn)
      av_log(NULL, AV_LOG_INFO, "%s", separator);

    if (fps)
      print_fps(av_q2d(st->avg_frame_rate), tbr || tbn ? "fps, " : "fps");
    if (tbr)
      print_fps(av_q2d(st->r_frame_rate), tbn ? "tbr, " : "tbr");
    if (tbn)
      print_fps(1 / av_q2d(st->time_base), "tbn");
  }

  // Stream dispositions
  if (st->disposition & AV_DISPOSITION_DEFAULT)
    av_log(NULL, AV_LOG_INFO, " (default)");
  if (st->disposition & AV_DISPOSITION_DUB)
    av_log(NULL, AV_LOG_INFO, " (dub)");
  if (st->disposition & AV_DISPOSITION_ORIGINAL)
    av_log(NULL, AV_LOG_INFO, " (original)");
  if (st->disposition & AV_DISPOSITION_COMMENT)
    av_log(NULL, AV_LOG_INFO, " (comment)");
  if (st->disposition & AV_DISPOSITION_LYRICS)
    av_log(NULL, AV_LOG_INFO, " (lyrics)");
  if (st->disposition & AV_DISPOSITION_KARAOKE)
    av_log(NULL, AV_LOG_INFO, " (karaoke)");
  if (st->disposition & AV_DISPOSITION_FORCED)
    av_log(NULL, AV_LOG_INFO, " (forced)");
  if (st->disposition & AV_DISPOSITION_HEARING_IMPAIRED)
    av_log(NULL, AV_LOG_INFO, " (hearing impaired)");
  if (st->disposition & AV_DISPOSITION_VISUAL_IMPAIRED)
    av_log(NULL, AV_LOG_INFO, " (visual impaired)");
  if (st->disposition & AV_DISPOSITION_CLEAN_EFFECTS)
    av_log(NULL, AV_LOG_INFO, " (clean effects)");
  if (st->disposition & AV_DISPOSITION_ATTACHED_PIC)
    av_log(NULL, AV_LOG_INFO, " (attached pic)");
  if (st->disposition & AV_DISPOSITION_TIMED_THUMBNAILS)
    av_log(NULL, AV_LOG_INFO, " (timed thumbnails)");
  if (st->disposition & AV_DISPOSITION_CAPTIONS)
    av_log(NULL, AV_LOG_INFO, " (captions)");
  if (st->disposition & AV_DISPOSITION_DESCRIPTIONS)
    av_log(NULL, AV_LOG_INFO, " (descriptions)");
  if (st->disposition & AV_DISPOSITION_METADATA)
    av_log(NULL, AV_LOG_INFO, " (metadata)");
  if (st->disposition & AV_DISPOSITION_DEPENDENT)
    av_log(NULL, AV_LOG_INFO, " (dependent)");
  if (st->disposition & AV_DISPOSITION_STILL_IMAGE)
    av_log(NULL, AV_LOG_INFO, " (still image)");
  if (st->disposition & AV_DISPOSITION_NON_DIEGETIC)
    av_log(NULL, AV_LOG_INFO, " (non-diegetic)");
  av_log(NULL, AV_LOG_INFO, "\n");

  // Stream metadata
  dump_metadata(NULL, st->metadata, "    ");

  // Side data is kind of irelevant
  // Check here to see what it includes:
  //  https://github.com/FFmpeg/FFmpeg/blob/n6.1.1/libavformat/dump.c#L430-L508
  // dump_sidedata(NULL, st, "    ");
}

/** Extract media info and print it */
void dump_format(AVFormatContext *ic, const char *url) {
  int i, index = 0;

  // Keep track of each printed stream
  uint8_t *printed = ic->nb_streams ? av_mallocz(ic->nb_streams) : NULL;
  if (!printed)
    return;

  // Media header
  av_log(NULL, AV_LOG_INFO, "%s #%d, %s, %s '%s':\n", "Input", index,
         ic->iformat->name, "from", url);

  // Media metadata
  dump_metadata(NULL, ic->metadata, "  ");

  // Duration
  av_log(NULL, AV_LOG_INFO, "  Duration: ");
  if (ic->duration != AV_NOPTS_VALUE) {
    int64_t hours, mins, secs, us;
    int64_t duration =
        ic->duration + (ic->duration <= INT64_MAX - 5000 ? 5000 : 0);
    secs = duration / AV_TIME_BASE;
    us = duration % AV_TIME_BASE;
    mins = secs / 60;
    secs %= 60;
    hours = mins / 60;
    mins %= 60;
    av_log(NULL, AV_LOG_INFO,
           "%02" PRId64 ":%02" PRId64 ":%02" PRId64 ".%02" PRId64 "", hours,
           mins, secs, (100 * us) / AV_TIME_BASE);
  } else {
    av_log(NULL, AV_LOG_INFO, "N/A");
  }

  // Start time
  if (ic->start_time != AV_NOPTS_VALUE) {
    int secs, us;
    av_log(NULL, AV_LOG_INFO, ", start: ");
    secs = llabs(ic->start_time / AV_TIME_BASE);
    us = llabs(ic->start_time % AV_TIME_BASE);
    av_log(NULL, AV_LOG_INFO, "%s%d.%06d", ic->start_time >= 0 ? "" : "-", secs,
           (int)av_rescale(us, 1000000, AV_TIME_BASE));
  }

  // Bitrate
  av_log(NULL, AV_LOG_INFO, ", bitrate: ");
  if (ic->bit_rate)
    av_log(NULL, AV_LOG_INFO, "%" PRId64 " kb/s", ic->bit_rate / 1000);
  else
    av_log(NULL, AV_LOG_INFO, "N/A");
  av_log(NULL, AV_LOG_INFO, "\n");

  // Chapters
  if (ic->nb_chapters) {
    av_log(NULL, AV_LOG_INFO, "  Chapters:\n");
    for (i = 0; i < ic->nb_chapters; i++) {
      const AVChapter *ch = ic->chapters[i];
      av_log(NULL, AV_LOG_INFO, "    Chapter #%d:%d: ", index, i);
      av_log(NULL, AV_LOG_INFO, "start %f, ",
             ch->start * av_q2d(ch->time_base));
      av_log(NULL, AV_LOG_INFO, "end %f\n", ch->end * av_q2d(ch->time_base));

      dump_metadata(NULL, ch->metadata, "      ");
    }
  }

  // Programs
  if (ic->nb_programs) {
    int j, k, total = 0;
    for (j = 0; j < ic->nb_programs; j++) {
      const AVProgram *program = ic->programs[j];
      const AVDictionaryEntry *name =
          av_dict_get(program->metadata, "name", NULL, 0);
      av_log(NULL, AV_LOG_INFO, "  Program %d %s\n", program->id,
             name ? name->value : "");
      dump_metadata(NULL, program->metadata, "    ");
      for (k = 0; k < program->nb_stream_indexes; k++) {
        dump_stream_format(ic, program->stream_index[k], index);
        printed[program->stream_index[k]] = 1;
      }
      total += program->nb_stream_indexes;
    }
    if (total < ic->nb_streams)
      av_log(NULL, AV_LOG_INFO, "  No Program\n");
  }

  // Streams
  for (i = 0; i < ic->nb_streams; i++)
    if (!printed[i])
      dump_stream_format(ic, i, index);

  av_free(printed);
}

/** Some basic logic to open a media file */
static int open_input_file(AVFormatContext **fmt_ctx, const char *filename) {
  int err, scan_all_pmts_set = 0;
  AVDictionary *format_opts = NULL;

  // Allocate ffmpeg internal context
  *fmt_ctx = avformat_alloc_context();
  if (!fmt_ctx)
    return AVERROR(ENOMEM);

  // Some MPEGTS specific option (copied and pasted from ffprobe)
  if (!av_dict_get(format_opts, "scan_all_pmts", NULL, AV_DICT_MATCH_CASE)) {
    av_dict_set(&format_opts, "scan_all_pmts", "1", AV_DICT_DONT_OVERWRITE);
    scan_all_pmts_set = 1;
  }

  // Open and parse the media file
  avformat_close_input(fmt_ctx);
  err = avformat_open_input(fmt_ctx, filename, NULL, &format_opts);
  if (err < 0) {
    return err;
  }

  // Again MPEGTS specific option
  if (scan_all_pmts_set)
    av_dict_set(&format_opts, "scan_all_pmts", NULL, AV_DICT_MATCH_CASE);

  // Automatically find stream info
  err = avformat_find_stream_info(*fmt_ctx, NULL);
  if (err < 0) {
    return err;
  }

  return 0;
}

int main(int argc, char **argv) {
  int err = 0;
  AVFormatContext *fmt_ctx = NULL;
  const char *filename;

  // Basic input validation and help text
  if (argc < 2 || argv[1][0] == '-') {
    printf("Usage: %s <file_path>\n", argv[0]);
    return 1;
  }

  // Open file
  filename = argv[1];
  err = open_input_file(&fmt_ctx, filename);
  if (err < 0) {
    printf("[ERROR] Failed to open input file: %s\n", argv[1]);
    return 1;
  }

  // Print media info
  dump_format(fmt_ctx, filename);

  // Cleanup
  avformat_close_input(&fmt_ctx);

  return 0;
}
