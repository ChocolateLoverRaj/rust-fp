#IP=192.168.124.164
IP=192.168.1.185
USER="test"

cargo build
scp ../target/debug/cli root@$IP:/usr/local/bin/cros-fp

#ssh $USER@$IP cros-fp add
