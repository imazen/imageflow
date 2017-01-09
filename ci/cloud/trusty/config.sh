#!/bin/bash
set -e

if [[ -z "$(cat droplet.name)" ]]; then
	exit 3
fi 

scp  -oStrictHostKeyChecking=no imageflow.conf "root@$(cat droplet.addr):/etc/supervisor/conf.d/imageflow.conf"

scp  -oStrictHostKeyChecking=no limits.conf "root@$(cat droplet.addr):/etc/security/limits.conf"

ssh -oStrictHostKeyChecking=no "root@$(cat droplet.addr)" /bin/bash <<EOF1
	echo "/tmp/cores/core.%e.%p.%h.%t" > /proc/sys/kernel/core_pattern
	mkdir /tmp/cores
	sudo ufw allow 3000
	sudo apt-get install -y supervisor
	sudo service supervisor restart
	supervisorctl reread
	supervisorctl update
EOF1

	#sudo systemctl enable supervisor