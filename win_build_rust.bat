cmd.exe /c "ci\wintools\win_verify_tools.bat"

REM EXPECTS CARGO_INCREMENTAL and RUST_FLAGS from ci\wintools\SETUP_PATH.bat
cargo test --all --release || exit /b
cargo build --all --release || exit /b
cargo doc --no-deps --all --release || exit /b
