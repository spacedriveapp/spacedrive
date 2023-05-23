#!/usr/bin/env bash

set -e          # exit immediate if an error occurs in a pipeline
set -E          # make commands inherit ERR trap
set -u          # don't allow not set variables to be utilized
set -o pipefail # trace ERR through pipes
set -o errtrace # trace ERR through 'time command' and other functions

# helper methods for downloading and building projects that can take generic input

reset_cflags() {
  export CFLAGS=$original_cflags
}

reset_cppflags() {
  export CPPFLAGS=$original_cppflags
}

do_svn_checkout() {
  repo_url="$1"
  to_dir="$2"
  desired_revision="$3"
  if [ ! -d $to_dir ]; then
    echo "svn checking out to $to_dir"
    if [[ -z $desired_revision ]]; then
      svn checkout $repo_url $to_dir.tmp --non-interactive --trust-server-cert || exit 1
    else
      svn checkout -r $desired_revision $repo_url $to_dir.tmp || exit 1
    fi
    mv $to_dir.tmp $to_dir
  else
    cd $to_dir
    echo "not svn Updating $to_dir since usually svn repo's aren't updated frequently enough..."
    # XXX accomodate for desired revision here if I ever uncomment the next line...
    # svn up
    cd ..
  fi
}

do_git_checkout() {
  local repo_url="$1"
  local to_dir="$2"
  if [[ -z $to_dir ]]; then
    to_dir=$(basename $repo_url | sed s/\.git/_git/) # http://y/abc.git -> abc_git
  fi
  local desired_branch="$3"
  if [ ! -d $to_dir ]; then
    echo "Downloading (via git clone) $to_dir from $repo_url"
    rm -rf $to_dir.tmp # just in case it was interrupted previously...
    git clone $repo_url $to_dir.tmp --recurse-submodules || exit 1
    # prevent partial checkouts by renaming it only after success
    mv $to_dir.tmp $to_dir
    echo "done git cloning to $to_dir"
    cd $to_dir
  else
    cd $to_dir
    if [[ $git_get_latest == "y" ]]; then
      git fetch # want this for later...
    else
      echo "not doing git get latest pull for latest code $to_dir" # too slow'ish...
    fi
  fi

  # reset will be useless if they didn't git_get_latest but pretty fast so who cares...plus what if they changed branches? :)
  old_git_version=$(git rev-parse HEAD)
  if [[ -z $desired_branch ]]; then
    desired_branch="origin/master"
  fi
  echo "doing git checkout $desired_branch"
  git -c 'advice.detachedHead=false' checkout "$desired_branch" || (git_hard_reset && git -c 'advice.detachedHead=false' checkout "$desired_branch") || (git reset --hard "$desired_branch") || exit 1 # can't just use merge -f because might "think" patch files already applied when their changes have been lost, etc...
  # vmaf on 16.04 needed that weird reset --hard? huh?
  if git show-ref --verify --quiet "refs/remotes/origin/$desired_branch"; then # $desired_branch is actually a branch, not a tag or commit
    git merge "origin/$desired_branch" || exit 1                               # get incoming changes to a branch
  fi
  new_git_version=$(git rev-parse HEAD)
  if [[ $old_git_version != "$new_git_version" ]]; then
    echo "got upstream changes, forcing re-configure. Doing git clean"
    git_hard_reset
  else
    echo "fetched no code changes, not forcing reconfigure for that..."
  fi
  cd ..
}

git_hard_reset() {
  git reset --hard # throw away results of patch files
  git clean -fx    # throw away local changes; 'already_*' and bak-files for instance.
}

get_small_touchfile_name() { # have to call with assignment like a=$(get_small...)
  local beginning="$1"
  local extra_stuff="$2"
  local touch_name="${beginning}_$(echo -- $extra_stuff $CFLAGS $LDFLAGS | /usr/bin/env md5sum)" # md5sum to make it smaller, cflags to force rebuild if changes
  touch_name=$(echo "$touch_name" | sed "s/ //g")                                                # md5sum introduces spaces, remove them
  echo "$touch_name"                                                                             # bash cruddy return system LOL
}

