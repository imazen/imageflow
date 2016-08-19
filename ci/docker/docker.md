

### FAQ

#### Cannot connect to the Docker daemon. Is the docker daemon running on this host?

See https://stackoverflow.com/questions/21871479/docker-cant-connect-to-docker-daemon


OS X only:

```
docker-machine start # start virtual machine for docker
docker-machine env  # it's helps to get environment variables
eval "$(docker-machine env default)" #set environment variables
```
