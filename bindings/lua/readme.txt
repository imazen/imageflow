You need luajit installed


sudo apt-get install luajit
brew install luajit


Run cargo build in ./imageflow_abi
Run ./generate_ffi.sh 
Run ./test.sh


This Lua stub binding may remain just that until Valgrind and Luajit get along again (Valgrind 3.9+ blocks MAP_32BIT). 

The primary purpose for this binding was to create the lightest weight path for valgrinding use of the external API
