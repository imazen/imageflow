echo PATH is currently: %PATH%
echo .
echo .
set PATH=%PATH%;C:\Program Files\Git\bin;C:\Program Files\Git\mingw64\bin
set PATH=%PATH%;C:\Program Files (x86)\NASM;
set PATH=%PATH%;C:\Program Files (x86)\Rust\bin
set PATH=%PATH%;C:\Program Files\CMake\bin

echo Updated path to
echo %PATH%
echo .

set RUST_TARGET=i686-pc-windows-msvc
set TARGET_CPU=sandybridge
set RUST_FLAGS=%RUST_FLAGS -C target-cpu=%TARGET_CPU%

set CARGO_INCREMENTAL=1
set RUST_TEST_THREADS=1
set VS_ARCH=x86

if [%1] == [x86] goto :x86
set RUST_TARGET=x86_64-pc-windows-msvc
set VS_ARCH=amd64
:x86

echo VS_ARCH=%VSARCH% RUST_TARGET=%RUST_TARGET% TARGET_CPU=%TARGET_CPU% CARGO_INCREMENTAL=%CARGO_INCREMENTAL%
echo NOW entering VS 14

%comspec% /k ""C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\vcvarsall.bat" %VS_ARCH%"
