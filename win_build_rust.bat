cmd.exe /c "win_verify_tools.bat"

REM set CONAN_ARCH=x86
set CONAN_ARCH=x86_64
SET RUST_TARGET=x86_64-pc-windows-msvc

cd c_components
conan export imazen/testing
cd ..
cd imageflow_core
conan install --build missing -s build_type=Release -s arch=%CONAN_ARCH%
cargo test
cargo build --target=%RUST_TARGET% --release
cargo doc --no-deps
cd ..
cd imageflow_tool
cargo test
cargo build --target=%RUST_TARGET% --release
cargo doc --no-deps
cd ..
cd imageflow_cdylib
cargo test
cargo build --target=%RUST_TARGET% --release
cargo doc --no-deps
cd ..

echo Copying to artifacts\staging

mkdir artifacts
mkdir artifacts\staging
mkdir artifacts\staging\doc
dir target\%RUST_TARGET%\release\
xcopy /Y target\%RUST_TARGET%\release\flow-proto1.exe  artifacts\staging\
xcopy /Y target\%RUST_TARGET%\release\imageflowrs.dll  artifacts\staging\
xcopy /Y target\%RUST_TARGET%\release\imageflow_tool.exe  artifacts\staging\
xcopy /Y /d target\doc  artifacts\staging\doc

