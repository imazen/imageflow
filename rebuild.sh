mkdir build
cd build
conan install -u --file ../conanfile.py -o build_tests=True --build missing
cd ..
conan build
