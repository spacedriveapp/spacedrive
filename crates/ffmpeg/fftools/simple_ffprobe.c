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

#include "libavcodec/avcodec.h"
#include "libavformat/avformat.h"
#include "libavutil/dict.h"
#include "libavutil/opt.h"

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

/** Extract a stream info and print it */
static void dump_stream_format(const AVFormatContext *ic, int i, int index) {
  char buf[256];
  const AVStream *st = ic->streams[i];
  const AVDictionaryEntry *lang =
      av_dict_get(st->metadata, "language", NULL, 0);
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
  avcodec_string(buf, sizeof(buf), avctx, 0);
  avcodec_free_context(&avctx);

  // Stream header
  av_log(NULL, AV_LOG_INFO, "  Stream #%d:%d", index, i);

  // Stream id
  av_log(NULL, AV_LOG_INFO, "[0x%x]", st->id);

  // Print language
  if (lang)
    av_log(NULL, AV_LOG_INFO, "(%s)", lang->value);

  // Stream codec type/info
  av_log(NULL, AV_LOG_INFO, ": %s", buf);

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

  // Side data is kind of irelevant rn
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
