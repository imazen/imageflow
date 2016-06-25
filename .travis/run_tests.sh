mkdir -p build
cd build

#Download and build DSSIM
sudo apt-get install -y pkg-config libpng-devel
wget https://github.com/pornel/dssim/archive/master.tar.gz
tar xvzf master.tar.gz
cd dssim-master
make
cd bin
export PATH=$PATH:$(pwd)
cd ../..

conan install -o build_tests=True -o coverage=True --build missing -u ../
conan build ../
