mkdir -p build
cd build

#Download and build DSSIM
sudo apt-get install -y pkg-config libpng-dev
wget https://github.com/pornel/dssim/archive/c6ad29c5a2dc37d8610120486f09eda145621c84.tar.gz
tar xvzf c6ad29c5a2dc37d8610120486f09eda145621c84.tar.gz
cd dssim-c6ad29c5a2dc37d8610120486f09eda145621c84
make
cd bin
export PATH=$PATH:$(pwd)
cd ../..

conan install --scope build_tests=True --scope coverage=True --scope valgrind=${VALGRIND} --build missing -u ../
conan build ../
