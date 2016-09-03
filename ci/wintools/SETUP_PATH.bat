echo PATH is currently: %PATH%
echo .
echo .
set PATH=%PATH%;C:\Program Files\Git\bin;C:\Program Files\Git\mingw64\bin
set PATH=%PATH%;C:\Program Files (x86)\NASM;
set PATH=%PATH%;C:\Program Files (x86)\Rust\bin
set PATH=%PATH%;C:\Program Files\CMake\bin
set PATH=%PATH%;C:\Program Files (x86)\Conan\conan

echo Updated path to
echo %PATH%
echo .
echo NOW entering VS 14

%comspec% /k ""C:\Program Files (x86)\Microsoft Visual Studio 14.0\VC\vcvarsall.bat"" amd64