do_configure() {
  local configure_options="$1"
  local configure_name="$2"
  if [[ $configure_name == "" ]]; then
    configure_name="./configure"
  fi
  local cur_dir2=$(pwd)
  local english_name=$(basename $cur_dir2)
  local touch_name=$(get_small_touchfile_name already_configured "$configure_options $configure_name")
  if [ ! -f "$touch_name" ]; then
    # make uninstall # does weird things when run under ffmpeg src so disabled for now...

    echo "configuring $english_name ($PWD) as $ PKG_CONFIG_PATH=$PKG_CONFIG_PATH PATH=$mingw_bin_path:\$PATH $configure_name $configure_options" # say it now in case bootstrap fails etc.
    echo "all touch files" already_configured* touchname= "$touch_name"
    echo "config options "$configure_options $configure_name""
    if [ -f bootstrap ]; then
      ./bootstrap # some need this to create ./configure :|
    fi
    if [[ ! -f $configure_name && -f bootstrap.sh ]]; then # fftw wants to only run this if no configure :|
      ./bootstrap.sh
    fi
    if [[ ! -f $configure_name ]]; then
      autoreconf -fiv # a handful of them require this to create ./configure :|
    fi
    rm -f already_* # reset
    nice -n 5 "$configure_name" $configure_options || {
      echo "failed configure $english_name"
      exit 1
    } # less nicey than make (since single thread, and what if you're running another ffmpeg nice build elsewhere?)
    touch -- "$touch_name"
    echo "doing preventative make clean"
    nice make clean -j $cpu_count # sometimes useful when files change, etc.
  #else
  #  echo "already configured $(basename $cur_dir2)"
  fi
}

do_make() {
  local extra_make_options="$1 -j $cpu_count"
  local cur_dir2=$(pwd)
  local touch_name=$(get_small_touchfile_name already_ran_make "$extra_make_options")

  if [ ! -f $touch_name ]; then
    echo
    echo "Making $cur_dir2 as $ PATH=$mingw_bin_path:\$PATH make $extra_make_options"
    echo
    if [ ! -f configure ]; then
      nice make clean -j $cpu_count # just in case helpful if old junk left around and this is a 're make' and wasn't cleaned at reconfigure time
    fi
    nice make $extra_make_options || exit 1
    touch $touch_name || exit 1 # only touch if the build was OK
  else
    echo "Already made $(dirname "$cur_dir2") $(basename "$cur_dir2") ..."
  fi
}

do_make_and_make_install() {
  local extra_make_options="$1"
  do_make "$extra_make_options"
  do_make_install "$extra_make_options"
}

do_make_install() {
  local extra_make_install_options="$1"
  local override_make_install_options="$2" # startingly, some need/use something different than just 'make install'
  if [[ -z $override_make_install_options ]]; then
    local make_install_options="install $extra_make_install_options"
  else
    local make_install_options="$override_make_install_options $extra_make_install_options"
  fi
  local touch_name=$(get_small_touchfile_name already_ran_make_install "$make_install_options")
  if [ ! -f $touch_name ]; then
    echo "make installing $(pwd) as $ PATH=$mingw_bin_path:\$PATH make $make_install_options"
    nice make $make_install_options || exit 1
    touch $touch_name || exit 1
  fi
}

do_cmake() {
  extra_args="$1"
  local build_from_dir="$2"
  if [[ -z $build_from_dir ]]; then
    build_from_dir="."
  fi
  local touch_name=$(get_small_touchfile_name already_ran_cmake "$extra_args")

  if [ ! -f $touch_name ]; then
    rm -f already_* # reset so that make will run again if option just changed
    local cur_dir2=$(pwd)
    echo doing cmake in $cur_dir2 with PATH=$mingw_bin_path:\$PATH with extra_args=$extra_args like this:
    if [[ $compiler_flavors != "native" ]]; then
      local command="${build_from_dir} -DENABLE_STATIC_RUNTIME=1 -DBUILD_SHARED_LIBS=0 -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_FIND_ROOT_PATH=$mingw_w64_x86_64_prefix -DCMAKE_FIND_ROOT_PATH_MODE_PROGRAM=NEVER -DCMAKE_FIND_ROOT_PATH_MODE_LIBRARY=ONLY -DCMAKE_FIND_ROOT_PATH_MODE_INCLUDE=ONLY -DCMAKE_RANLIB=${cross_prefix}ranlib -DCMAKE_C_COMPILER=${cross_prefix}gcc -DCMAKE_CXX_COMPILER=${cross_prefix}g++ -DCMAKE_RC_COMPILER=${cross_prefix}windres -DCMAKE_INSTALL_PREFIX=$mingw_w64_x86_64_prefix $extra_args"
    else
      local command="${build_from_dir} -DENABLE_STATIC_RUNTIME=1 -DBUILD_SHARED_LIBS=0 -DCMAKE_INSTALL_PREFIX=$mingw_w64_x86_64_prefix $extra_args"
    fi
    echo "doing ${cmake_command}  -G\"Unix Makefiles\" $command"
    nice -n 5 ${cmake_command} -G"Unix Makefiles" $command || exit 1
    touch $touch_name || exit 1
  fi
}

do_cmake_from_build_dir() { # some sources don't allow it, weird XXX combine with the above :)
  source_dir="$1"
  extra_args="$2"
  do_cmake "$extra_args" "$source_dir"
}

do_cmake_and_install() {
  do_cmake "$1"
  do_make_and_make_install
}

