# CI scripts


* **nixtools\install_\*.sh scripts are not used! They are guidance if you need to install these components yourself**
* travis_install.sh is run first
* travis_run.sh is run next, which selects travis_run_docker.sh (run inside a docker container) or build.sh (run directly)
