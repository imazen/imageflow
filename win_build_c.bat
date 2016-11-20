cmd.exe /c "win_verify_tools.bat"

cd c_components
mkdir build
cd build

REM set CONAN_ARCH=x86
set CONAN_ARCH=x86_64
conan install --scope build_tests=True -o shared=True --build missing -s build_type=Release -s arch=%CONAN_ARCH%  -u ../
conan build ../
cd ..
cd ..

echo Clearing cached C component
conan remove imageflow_c/* -f
