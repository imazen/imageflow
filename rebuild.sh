rm -rf build
rm libimageflow.so
rm conaninfo.txt
rm conanbuildinfo.cmake
rm *.user
rm conanfile.pyc
rm -rf node_frames
rm tests/visuals/*.png
rm tests/visuals/*.html
rm tests/visuals/*~
rm cacert.pem
rm -rf bin
rm *.png
rm *.jpg
rm *.jpeg
rm *.gif
rm *~
mkdir build
cd build
conan install -u --file ../conanfile.py -o build_tests=True --build missing
cd ..
conan build
