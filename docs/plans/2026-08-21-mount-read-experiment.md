# Mount read-pattern experiment

## 1. Start the daemon
    SD_MOUNT_SMB_PASSWORD=spacedrive cargo run --bin sd-daemon
Note the SMB URL it logs.

## 2. Mount
    mkdir -p /tmp/sdmnt
    mount_smbfs -o nobrowse,soft "//spacedrive:spacedrive@127.0.0.1:PORT/spacedrive" /tmp/sdmnt

## 3. Start recording (also resets the cache so numbers are cold)
    sd-cli op mounts.trace_set --json '{"enabled":true}'
    sd-cli op mounts.cache_clear --json '{"include_disk":true}'

## 4. Do the thing
Open a large video from the mount in QuickTime or DaVinci.
Let it play, then scrub: jump to 25%, 75%, back to 10%.

## 5. Read the numbers
    sd-cli op mounts.read_trace
    sd-cli op mounts.cache_status

## 6. Stop
    sd-cli op mounts.trace_set --json '{"enabled":false}'
    umount /tmp/sdmnt

## What the numbers mean

common_read_bytes   The size macOS asks for. If it's 1 MiB or larger, the
                    client is already batching well and a native module
                    gains little on read size. If it's 64 KiB or smaller,
                    that's the case for FSKit in one number.

sequential/seeks    High sequential during playback means the client reads
                    straight through and our read-ahead has something to
                    work with.

rereads             Bytes the client asked for twice. Pure waste a native
                    module would not produce. If this is large, that's an
                    argument for going native on its own.

p95_micros          Tail latency per read. Stalls during playback show up
                    here before they show up in your eyes.

fetched vs served   From cache_status. served >> fetched means the cache is
                    carrying the session. If they track each other, the
                    cache is not helping and something upstream is wrong.

## Also worth running
The same file over WebDAV, for a side-by-side:
    mount_webdav -S http://127.0.0.1:7764/dav /tmp/sddav
by_frontend in the trace splits the two.
