#IP=192.168.124.164
IP=192.168.1.3

cargo build
scp ../target/debug/libcros_fp_pam.so root@$IP:/lib64/security