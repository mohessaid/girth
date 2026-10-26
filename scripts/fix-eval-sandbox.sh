#!/bin/sh
# Lets bubblewrap (the eval sandbox) create user namespaces on Ubuntu 24.04+, where AppArmor
# blocks them by default. Only /usr/bin/bwrap gets the exception. Run: sudo sh scripts/fix-eval-sandbox.sh
set -e
cat > /etc/apparmor.d/bwrap <<'PROFILE'
abi <abi/4.0>,
include <tunables/global>
profile bwrap /usr/bin/bwrap flags=(unconfined) {
  userns,
  include if exists <local/bwrap>
}
PROFILE
apparmor_parser -r /etc/apparmor.d/bwrap
echo "bwrap profile loaded: $(ls -l /etc/apparmor.d/bwrap)"
