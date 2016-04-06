mkdir -p build
cd build

#Download and build DSSIM
wget https://github.com/pornel/dssim/archive/master.tar.gz
tar xvzf master.tar.gz
cd dssim-master
make
cd ..
cp dssim-master/bin/dssim ./dssim

conan install --file ../conanfile.py -o build_tests=True --build missing -u
conan build --file ../conanfile.py
