
#get docker first

#then log in
docker login

#This installs the CLI via docker image
docker run dockercloud/cli -h

alias docker-cloud="docker run -it -v ~/.docker:/root/.docker:ro --rm dockercloud/cli"
# Company time
export DOCKERCLOUD_NAMESPACE=imazen

dc stack ls

docker-cloud stack create --name hello-world -f  