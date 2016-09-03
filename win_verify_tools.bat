
echo ============ VERIFY TOOLS REPORT ==================
echo CMAKE MUST be 3.5+
cmake --version
echo RUST MUST BE NIGHTLY 1.13+
rustc -V
cargo -V
echo Conan must be 0.11+
conan --version


echo ============ END VERIFY TOOLS REPORT ==============