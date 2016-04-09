mkdir profiling
cd profiling
conan install --file ../conanfile.py -o profiling=True -o build_tests=False --build missing -u
conan build --file ../conanfile.py
build/bin/profile_imageflow
gprof build/bin/profile_imageflow gmon.out > ../profile.txt
cd ..
