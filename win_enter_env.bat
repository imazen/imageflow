echo "You will need to modify ~/.conan/settings.yml" 
echo "add: target_cpu: [x86, x86-64, nehalem, sandybridge, haswell, native]"

REM To target 64-bit, use this:
REM %COMSPEC% /c ""ci\wintools\SETUP_PATH.bat""

REM To target 32-bit, use this:
REM %COMSPEC% /c ""ci\wintools\SETUP_PATH.bat" x86"

%COMSPEC% /c ""ci\wintools\SETUP_PATH.bat""
