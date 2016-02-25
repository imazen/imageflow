

# Auto-formatting source code

On Ubuntu 14.04

1. Ensure you have clang-format installed and you can run it as `clang-format`

sudo apt-get install clang-format-3.5

sudo ln -s /usr/bin/clang-format-3.5 /usr/bin/clang-format


2. Install git-clang-format

sudo wget -O /usr/local/bin/git-clang-format https://raw.githubusercontent.com/llvm-mirror/clang/master/tools/clang-format/git-clang-format

sudo chmod +x /usr/local/bin/git-clang-format


3. Clean up that nasty commit you just pushed

git clang-format --commit HEAD~1

git commit -m"Reformatting"

4. Reformat the whole repository

clang-format -i {lib,tests,.}/*.{c,h,cpp,hpp}

