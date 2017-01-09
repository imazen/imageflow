# Prereq

1. jq and doctl must be installed. 
2. `doctl auth init`
3. Put the right SSH fingerprint in use_key.txt


Helpful commands

```
./fetch_cores.sh xenial
cd cores && ../mods/download_imageflow.sh
gdb imageflow_server core.*

> bt

############

ssh-keygen -lf ~/.ssh/id_rsa.pub


# https://stedolan.github.io/jq/ (1.5)
# sudo apt-get install jq (1.3)

#wget https://github.com/digitalocean/doctl/releases/download/v1.4.0/doctl-1.4.0-linux-amd64.tar.gz
#tar xf ~/doctl-1.4.0-linux-amd64.tar.
#sudo mv ./doctl /usr/local/bin

#doctl auth init

#Find the fingerprint

ssh-keygen -lf ~/.ssh/id_rsa.pub



#list regions

doctl compute region list

# List sizes

doctl compute size list

# Slug	Memory	VCPUs	Disk	Price Monthly	Price Hourly
# 512mb	512	1	20	5.00		0.007440
# 1gb	1024	1	30	10.00		0.014880
# 2gb	2048	2	40	20.00		0.029760
# 4gb	4096	2	60	40.00		0.059520
# 8gb	8192	4	80	80.00		0.119050
# 16gb	16384	8	160	160.00		0.238100
# m-16gb	16384	2	30	120.00		0.178570
# 32gb	32768	12	320	320.00		0.476190
# m-32gb	32768	4	90	240.00		0.357140
# 48gb	49152	16	480	480.00		0.714290
# m-64gb	65536	8	200	480.00		0.714290
# 64gb	65536	20	640	640.00		0.952380
# m-128gb	131072	16	340	960.00		1.428570
# m-224gb	229376	32	500	1680.00		2.500000
 doctl compute droplet list
```