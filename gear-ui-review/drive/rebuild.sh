#!/bin/sh
# Quit my app (7980), rebuild the ci app, start it again on 7980.
cd /c/Users/arian/Projects/sg-ui/gear-ui-review/drive && node quit.mjs 2>/dev/null; sleep 3
cd /c/Users/arian/Projects/sg-ui
export PATH="$HOME/.cargo/bin:$PATH" CARGO_TARGET_DIR=C:/Users/arian/Projects/sg-ui/target CARGO_BUILD_JOBS=8
cargo build --profile ci -p solidcraft --bin solidcraft 2>&1 | grep -E "^(error|warning)|-->" | head -40
rm -f $LOCALAPPDATA/SolidCraft/control/7980.token
(./target/ci/solidcraft.exe --control 7980 > /dev/null 2>&1 &)
until [ -f $LOCALAPPDATA/SolidCraft/control/7980.token ]; do sleep 2; done; sleep 4
cd gear-ui-review/drive && node -e "import('./sc.mjs').then(async({connect})=>{const sc=await connect(7980);for(let i=0;i<3;i++){try{await sc.call('ui.recover',{action:'discard'})}catch(e){}};sc.close()})"
echo started
