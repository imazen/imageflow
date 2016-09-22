cmd.exe /c "win_verify_tools.bat"


echo "Run ci/wintools/install_openssl.bat to create C:\OpenSSL"
REM set CONAN_ARCH=x86
set CONAN_ARCH=x86_64
SET RUST_TARGET=x86_64-pc-windows-msvc


set OPENSSL_INCLUDE_DIR=C:\OpenSSL\include
set OPENSSL_LIB_DIR=C:\OpenSSL\lib
set OPENSSL_LIBS=ssleay32:libeay32

cd imageflow_server
cargo test
cargo build --target=%RUST_TARGET% --release
cargo doc --no-deps
cd ..

echo Copying to artifacts\staging

mkdir artifacts
mkdir artifacts\staging
mkdir artifacts\staging\doc
dir target\%RUST_TARGET%\release\
xcopy /Y target\%RUST_TARGET%\release\imageflow_server.exe  artifacts\staging\
xcopy /Y /E target\doc  artifacts\staging\doc




