#!/bin/bash

export PATH="$PATH:/home/appuser/.local/bin"
export RESTARTED="false"

while true; do
    # always update yt-dlp
    pip3 install --break-system-packages --upgrade yt-dlp bgutil-ytdlp-pot-provider
    astionicbot
    if [ $? -eq 42 ]; then
        echo "astionicbot exited with code 42, restarting..."
        continue
    else
        echo "astionicbot exited with code $?, stopping..."
        break
    fi
done