apply_patch() {
  local url=$1 # if you want it to use a local file instead of a url one [i.e. local file with local modifications] specify it like file://localhost/full/path/to/filename.patch
  local patch_type=$2
  if [[ -z $patch_type ]]; then
    patch_type="-p0" # some are -p1 unfortunately, git's default
  fi
  local patch_name=$(basename $url)
  local patch_done_name="$patch_name.done"
  if [[ ! -e $patch_done_name ]]; then
    if [[ -f $patch_name ]]; then
      rm $patch_name || exit 1 # remove old version in case it has been since updated on the server...
    fi
    curl -4 --retry 5 $url -O --fail || echo_and_exit "unable to download patch file $url"
    echo "applying patch $patch_name"
    patch $patch_type <"$patch_name" || exit 1
    touch $patch_done_name || exit 1
    # too crazy, you can't do do_configure then apply a patch?
    # rm -f already_ran* # if it's a new patch, reset everything too, in case it's really really really new
  #else
  #  echo "patch $patch_name already applied" # too chatty
  fi
}

echo_and_exit() {
  echo "failure, exiting: $1"
  exit 1
}

# takes a url, output_dir as params, output_dir optional
download_and_unpack_file() {
  url="$1"
  output_name=$(basename $url)
  output_dir="$2"
  if [[ -z $output_dir ]]; then
    output_dir=$(basename $url | sed s/\.tar\.*//) # remove .tar.xx
  fi
  if [ ! -f "$output_dir/unpacked.successfully" ]; then
    echo "downloading $url" # redownload in case failed...
    if [[ -f $output_name ]]; then
      rm $output_name || exit 1
    fi

    #  From man curl
    #  -4, --ipv4
    #  If curl is capable of resolving an address to multiple IP versions (which it is if it is  IPv6-capable),
    #  this option tells curl to resolve names to IPv4 addresses only.
    #  avoid a "network unreachable" error in certain [broken Ubuntu] configurations a user ran into once
    #  -L means "allow redirection" or some odd :|

    curl -4 "$url" --retry 50 -O -L --fail || echo_and_exit "unable to download $url"
    echo "unzipping $output_name ..."
    tar -xf "$output_name" || unzip "$output_name" || exit 1
    touch "$output_dir/unpacked.successfully" || exit 1
    rm "$output_name" || exit 1
  fi
}

generic_configure() {
  local extra_configure_options="$1"
  do_configure "--host=$host_target --prefix=$mingw_w64_x86_64_prefix --disable-shared --enable-static $extra_configure_options"
}

generic_configure_make_install() {
  if [ $# -gt 0 ]; then
    echo "cant pass parameters to this method today, they'd be a bit ambiguous"
    echo "The following arguments where passed: $*"
    exit 1
  fi
  generic_configure # no parameters, force myself to break it up if needed
  do_make_and_make_install
}

gen_ld_script() {
  lib=$mingw_w64_x86_64_prefix/lib/$1
  lib_s="$2"
  if [[ ! -f $mingw_w64_x86_64_prefix/lib/lib$lib_s.a ]]; then
    echo "Generating linker script $lib: $2 $3"
    mv -f $lib $mingw_w64_x86_64_prefix/lib/lib$lib_s.a
    echo "GROUP ( -l$lib_s $3 )" >$lib
  fi
}

build_dlfcn() {
  do_git_checkout https://github.com/dlfcn-win32/dlfcn-win32.git
  cd dlfcn-win32_git
  if [[ ! -f Makefile.bak ]]; then # Change CFLAGS.
    sed -i.bak "s/-O3/-O2/" Makefile
  fi
  do_configure "--prefix=$mingw_w64_x86_64_prefix --cross-prefix=$cross_prefix" # rejects some normal cross compile options so custom here
  do_make_and_make_install
  gen_ld_script libdl.a dl_s -lpsapi # dlfcn-win32's 'README.md': "If you are linking to the static 'dl.lib' or 'libdl.a', then you would need to explicitly add 'psapi.lib' or '-lpsapi' to your linking command, depending on if MinGW is used."
  cd ..
}

build_bzip2() {
  download_and_unpack_file https://sourceware.org/pub/bzip2/bzip2-1.0.8.tar.gz
  cd bzip2-1.0.8
  apply_patch file://$patch_dir/bzip2-1.0.8_brokenstuff.diff
  if [[ ! -f ./libbz2.a ]] || [[ -f $mingw_w64_x86_64_prefix/lib/libbz2.a && $(/usr/bin/env md5sum ./libbz2.a) != $(/usr/bin/env md5sum $mingw_w64_x86_64_prefix/lib/libbz2.a) ]]; then # Not built or different build installed
    do_make "$make_prefix_options libbz2.a"
    install -m644 bzlib.h $mingw_w64_x86_64_prefix/include/bzlib.h
    install -m644 libbz2.a $mingw_w64_x86_64_prefix/lib/libbz2.a
  else
    echo "Already made bzip2-1.0.8"
  fi
  cd ..
}

build_liblzma() {
  download_and_unpack_file https://sourceforge.net/projects/lzmautils/files/xz-5.2.5.tar.xz
  cd xz-5.2.5
  generic_configure "--disable-xz --disable-xzdec --disable-lzmadec --disable-lzmainfo --disable-scripts --disable-doc --disable-nls"
  do_make_and_make_install
  cd ..
}

build_zlib() {
  download_and_unpack_file https://github.com/madler/zlib/archive/v1.2.11.tar.gz zlib-1.2.11
  cd zlib-1.2.11
  local make_options
  if [[ $compiler_flavors == "native" ]]; then
    export CFLAGS="$CFLAGS -fPIC" # For some reason glib needs this even though we build a static library
  else
    export ARFLAGS=rcs # Native can't take ARFLAGS; https://stackoverflow.com/questions/21396988/zlib-build-not-configuring-properly-with-cross-compiler-ignores-ar
  fi
  do_configure "--prefix=$mingw_w64_x86_64_prefix --static"
  do_make_and_make_install "$make_prefix_options ARFLAGS=rcs"
  if [[ $compiler_flavors == "native" ]]; then
    reset_cflags
  else
    unset ARFLAGS
  fi
  cd ..
}

build_iconv() {
  download_and_unpack_file https://ftp.gnu.org/pub/gnu/libiconv/libiconv-1.16.tar.gz
  cd libiconv-1.16
  generic_configure "--disable-nls"
  do_make "install-lib" # No need for 'do_make_install', because 'install-lib' already has install-instructions.
  cd ..
}

build_libzimg() {
  do_git_checkout https://github.com/sekrit-twc/zimg.git zimg_git
  cd zimg_git
  generic_configure_make_install
  cd ..
}

build_libopenjpeg() {
  do_git_checkout https://github.com/uclouvain/openjpeg.git # basically v2.3+
  cd openjpeg_git
  do_cmake_and_install "-DBUILD_CODEC=0"
  cd ..
}

build_libpng() {
  do_git_checkout https://github.com/glennrp/libpng.git
  cd libpng_git
  generic_configure
  do_make_and_make_install
  cd ..
}

build_libwebp() {
  do_git_checkout https://chromium.googlesource.com/webm/libwebp.git libwebp_git v1.2.4
  cd libwebp_git
  export LIBPNG_CONFIG="$mingw_w64_x86_64_prefix/bin/libpng-config --static" # LibPNG somehow doesn't get autodetected.
  generic_configure "--disable-wic"
  do_make_and_make_install
  unset LIBPNG_CONFIG
  cd ..
}

build_harfbuzz() {
  local new_build=false
  do_git_checkout https://github.com/harfbuzz/harfbuzz.git harfbuzz_git "origin/main"
  if [ ! -f harfbuzz_git/already_done_harf ]; then # Not done or new master, so build
    new_build=true
  fi

  # basically gleaned from https://gist.github.com/roxlu/0108d45308a0434e27d4320396399153
  build_freetype "--without-harfbuzz" $new_build # Check for initial or new freetype or force rebuild if needed
  local new_freetype=$?
  if $new_build || [ $new_freetype = 0 ]; then # 0 is true
    rm -f harfbuzz_git/already*                # Force rebuilding in case only freetype has changed
    # cmake no .pc file generated so use configure :|
    cd harfbuzz_git
    if [ ! -f configure ]; then
      ./autogen.sh # :|
    fi
    export LDFLAGS=-lpthread                                                   # :|
    generic_configure "--with-freetype=yes --with-fontconfig=no --with-icu=no" # no fontconfig, don't want another circular what? icu is #372
    unset LDFLAGS
    do_make_and_make_install
    cd ..

    build_freetype "--with-harfbuzz" true # with harfbuzz now...
    touch harfbuzz_git/already_done_harf
    echo "Done harfbuzz"
  else
    echo "Already done harfbuzz"
  fi
  sed -i.bak 's/-lfreetype.*/-lfreetype -lharfbuzz -lpthread/' "$PKG_CONFIG_PATH/freetype2.pc"                         # for some reason it lists harfbuzz as Requires.private only??
  sed -i.bak 's/-lharfbuzz.*/-lharfbuzz -lfreetype/' "$PKG_CONFIG_PATH/harfbuzz.pc"                                    # does anything even use this?
  sed -i.bak 's/libfreetype.la -lbz2/libfreetype.la -lharfbuzz -lbz2/' "${mingw_w64_x86_64_prefix}/lib/libfreetype.la" # XXX what the..needed?
  sed -i.bak 's/libfreetype.la -lbz2/libfreetype.la -lharfbuzz -lbz2/' "${mingw_w64_x86_64_prefix}/lib/libharfbuzz.la"
}

build_freetype() {
  local force_build=$2
  local new_build=1
  if [[ ! -f freetype-2.10.4/already_done_freetype || $force_build == true ]]; then
    download_and_unpack_file https://sourceforge.net/projects/freetype/files/freetype2/2.10.4/freetype-2.10.4.tar.xz
    rm -f freetype-2.10.4/already*
    cd freetype-2.10.4
    apply_patch file://$patch_dir/freetype2-crosscompiled-apinames.diff # src/tools/apinames.c gets crosscompiled and makes the compilation fail
    # harfbuzz autodetect :|
    generic_configure "--with-bzip2 $1"
    do_make_and_make_install
    touch already_done_freetype
    new_build=0
    cd ..
  fi
  return $new_build # Give caller a way to know if a new build was done
}

build_libxml2() {
  download_and_unpack_file http://xmlsoft.org/sources/libxml2-2.9.10.tar.gz libxml2-2.9.10
  cd libxml2-2.9.10
  generic_configure "--with-ftp=no --with-http=no --with-python=no"
  do_make_and_make_install
  cd ..
}

build_fontconfig() {
  download_and_unpack_file https://www.freedesktop.org/software/fontconfig/release/fontconfig-2.13.92.tar.xz
  cd fontconfig-2.13.92
  #export CFLAGS= # compile fails with -march=sandybridge ... with mingw 4.0.6 at least ...
  generic_configure "--enable-iconv --enable-libxml2 --disable-docs --with-libiconv" # Use Libxml2 instead of Expat.
  do_make_and_make_install
  #reset_cflags
  cd ..
}

build_libogg() {
  do_git_checkout https://github.com/xiph/ogg.git
  cd ogg_git
  generic_configure_make_install
  cd ..
}

build_libvorbis() {
  do_git_checkout https://github.com/xiph/vorbis.git
  cd vorbis_git
  generic_configure "--disable-docs --disable-examples --disable-oggtest"
  do_make_and_make_install
  cd ..
}

build_libopus() {
  do_git_checkout https://github.com/xiph/opus.git
  cd opus_git
  generic_configure "--disable-doc --disable-extra-programs --disable-stack-protector"
  do_make_and_make_install
  cd ..
}

build_libtheora() {
  do_git_checkout https://github.com/xiph/theora.git
  cd theora_git
  generic_configure "--disable-doc --disable-spec --disable-oggtest --disable-vorbistest --disable-examples --disable-asm" # disable asm: avoid [theora @ 0x1043144a0]error in unpack_block_qpis in 64 bit... [OK OS X 64 bit tho...]
  do_make_and_make_install
  cd ..
}

build_libsndfile() {
  do_git_checkout https://github.com/libsndfile/libsndfile.git
  cd libsndfile_git
  generic_configure "--disable-sqlite --disable-external-libs --disable-full-suite"
  do_make_and_make_install
  if [ "$1" = "install-libgsm" ]; then
    if [[ ! -f $mingw_w64_x86_64_prefix/lib/libgsm.a ]]; then
      install -m644 src/GSM610/gsm.h $mingw_w64_x86_64_prefix/include/gsm.h || exit 1
      install -m644 src/GSM610/.libs/libgsm.a $mingw_w64_x86_64_prefix/lib/libgsm.a || exit 1
    else
      echo "already installed GSM 6.10 ..."
    fi
  fi
  cd ..
}

build_lame() {
  do_svn_checkout https://svn.code.sf.net/p/lame/svn/trunk/lame lame_svn
  cd lame_svn
  sed -i.bak '1s/^\xEF\xBB\xBF//' libmp3lame/i386/nasm.h # Remove a UTF-8 BOM that breaks nasm if it's still there; should be fixed in trunk eventually https://sourceforge.net/p/lame/patches/81/
  generic_configure "--enable-nasm --enable-libmpg123"
  do_make_and_make_install
  cd ..
}

build_twolame() {
  do_git_checkout https://github.com/njh/twolame.git twolame_git "origin/main"
  cd twolame_git
  if [[ ! -f Makefile.am.bak ]]; then # Library only, front end refuses to build for some reason with git master
    sed -i.bak "/^SUBDIRS/s/ frontend.*//" Makefile.am || exit 1
  fi
  cpu_count=1 # maybe can't handle it http://betterlogic.com/roger/2017/07/mp3lame-woe/ comments
  generic_configure_make_install
  cpu_count=$original_cpu_count
  cd ..
}

build_mingw_std_threads() {
  do_git_checkout https://github.com/meganz/mingw-std-threads.git # it needs std::mutex too :|
  cd mingw-std-threads_git
  cp *.h "$mingw_w64_x86_64_prefix/include"
  cd ..
}

build_libsoxr() {
  do_git_checkout https://github.com/chirlu/soxr.git soxr_git
  cd soxr_git
  do_cmake_and_install "-DHAVE_WORDS_BIGENDIAN_EXITCODE=0 -DWITH_OPENMP=0 -DBUILD_TESTS=0 -DBUILD_EXAMPLES=0"
  cd ..
}

build_svt-av1() {
  do_git_checkout https://gitlab.com/AOMediaCodec/SVT-AV1.git
  cd SVT-AV1_git
  cd Build
  do_cmake_from_build_dir .. "-DCMAKE_BUILD_TYPE=Release -DCMAKE_SYSTEM_PROCESSOR=AMD64"
  do_make_and_make_install
  cd ../..
}

build_fribidi() {
  download_and_unpack_file https://github.com/fribidi/fribidi/releases/download/v1.0.9/fribidi-1.0.9.tar.xz # Get c2man errors building from repo
  cd fribidi-1.0.9
  generic_configure "--disable-debug --disable-deprecated --disable-docs"
  do_make_and_make_install
  cd ..
}

build_libxvid() {
  download_and_unpack_file https://downloads.xvid.com/downloads/xvidcore-1.3.7.tar.gz xvidcore
  cd xvidcore/build/generic
  apply_patch file://$patch_dir/xvidcore-1.3.7_static-lib.patch
  do_configure "--host=$host_target --prefix=$mingw_w64_x86_64_prefix" # no static option...
  do_make_and_make_install
  cd ../../..
}

build_libvpx() {
  do_git_checkout https://chromium.googlesource.com/webm/libvpx.git libvpx_git "origin/main"
  cd libvpx_git
  apply_patch file://$patch_dir/vpx_160_semaphore.patch -p1 # perhaps someday can remove this after 1.6.0 or mingw fixes it LOL
  if [[ $compiler_flavors == "native" ]]; then
    local config_options=""
  elif [[ $bits_target == "32" ]]; then
    local config_options="--target=x86-win32-gcc"
  else
    local config_options="--target=x86_64-win64-gcc"
  fi
  export CROSS="$cross_prefix"
  # VP8 encoder *requires* sse3 support
  do_configure "$config_options --prefix=$mingw_w64_x86_64_prefix --enable-ssse3 --enable-static --disable-shared --disable-examples --disable-tools --disable-docs --disable-unit-tests --enable-vp9-highbitdepth --extra-cflags=-fno-asynchronous-unwind-tables --extra-cflags=-mstackrealign" # fno for Error: invalid register for .seh_savexmm
  do_make_and_make_install
  unset CROSS
  cd ..
}

build_libaom() {
  do_git_checkout https://aomedia.googlesource.com/aom aom_git
  if [[ $compiler_flavors == "native" ]]; then
    local config_options=""
  elif [ "$bits_target" = "32" ]; then
    local config_options="-DCMAKE_TOOLCHAIN_FILE=../build/cmake/toolchains/x86-mingw-gcc.cmake -DAOM_TARGET_CPU=x86"
  else
    local config_options="-DCMAKE_TOOLCHAIN_FILE=../build/cmake/toolchains/x86_64-mingw-gcc.cmake -DAOM_TARGET_CPU=x86_64"
  fi
  mkdir -p aom_git/aom_build
  cd aom_git/aom_build
  do_cmake_from_build_dir .. $config_options
  do_make_and_make_install
  cd ../..
}

build_libx265() {
  local checkout_dir=x265
  local remote="https://bitbucket.org/multicoreware/x265_git"
  do_git_checkout "$remote" $checkout_dir "origin/stable"
  cd $checkout_dir

  local cmake_params="-DENABLE_SHARED=0" # build x265.exe

  if [ "$bits_target" = "32" ]; then
    cmake_params+=" -DWINXP_SUPPORT=1" # enable windows xp/vista compatibility in x86 build, since it still can I think...
  fi
  mkdir -p 8bit 10bit 12bit

  # Build 12bit (main12)
  cd 12bit
  local cmake_12bit_params="$cmake_params -DENABLE_CLI=0 -DHIGH_BIT_DEPTH=1 -DMAIN12=1 -DEXPORT_C_API=0"
  if [ "$bits_target" = "32" ]; then
    cmake_12bit_params="$cmake_12bit_params -DENABLE_ASSEMBLY=OFF" # apparently required or build fails
  fi
  do_cmake_from_build_dir ../source "$cmake_12bit_params"
  do_make
  cp libx265.a ../8bit/libx265_main12.a

  # Build 10bit (main10)
  cd ../10bit
  local cmake_10bit_params="$cmake_params -DENABLE_CLI=0 -DHIGH_BIT_DEPTH=1 -DENABLE_HDR10_PLUS=1 -DEXPORT_C_API=0"
  if [ "$bits_target" = "32" ]; then
    cmake_10bit_params="$cmake_10bit_params -DENABLE_ASSEMBLY=OFF" # apparently required or build fails
  fi
  do_cmake_from_build_dir ../source "$cmake_10bit_params"
  do_make
  cp libx265.a ../8bit/libx265_main10.a

  # Build 8 bit (main) with linked 10 and 12 bit then install
  cd ../8bit
  cmake_params="$cmake_params -DENABLE_CLI=1 -DEXTRA_LINK_FLAGS=-L. -DLINKED_10BIT=1 -DLINKED_12BIT=1"
  if [[ $compiler_flavors == "native" && $OSTYPE != darwin* ]]; then
    cmake_params+=" -DENABLE_SHARED=0 -DEXTRA_LIB='$(pwd)/libx265_main10.a;$(pwd)/libx265_main12.a;-ldl'" # Native multi-lib CLI builds are slightly broken right now; other option is to -DENABLE_CLI=0, but this seems to work (https://bitbucket.org/multicoreware/x265/issues/520)
  else
    cmake_params+=" -DEXTRA_LIB='$(pwd)/libx265_main10.a;$(pwd)/libx265_main12.a'"
  fi
  do_cmake_from_build_dir ../source "$cmake_params"
  do_make
  mv libx265.a libx265_main.a
  if [[ $compiler_flavors == "native" && $OSTYPE == darwin* ]]; then
    libtool -static -o libx265.a libx265_main.a libx265_main10.a libx265_main12.a 2>/dev/null
  else
    ${cross_prefix}ar -M <<EOF
CREATE libx265.a
ADDLIB libx265_main.a
ADDLIB libx265_main10.a
ADDLIB libx265_main12.a
SAVE
END
EOF
  fi
  make install # force reinstall in case you just switched from stable to not :|
  cd ../..
}

build_libx264() {
  local checkout_dir="x264"

  do_git_checkout "https://code.videolan.org/videolan/x264.git" $checkout_dir "origin/stable"
  cd $checkout_dir
  if [[ ! -f configure.bak ]]; then # Change CFLAGS.
    sed -i.bak "s/O3 -/O2 -/" configure
  fi

  local configure_flags="--host=$host_target --enable-static --cross-prefix=$cross_prefix --prefix=$mingw_w64_x86_64_prefix --enable-strip" # --enable-win32thread --enable-debug is another useful option here?
  configure_flags+=" --disable-lavf" # lavf stands for libavformat, there is no --enable-lavf option, either auto or disable...
  configure_flags+=" --bit-depth=all"
  for i in $CFLAGS; do
    configure_flags+=" --extra-cflags=$i" # needs it this way seemingly :|
  done

  # normal path non profile guided
  do_configure "$configure_flags"
  do_make
  make install # force reinstall in case changed stable -> unstable

  unset LAVF_LIBS
  unset LAVF_CFLAGS
  unset SWSCALE_LIBS
  cd ..
}

build_libjpeg_turbo() {
  do_git_checkout https://github.com/libjpeg-turbo/libjpeg-turbo libjpeg-turbo_git "origin/main"
  cd libjpeg-turbo_git
  local cmake_params="-DENABLE_SHARED=0 -DCMAKE_ASM_NASM_COMPILER=yasm"
  if [[ $compiler_flavors != "native" ]]; then
    cmake_params+=" -DCMAKE_TOOLCHAIN_FILE=toolchain.cmake"
    local target_proc=AMD64
    if [ "$bits_target" = "32" ]; then
      target_proc=X86
    fi
    cat >toolchain.cmake <<EOF
set(CMAKE_SYSTEM_NAME Windows)
set(CMAKE_SYSTEM_PROCESSOR ${target_proc})
set(CMAKE_C_COMPILER ${cross_prefix}gcc)
set(CMAKE_RC_COMPILER ${cross_prefix}windres)
EOF
  fi
  do_cmake_and_install "$cmake_params"
  cd ..
}

# set some parameters initial values
patch_dir="$(pwd)/patches"
cpu_count="$(grep -c processor /proc/cpuinfo 2>/dev/null)" # linux cpu count
if [ -z "$cpu_count" ]; then
  cpu_count=$(sysctl -n hw.ncpu | tr -d '\n') # OS X cpu count
  if [ -z "$cpu_count" ]; then
    echo "warning, unable to determine cpu count, defaulting to 1"
    cpu_count=1 # else default to just 1, instead of blank, which means infinite
  fi
fi

original_cpu_count=$cpu_count # save it away for some that revert it temporarily

# variables with their defaults
git_get_latest=y
original_cflags='-mtune=generic -O3'                      # high compatible by default, see #219, some other good options are listed below, or you could use -march=native to target your local box:
original_cppflags='-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=0' # Needed for mingw-w64 7 as FORTIFY_SOURCE is now partially implemented, but not actually working

#------------------------------------ Main ------------------------------------/

work_dir="/srv/win64"
bits_target=64
host_target='x86_64-w64-mingw32'
mingw_bin_path="/srv/mingw-w64-x86_64/bin"

cross_prefix="${mingw_bin_path}/x86_64-w64-mingw32-"
mingw_w64_x86_64_prefix="/srv/mingw-w64-x86_64/${host_target}"

make_prefix_options="CC=${cross_prefix}gcc AR=${cross_prefix}ar PREFIX=$mingw_w64_x86_64_prefix RANLIB=${cross_prefix}ranlib LD=${cross_prefix}ld STRIP=${cross_prefix}strip CXX=${cross_prefix}g++"

export PKG_CONFIG_PATH="${mingw_w64_x86_64_prefix}/lib/pkgconfig"
mkdir -p "$work_dir" "$mingw_w64_x86_64_prefix"
cd "$work_dir"

build_dlfcn
build_mingw_std_threads
build_zlib    # Zlib in FFmpeg is autodetected.
build_bzip2   # Bzlib (bzip2) in FFmpeg is autodetected.
build_liblzma # Lzma in FFmpeg is autodetected. Uses dlfcn.
build_iconv   # Iconv in FFmpeg is autodetected. Uses dlfcn.
build_libzimg # Uses dlfcn.
build_libopenjpeg
build_libpng                      # Needs zlib >= 1.0.4. Uses dlfcn.
build_libwebp                     # Uses dlfcn.
build_harfbuzz                    # harf does now include build_freetype # Uses zlib, bzip2, and libpng.
build_libxml2                     # Uses zlib, liblzma, iconv and dlfcn.
build_fontconfig                  # Needs freetype and libxml >= 2.6. Uses iconv and dlfcn.
build_libogg                      # Uses dlfcn.
build_libvorbis                   # Needs libogg >= 1.0. Uses dlfcn.
build_libopus                     # Uses dlfcn.
build_libtheora                   # Needs libogg >= 1.1. Needs libvorbis >= 1.0.1, sdl and libpng for test, programs and examples [disabled]. Uses dlfcn.
build_libsndfile "install-libgsm" # Needs libogg >= 1.1.3 and libvorbis >= 1.2.3 for external support [disabled]. Uses dlfcn. 'build_libsndfile "install-libgsm"' to install the included LibGSM 6.10.
build_lame                        # Uses dlfcn, mpg123
build_twolame                     # Uses libsndfile >= 1.0.0 and dlfcn.
build_libsoxr
build_svt-av1
build_fribidi # Uses dlfcn.
build_libxvid # FFmpeg now has native support, but libxvid still provides a better image.
build_libvpx
build_libx265
build_libaom
# TODO rav1e
# TODO libheif
build_libx264 # at bottom as it might internally build a copy of ffmpeg (which needs all the above deps...

# Create a tmp TARGET_DIR
TARGET_DIR="$(mktemp -d -t ffmpeg-windows-XXXXXXXXXX)"
trap 'rm -rf "$TARGET_DIR"' EXIT

do_configure \
  --pkg-config=pkg-config \
  --pkg-config-flags=--static \
  --enable-version3 \
  --disable-debug \
  --disable-w32threads \
  --arch='x86_64' \
  --target-os=mingw32 \
  --cross-prefix=$cross_prefix \
  --disable-schannel \
  --enable-gray \
  --enable-fontconfig \
  --enable-libfreetype \
  --enable-libfribidi \
  --enable-libgsm \
  --enable-libmp3lame \
  --enable-libopus \
  --enable-libsoxr \
  --enable-libtheora \
  --enable-libtwolame \
  --enable-libvorbis \
  --enable-libwebp \
  --enable-libzimg \
  --enable-libopenjpeg \
  --enable-libxml2 \
  --enable-opengl \
  --enable-libsvtav1 \
  --enable-libvpx \
  --enable-libaom \
  --extra-cflags=-DLIBTWOLAME_STATIC \
  --enable-gpl \
  --enable-libx264 \
  --enable-libx265 \
  --enable-libxvid \
  --enable-shared \
  --disable-static \
  --prefix="$TARGET_DIR"

# Not on macOS?
# if [[ $compiler_flavors != "native" ]]; then
#   config_options+=" --extra-libs=-lshlwapi" # lame needed this, no .pc file?
# fi
# config_options+=" --extra-libs=-lmpg123" # ditto

rm -f -- */*.a */*.dll *.exe # just in case some dependency library has changed, force it to re-link even if the ffmpeg source hasn't changed...
rm -f already_ran_make*
echo "doing ffmpeg make $(pwd)"

do_make_and_make_install # install ffmpeg as well (for shared, to separate out the .dll's, for things that depend on it like VLC, to create static libs)
