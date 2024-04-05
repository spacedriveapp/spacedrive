# Makefile for compiling simple_ffprobe
TARGET = simple_ffprobe
SRCS   = $(TARGET).c
OBJS   = $(SRCS:.c=.o)

# Compiler flags
CC = clang
CFLAGS = -std=c17 -Wall -I../../../apps/.deps/include

# Linker flags
LDFLAGS = -L../../../apps/.deps/libs -lm -lavformat -lavutil -lavcodec

all: $(TARGET)

$(TARGET): $(OBJS)
	$(CC) $(CFLAGS) $(OBJS) -o $(TARGET) $(LDFLAGS)

%.o: %.c
	$(CC) $(CFLAGS) -c $< -o $@

clean:
	rm -f $(OBJS) $(TARGET)
