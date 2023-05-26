FROM vvasconcellos/mingw:10.2_9.0

WORKDIR /srv

RUN rustup toolchain install stable --profile minimal

RUN rustup target add x86_64-pc-windows-gnu

RUN cargo install cargo-c --features=vendored-openssl

COPY --chmod=755 ffmpeg-build-windows.sh /srv/
COPY *.patch *.diff /srv/

RUN ./ffmpeg-build-windows.sh
