FROM scratch

ADD templates/ /etc/docker-gen/templates/

ADD ./true /true
ADD ./true-asm /trueasm

VOLUME /etc/nginx/conf.d
VOLUME /etc/nginx/vhost.d
VOLUME /etc/docker-gen/templates
VOLUME /usr/share/nginx/html

ENTRYPOINT ["/trueasm"]