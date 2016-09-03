REM SET RUST_TARGET=i686-pc-windows-msvc

SET RUST_TARGET=x86_64-pc-windows-msvc
SET RUST_ARTIFACT=rust-nightly-%RUST_TARGET%.exe

echo Fetching https://static.rust-lang.org/dist/%RUST_ARTIFACT%
curl -L -o  install_rust.exe https://static.rust-lang.org/dist/%RUST_ARTIFACT%
install_rust.exe /VERYSILENT /NORESTART /DIR="C:\Program Files (x86)\Rust"
set PATH=%PATH%;C:\Program Files (x86)\Rust\bin

