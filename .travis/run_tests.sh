mkdir -p build
cd build

#Download and build DSSIM
sudo apt-get install -y libpng12-dev
wget https://github.com/pornel/dssim/archive/master.tar.gz
tar xvzf master.tar.gz
cd dssim-master
make
cd bin
export PATH=$PATH:$(pwd)
cd ../..

conan install --file ../conanfile.py -o build_tests=True --build missing -u
conan build --file ../conanfile.py
