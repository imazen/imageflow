
echo ============ VERIFY TOOLS REPORT ==================
echo CMAKE MUST be 3.5+
cmake --version
echo RUST MUST BE NIGHTLY 2017-03-04 or later
rustc -V
cargo -V
echo Conan must be 0.21+
conan --version


echo ============ END VERIFY TOOLS REPORT ==============
