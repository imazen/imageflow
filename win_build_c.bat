cmd.exe /c "ci\wintools\win_verify_tools.bat"

REM EXPECTS CONAN_ARCH and TARGET_CPU from ci\wintools\SETUP_PATH.bat

cd c_components || exit /b
mkdir build
cd build || exit /b

conan install --generator txt --scope build_tests=True -o shared=True --build missing -s build_type=Release -s arch=%CONAN_ARCH%  -s target_cpu=%TARGET_CPU% -u ../ || exit /b

conan build ../ || exit /b
cd .. || exit /b

echo Clearing cached C component
conan remove imageflow_c/* -f
conan export imazen/testing  || exit /b
cd ..  || exit /b 
cd imageflow_core  || exit /b
conan install --build missing -s build_type=Release -s arch=%CONAN_ARCH%  -s target_cpu=%TARGET_CPU% || exit /b
cd .. || exit /b