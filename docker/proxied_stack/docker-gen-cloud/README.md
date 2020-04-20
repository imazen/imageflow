# docker-gen for Docker Cloud

This is an enhancement to the [docker-gen](https://github.com/jwilder/docker-gen) image that adds support
for [Docker Cloud](https://cloud.docker.com).

This image is used in context of [this](https://blog.switchbit.io/developing-a-ghost-theme-with-gulp-part-5/) 
post on using [JrCs/docker-letsencrypt-nginx-proxy-companion](https://github.com/JrCs/docker-letsencrypt-nginx-proxy-companion)
to generate Let's Encrypt certificates for a Ghost specific stack. 

# The problem

The usual way of using `docker-gen` in conjunction with `docker-letsencrypt-nginx-proxy-companion` using the separate
container method, is as follows (as per the [docs](https://github.com/JrCs/docker-letsencrypt-nginx-proxy-companion#separate-containers-recommended-method)):

```
$ docker run -d \
    --name nginx-gen \
    --volumes-from nginx \
    -v /path/to/nginx.tmpl:/etc/docker-gen/templates/nginx.tmpl:ro \
    -v /var/run/docker.sock:/tmp/docker.sock:ro \
    jwilder/docker-gen \
    -notify-sighup nginx -watch -only-exposed -wait 5s:30s /etc/docker-gen/templates/nginx.tmpl /etc/nginx/conf.d/default.conf
```

however, within a Docker Cloud based environment we cannot use `-notify-sighup nginx` due to the fact that
the container names (on the actual nodes) do not match their [Service](https://docs.docker.com/docker-cloud/apps/stacks/) names.
The result is that the `nginx` container (Service) never get's reloaded to take advantage of the generated Nginx configuration.

# The solution

How we get around this is to add the [Docker Cloud CLI](https://github.com/docker/dockercloud-cli)
to the `docker-gen` image and add a script (`restart_service.sh`) that uses the CLI to redeploy a Service.
For example, the following configuration, using Docker Cloud [Stack file](https://docs.docker.com/docker-cloud/apps/stack-yaml-reference/) 
format, would be used to achieve the desired affect:

```
nginx-gen:
  image: donovanmuller/docker-gen-docker-cloud:1
  volumes:
    - "/var/run/docker.sock:/tmp/docker.sock:ro"
  volumes_from:
    - nginx-proxy
    - ghost-nginx-proxy-config
  entrypoint: /usr/local/bin/docker-gen -notify-output -notify "./restart_service.sh" -watch -only-exposed -wait 10s:30s /etc/docker-gen/templates/nginx.tmpl /etc/nginx/conf.d/default.conf
  environment:
    - PROXY_SERVICE_ENV_VAR=nginx-proxy
  roles:
   - global
```

Note the use of the `restart_service.sh` script with `-notify-output -notify "./restart_service.sh"`.
Instead of using `-notify-sighup` the script is executed which uses the `docker-cloud` CLI to redeploy the Service 
indicated by the environment variable `PROXY_SERVICE_ENV_VAR`. 
This variable represents the Service name (`nginx-proxy` in the example above) to redeploy, not the container name.

We also need the `global` [role](https://docs.docker.com/docker-cloud/apps/api-roles/) so that Docker Cloud can inject the `DOCKERCLOUD_AUTH` details needed by `docker-cloud` CLI 
to [authenticate](https://github.com/docker/dockercloud-cli#authentication) against.




