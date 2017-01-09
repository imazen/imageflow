#/bin/bash
set -e

"$( dirname "${BASH_SOURCE[0]}" )/validate_droplet.sh" "$@"

export IMAGEFLOW_COMMIT=7557de472008beec2bff8223626dbb566ee3e90d

ssh -oStrictHostKeyChecking=no "root@$(cat droplet.addr)" /bin/bash <<EOF1
	mkdir nightly && cd nightly && wget -nv -O ifs.tar.gz https://s3-us-west-1.amazonaws.com/imageflow-nightlies/commits/${IMAGEFLOW_COMMIT}/linux64.tar.gz
	tar xvzf ifs.tar.gz && mv ./imageflow_server ../ && cd .. && rm -rf nightly
	./imageflow_server --version
	./imageflow_server diagnose --smoke-test-core
	sudo mkdir -p /var/log/imageflow/
	sudo mkdir -p /var/lib/imageflow/data/
	sudo mkdir -p /srv/imageflow/
	sudo mkdir -p /etc/supervisor/conf.d/
EOF1
