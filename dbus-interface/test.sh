#IP=192.168.124.164
IP=192.168.1.185

cargo build
scp ./org.crosfp.CrosFp.conf root@$IP:/etc/dbus-1/system.d
scp ../target/debug/dbus-interface root@$IP:/usr/local/bin/cros-dbus-interface
