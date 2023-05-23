FROM debian:bookworm

WORKDIR /srv

ENV CFLAGS='-mtune=generic -O3' \
	CPPFLAGS='-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0'

ADD https://gist.githubusercontent.com/HeavenVolkoff/ff7b77b9087f956b8df944772e93c071/raw \
	/etc/apt/apt.conf.d/99docker-apt-config

RUN rm -f /etc/apt/apt.conf.d/docker-clean; \
	echo 'Binary::apt::APT::Keep-Downloaded-Packages "true";' > /etc/apt/apt.conf.d/keep-cache

RUN --mount=type=cache,target=/var/cache/apt --mount=type=cache,target=/var/lib/apt \
  apt-get update && apt-get upgrade && apt-get install -y autoconf autoconf-archive autogen automake \
	bison bzip2 clang cmake curl cvs ed ed flex g++ gcc git gperf libtool \
	libtool-bin make meson nasm p7zip-full pax pkg-config ragel subversion \
	texinfo unzip wget yasm zlib1g-dev

ADD --chmod=755 https://raw.githubusercontent.com/rdp/ffmpeg-windows-build-helpers/master/patches/mingw-w64-build-r22.local \
	./

RUN nice ./mingw-w64-build-r22.local --gcc-ver=10.2.0 --mingw-w64-ver=9.0.0 \
	--default-configure --cpu-count="$(nproc)" --disable-shared --clean-build \
	--verbose --allow-overwrite --threads=winpthreads --build-type=win64

ENV PATH="/srv/mingw-w64-x86_64/bin:$PATH"

