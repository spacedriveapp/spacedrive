-- CreateTable
CREATE TABLE "ffmpeg_data" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "formats" TEXT NOT NULL,
    "bit_rate" BLOB NOT NULL,
    "duration" BLOB,
    "start_time" BLOB,
    "title" TEXT,
    "creation_time" DATETIME,
    "date" DATETIME,
    "album_artist" TEXT,
    "disc" TEXT,
    "track" TEXT,
    "album" TEXT,
    "artist" TEXT,
    "metadata" BLOB,
    "object_id" INTEGER NOT NULL,
    CONSTRAINT "ffmpeg_data_object_id_fkey" FOREIGN KEY ("object_id") REFERENCES "object" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "ffmpeg_media_chapter" (
    "chapter_id" INTEGER NOT NULL,
    "start" BLOB NOT NULL,
    "end" BLOB NOT NULL,
    "time_base_den" INTEGER NOT NULL,
    "time_base_num" INTEGER NOT NULL,
    "title" TEXT,
    "metadata" BLOB,
    "ffmpeg_data_id" INTEGER NOT NULL,

    PRIMARY KEY ("ffmpeg_data_id", "chapter_id"),
    CONSTRAINT "ffmpeg_media_chapter_ffmpeg_data_id_fkey" FOREIGN KEY ("ffmpeg_data_id") REFERENCES "ffmpeg_data" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "ffmpeg_media_program" (
    "program_id" INTEGER NOT NULL,
    "name" TEXT,
    "metadata" BLOB,
    "ffmpeg_data_id" INTEGER NOT NULL,

    PRIMARY KEY ("ffmpeg_data_id", "program_id"),
    CONSTRAINT "ffmpeg_media_program_ffmpeg_data_id_fkey" FOREIGN KEY ("ffmpeg_data_id") REFERENCES "ffmpeg_data" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "ffmpeg_media_stream" (
    "stream_id" INTEGER NOT NULL,
    "name" TEXT,
    "aspect_ratio_num" INTEGER NOT NULL,
    "aspect_ratio_den" INTEGER NOT NULL,
    "frames_per_second_num" INTEGER NOT NULL,
    "frames_per_second_den" INTEGER NOT NULL,
    "time_base_real_den" INTEGER NOT NULL,
    "time_base_real_num" INTEGER NOT NULL,
    "dispositions" TEXT,
    "title" TEXT,
    "encoder" TEXT,
    "language" TEXT,
    "duration" BLOB,
    "metadata" BLOB,
    "program_id" INTEGER NOT NULL,
    "ffmpeg_data_id" INTEGER NOT NULL,

    PRIMARY KEY ("ffmpeg_data_id", "program_id", "stream_id"),
    CONSTRAINT "ffmpeg_media_stream_ffmpeg_data_id_program_id_fkey" FOREIGN KEY ("ffmpeg_data_id", "program_id") REFERENCES "ffmpeg_media_program" ("ffmpeg_data_id", "program_id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "ffmpeg_media_codec" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "kind" TEXT,
    "sub_kind" TEXT,
    "tag" TEXT,
    "name" TEXT,
    "profile" TEXT,
    "bit_rate" INTEGER NOT NULL,
    "stream_id" INTEGER NOT NULL,
    "program_id" INTEGER NOT NULL,
    "ffmpeg_data_id" INTEGER NOT NULL,
    CONSTRAINT "ffmpeg_media_codec_ffmpeg_data_id_program_id_stream_id_fkey" FOREIGN KEY ("ffmpeg_data_id", "program_id", "stream_id") REFERENCES "ffmpeg_media_stream" ("ffmpeg_data_id", "program_id", "stream_id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "ffmpeg_media_video_props" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "pixel_format" TEXT,
    "color_range" TEXT,
    "bits_per_channel" INTEGER,
    "color_space" TEXT,
    "color_primaries" TEXT,
    "color_transfer" TEXT,
    "field_order" TEXT,
    "chroma_location" TEXT,
    "width" INTEGER NOT NULL,
    "height" INTEGER NOT NULL,
    "aspect_ratio_num" INTEGER,
    "aspect_ratio_Den" INTEGER,
    "properties" TEXT,
    "codec_id" INTEGER NOT NULL,
    CONSTRAINT "ffmpeg_media_video_props_codec_id_fkey" FOREIGN KEY ("codec_id") REFERENCES "ffmpeg_media_codec" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateTable
CREATE TABLE "ffmpeg_media_audio_props" (
    "id" INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    "delay" INTEGER NOT NULL,
    "padding" INTEGER NOT NULL,
    "sample_rate" INTEGER,
    "sample_format" TEXT,
    "bit_per_sample" INTEGER,
    "channel_layout" TEXT,
    "codec_id" INTEGER NOT NULL,
    CONSTRAINT "ffmpeg_media_audio_props_codec_id_fkey" FOREIGN KEY ("codec_id") REFERENCES "ffmpeg_media_codec" ("id") ON DELETE CASCADE ON UPDATE CASCADE
);

-- CreateIndex
CREATE UNIQUE INDEX "ffmpeg_data_object_id_key" ON "ffmpeg_data"("object_id");

-- CreateIndex
CREATE UNIQUE INDEX "ffmpeg_media_codec_ffmpeg_data_id_program_id_stream_id_key" ON "ffmpeg_media_codec"("ffmpeg_data_id", "program_id", "stream_id");

-- CreateIndex
CREATE UNIQUE INDEX "ffmpeg_media_video_props_codec_id_key" ON "ffmpeg_media_video_props"("codec_id");

-- CreateIndex
CREATE UNIQUE INDEX "ffmpeg_media_audio_props_codec_id_key" ON "ffmpeg_media_audio_props"("codec_id");
