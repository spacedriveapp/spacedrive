# Makefile for compiling ffprobe

# Compiler and flags
CC = clang
CFLAGS = -std=c17 -Wall -I../../../apps/.deps/include

# Source files
SRCS = cmdutils.c opt_common.c ffprobe.c

# Object files
OBJS = $(SRCS:.c=.o)

# Output binary
TARGET = ffprobe

# Linker flags for system libraries
LDFLAGS = -L../../../apps/.deps/libs -lm -lswresample -lavformat -lavutil -lswscale -lavcodec -lavfilter -lavdevice

all: $(TARGET)

$(TARGET): $(OBJS)
	$(CC) $(CFLAGS) $(OBJS) -o $(TARGET) $(LDFLAGS)

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJS) $(TARGET)
