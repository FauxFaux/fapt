    http_proxy=http://urika:3142 cargo run -- --keyring /etc/apt/trusted.gpg -c ./tmp/ -r 'debs http://deb.debian.org/debian/ sid main' source-ninja > ../examples/ninja/sid-main.ninja
