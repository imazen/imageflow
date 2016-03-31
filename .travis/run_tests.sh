mkdir -p build
cd build
conan install --file ../conanfile.py -o build_tests=True --build missing
conan build --file ../conanfile.py
