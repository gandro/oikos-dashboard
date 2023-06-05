#!/bin/sh
BASEDIR=/mnt/us/oikos
if [ ! -x "${BASEDIR}/oikos" ]; then
    eips "oikos binary not found"
    exit 1
fi

/etc/init.d/framework stop
lipc-set-prop com.lab126.powerd preventScreenSaver 1

eips -f -g splash.png
( cd "${BASEDIR}" && ./oikos 2>&1 | tee -a "oikos.log" )

lipc-set-prop com.lab126.powerd preventScreenSaver 0
/etc/init.d/framework start
